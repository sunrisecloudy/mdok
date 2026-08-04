#include "mdok_curl_tool_parser.h"

#include "tool_cfgable.h"
#include "tool_getparam.h"
#include "tool_main.h"
#include "tool_stderr.h"

#include <limits.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#define MDOK_TOOL_MAX_ARGC ((size_t)4096)
#define MDOK_TOOL_MAX_ARG_BYTES ((size_t)(64u * 1024u * 1024u))
#define MDOK_TOOL_MAX_BODY_BYTES ((size_t)(128u * 1024u * 1024u))

static int valid_slice(mdok_curl_slice slice)
{
  return slice.len <= MDOK_TOOL_MAX_ARG_BYTES &&
         (slice.len == 0 || slice.ptr != NULL);
}

static int slice_equals_literal(mdok_curl_slice slice, const char *literal)
{
  size_t length;
  if(!valid_slice(slice) || !literal)
    return 0;
  length = strlen(literal);
  return slice.len == length &&
         (length == 0 || memcmp(slice.ptr, literal, length) == 0);
}

static struct GlobalConfig mdok_global_config;
static atomic_flag mdok_parser_lock = ATOMIC_FLAG_INIT;

static void clear_error(mdok_curl_error *error)
{
  if(!error)
    return;
  error->code = 0;
  error->argv_index = 0;
  error->message.ptr = NULL;
  error->message.len = 0;
}

static void set_error(mdok_curl_error *error, int32_t code, const char *message)
{
  static _Thread_local char text[512];
  size_t length;
  if(!message)
    message = "curl tool parser error";
  length = strlen(message);
  if(length >= sizeof(text))
    length = sizeof(text) - 1;
  memcpy(text, message, length);
  text[length] = 0;
  if(error) {
    error->code = code;
    error->argv_index = 0;
    error->message.ptr = (const uint8_t *)text;
    error->message.len = length;
  }
}

static void set_error_at(mdok_curl_error *error, int32_t code,
                         const char *message, size_t argv_index)
{
  set_error(error, code, message);
  if(error)
    error->argv_index = argv_index;
}

static char *copy_slice(mdok_curl_slice slice)
{
  char *copy;
  if(slice.len > MDOK_TOOL_MAX_ARG_BYTES || (slice.len && !slice.ptr))
    return NULL;
  copy = (char *)malloc(slice.len + 1);
  if(!copy)
    return NULL;
  if(slice.len)
    memcpy(copy, slice.ptr, slice.len);
  copy[slice.len] = 0;
  return copy;
}

static char *duplicate_string(const char *value)
{
  size_t length;
  char *copy;
  if(!value)
    return NULL;
  length = strlen(value);
  if(length > MDOK_TOOL_MAX_ARG_BYTES)
    return NULL;
  copy = (char *)malloc(length + 1);
  if(!copy)
    return NULL;
  memcpy(copy, value, length + 1);
  return copy;
}

enum parser_option_kind {
  PARSER_OPTION_NONE = 0,
  PARSER_OPTION_ARGUMENT,
  PARSER_OPTION_FILE_ARGUMENT,
};

static int slice_prefix(mdok_curl_slice slice, const char *prefix)
{
  size_t length;
  if(!valid_slice(slice) || !prefix)
    return 0;
  length = strlen(prefix);
  return slice.len >= length &&
         (length == 0 || memcmp(slice.ptr, prefix, length) == 0);
}

static int slice_find_byte(mdok_curl_slice slice, uint8_t byte, size_t *index)
{
  size_t current;
  if(!valid_slice(slice))
    return 0;
  for(current = 0; current < slice.len; current++) {
    if(slice.ptr[current] == byte) {
      if(index)
        *index = current;
      return 1;
    }
  }
  return 0;
}

static enum parser_option_kind option_kind(mdok_curl_slice option)
{
  if(slice_equals_literal(option, "--request") ||
     slice_equals_literal(option, "-X") ||
     slice_prefix(option, "--request=") ||
     slice_prefix(option, "-X"))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--header") ||
     slice_equals_literal(option, "-H") ||
     slice_prefix(option, "--header=") ||
     slice_prefix(option, "-H"))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--data") ||
     slice_equals_literal(option, "-d") ||
     slice_prefix(option, "--data=") ||
     slice_prefix(option, "-d"))
    return PARSER_OPTION_FILE_ARGUMENT;
  if(slice_equals_literal(option, "--data-raw") ||
     slice_prefix(option, "--data-raw="))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--data-binary") ||
     slice_prefix(option, "--data-binary="))
    return PARSER_OPTION_FILE_ARGUMENT;
  if(slice_equals_literal(option, "--data-urlencode") ||
     slice_prefix(option, "--data-urlencode="))
    return PARSER_OPTION_FILE_ARGUMENT;
  if(slice_equals_literal(option, "--json") ||
     slice_prefix(option, "--json="))
    return PARSER_OPTION_FILE_ARGUMENT;
  if(slice_equals_literal(option, "--range") ||
     slice_equals_literal(option, "-r") ||
     slice_prefix(option, "--range=") ||
     slice_prefix(option, "-r"))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--max-time") ||
     slice_equals_literal(option, "-m") ||
     slice_prefix(option, "--max-time=") ||
     slice_prefix(option, "-m"))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--connect-timeout") ||
     slice_prefix(option, "--connect-timeout="))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--max-redirs") ||
     slice_prefix(option, "--max-redirs="))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--user-agent") ||
     slice_equals_literal(option, "-A") ||
     slice_prefix(option, "--user-agent=") ||
     slice_prefix(option, "-A"))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--referer") ||
     slice_equals_literal(option, "-e") ||
     slice_prefix(option, "--referer=") ||
     slice_prefix(option, "-e"))
    return PARSER_OPTION_ARGUMENT;
  if(slice_equals_literal(option, "--compressed") ||
     slice_equals_literal(option, "--location") ||
     slice_equals_literal(option, "-L") ||
     slice_equals_literal(option, "--insecure") ||
     slice_equals_literal(option, "-k") ||
     slice_equals_literal(option, "--get") ||
     slice_equals_literal(option, "--no-buffer") ||
     slice_equals_literal(option, "--silent") ||
     slice_equals_literal(option, "-s") ||
     slice_equals_literal(option, "--show-error") ||
     slice_equals_literal(option, "-S") ||
     slice_equals_literal(option, "-q"))
    return PARSER_OPTION_NONE;
  return PARSER_OPTION_NONE;
}

static int is_allowed_option(mdok_curl_slice option)
{
  if(slice_equals_literal(option, "--"))
    return 1;
  if(option_kind(option) != PARSER_OPTION_NONE)
    return 1;
  return slice_equals_literal(option, "--compressed") ||
         slice_equals_literal(option, "--location") ||
         slice_equals_literal(option, "-L") ||
         slice_equals_literal(option, "--insecure") ||
         slice_equals_literal(option, "-k") ||
         slice_equals_literal(option, "--get") ||
         slice_equals_literal(option, "--no-buffer") ||
         slice_equals_literal(option, "--silent") ||
         slice_equals_literal(option, "-s") ||
         slice_equals_literal(option, "--show-error") ||
         slice_equals_literal(option, "-S") ||
         slice_equals_literal(option, "-q");
}

static mdok_curl_status preflight_argv(const mdok_curl_argv *argv,
                                       mdok_curl_error *error)
{
  size_t index;
  int options_enabled = 1;
  for(index = 0; index < argv->argc; index++) {
    mdok_curl_slice option = argv->argv[index];
    enum parser_option_kind kind;
    if(!valid_slice(option)) {
      set_error_at(error, 1, "invalid curl argv slice", index);
      return MDOK_CURL_PARSE_ERROR;
    }
    if(index == 0)
      continue;
    if(!options_enabled || option.len == 0 || option.ptr[0] != '-')
      continue;
    if(slice_equals_literal(option, "--")) {
      options_enabled = 0;
      continue;
    }
    if(!is_allowed_option(option)) {
      set_error_at(error, 301, "curl option is not allowed by the MDOK bridge", index);
      return MDOK_CURL_PARSE_ERROR;
    }
    kind = option_kind(option);
    if(kind != PARSER_OPTION_NONE &&
       !(slice_prefix(option, "--") && slice_find_byte(option, '=', NULL)) &&
       !(option.len > 2 && option.ptr[0] == '-' && option.ptr[1] != '-')) {
      if(index + 1 >= argv->argc) {
        set_error_at(error, 300, "missing curl option argument", index);
        return MDOK_CURL_PARSE_ERROR;
      }
      if(kind == PARSER_OPTION_FILE_ARGUMENT &&
         argv->argv[index + 1].len != 0 && argv->argv[index + 1].ptr[0] == '@') {
        set_error_at(error, 303, "file-backed curl data is not allowed", index);
        return MDOK_CURL_POLICY_ERROR;
      }
      index++;
    }
    else if(kind == PARSER_OPTION_FILE_ARGUMENT) {
      size_t equals;
      if(slice_find_byte(option, '=', &equals) &&
         equals + 1 < option.len && option.ptr[equals + 1] == '@') {
        set_error_at(error, 303, "file-backed curl data is not allowed", index);
        return MDOK_CURL_POLICY_ERROR;
      }
    }
  }
  return MDOK_CURL_OK;
}

static int copy_headers(struct curl_slist **out, const struct curl_slist *headers)
{
  const struct curl_slist *current;
  for(current = headers; current; current = current->next) {
    struct curl_slist *copy = curl_slist_append(*out, current->data);
    if(!copy)
      return 0;
    *out = copy;
  }
  return 1;
}

static void parser_begin(void)
{
  memset(&mdok_global_config, 0, sizeof(mdok_global_config));
  global = &mdok_global_config;
  global->showerror = FALSE;
  global->styled_output = TRUE;
  global->parallel_max = PARALLEL_DEFAULT;
  global->first = global->last = config_alloc();
}

static void parser_end(void)
{
  if(global == &mdok_global_config && global->last)
    config_free(global->last);
  memset(&mdok_global_config, 0, sizeof(mdok_global_config));
  global = NULL;
}

static mdok_curl_status reject_if_unsupported(
    const struct OperationConfig *config,
    mdok_curl_error *error)
{
  /* The bridge currently executes one safe easy transfer, not curl's
   * process-oriented output/parallel machinery.  Parse all options with the
   * upstream parser, then reject semantics that cannot be represented by the
   * narrow MDOK plan instead of silently changing them. */
  if(global->first != global->last || config->next || config->prev ||
     config->num_urls != 1 || config->url_list == NULL ||
     config->url_list->next != NULL || config->url_list->outfile != NULL ||
     config->url_list->infile != NULL || config->url_list->uploadset ||
     config->url_list->useremote || config->url_list->out_null ||
     config->proxy || config->preproxy || config->proxyuserpwd ||
     config->resolve || config->connect_to || config->unix_socket_path ||
     config->noproxy || config->cookies || config->cookiefiles ||
     config->cookiejar || config->netrc || config->netrc_opt ||
     config->mimeroot || config->mimepost || config->upload_flags != CURLULFLAG_SEEN ||
     config->writeout || global->trace_dump || global->libcurl ||
     global->parallel || global->parallel_connect || config->remote_name_all ||
     config->no_body || config->use_resume || config->resume_from_current ||
     config->httpreq == TOOL_HTTPREQ_PUT || config->httpreq == TOOL_HTTPREQ_MIMEPOST ||
     config->httpgetfields) {
    set_error(error, 301, "curl option is not supported by the MDOK single-transfer plan");
    return MDOK_CURL_PARSE_ERROR;
  }
  return MDOK_CURL_OK;
}

mdok_curl_status mdok_curl_tool_parse(
    const mdok_curl_argv *argv,
    mdok_curl_tool_result *out_result,
    mdok_curl_error *out_error)
{
  char **owned_argv = NULL;
  size_t index;
  ParameterError parse_error;
  struct OperationConfig *config;
  mdok_curl_status status;

  if(out_result)
    memset(out_result, 0, sizeof(*out_result));
  clear_error(out_error);
  if(!out_result || !argv || !argv->argv || argv->argc == 0 ||
     argv->argc > MDOK_TOOL_MAX_ARGC) {
    set_error(out_error, 1, "invalid curl argv");
    return MDOK_CURL_PARSE_ERROR;
  }
  if(!slice_equals_literal(argv->argv[0], "curl")) {
    set_error(out_error, 1, "argv must begin with curl");
    return MDOK_CURL_PARSE_ERROR;
  }
  status = preflight_argv(argv, out_error);
  if(status != MDOK_CURL_OK)
    return status;

  owned_argv = (char **)calloc(argv->argc, sizeof(*owned_argv));
  if(!owned_argv) {
    set_error(out_error, 0, "curl argv allocation failed");
    return MDOK_CURL_INTERNAL_ERROR;
  }
  while(atomic_flag_test_and_set(&mdok_parser_lock)) {
    /* curl's tool parser uses one process-global OperationConfig pointer.
     * Serialize the short parse/copy critical section rather than exposing
     * that mutable state to concurrent FFI callers. */
  }
  for(index = 0; index < argv->argc; index++) {
    owned_argv[index] = copy_slice(argv->argv[index]);
    if(!owned_argv[index]) {
      set_error(out_error, 1, "invalid or oversized curl argv slice");
      goto parse_error_out;
    }
  }
  tool_init_stderr();
  parser_begin();
  if(!global->first) {
    set_error(out_error, 0, "curl tool parser config allocation failed");
    parser_end();
    goto internal_error;
  }
  global->silent = TRUE;
  parse_error = parse_args((int)argv->argc, owned_argv);
  if(parse_error != PARAM_OK) {
    set_error(out_error, 300 + (int32_t)parse_error,
              "upstream curl tool parser rejected argv");
    parser_end();
    goto parse_error_out;
  }
  config = global->first;
  status = reject_if_unsupported(config, out_error);
  if(status != MDOK_CURL_OK) {
    parser_end();
    goto parse_error_out;
  }

  out_result->url = duplicate_string(config->url_list->url);
  out_result->method = duplicate_string(config->customrequest ?
                                        config->customrequest :
                                        (config->postfields ? "POST" : "GET"));
  out_result->timeout_ms = config->timeout_ms;
  out_result->connect_timeout_ms = config->connecttimeout_ms;
  out_result->max_redirs = config->maxredirs;
  out_result->follow = config->followlocation;
  out_result->insecure = config->insecure_ok;
  out_result->compressed = config->encoding;
  out_result->range = duplicate_string(config->range);
  out_result->user_agent = duplicate_string(config->useragent);
  out_result->referer = duplicate_string(config->referer);
  if(config->postfields) {
    size_t length = strlen(config->postfields);
    if(length > MDOK_TOOL_MAX_BODY_BYTES) {
      set_error(out_error, 300, "request body is too large");
      parser_end();
      goto internal_error;
    }
    out_result->body = (unsigned char *)malloc(length ? length : 1);
    if(out_result->body && length)
      memcpy(out_result->body, config->postfields, length);
    out_result->body_len = length;
  }
  if(!out_result->url || !out_result->method ||
     (config->postfields && !out_result->body) ||
     (config->range && !out_result->range) ||
     (config->useragent && !out_result->user_agent) ||
     (config->referer && !out_result->referer) ||
     !copy_headers(&out_result->headers, config->headers)) {
    set_error(out_error, 0, "curl tool parser result allocation failed");
    parser_end();
    goto internal_error;
  }
  parser_end();
  for(index = 0; index < argv->argc; index++)
    free(owned_argv[index]);
  free(owned_argv);
  atomic_flag_clear(&mdok_parser_lock);
  return MDOK_CURL_OK;

internal_error:
  mdok_curl_tool_result_free(out_result);
parse_error_out:
  for(index = 0; index < argv->argc; index++)
    free(owned_argv[index]);
  free(owned_argv);
  atomic_flag_clear(&mdok_parser_lock);
  return out_error && out_error->code == 0 ? MDOK_CURL_INTERNAL_ERROR :
                                             MDOK_CURL_PARSE_ERROR;
}

void mdok_curl_tool_result_free(mdok_curl_tool_result *result)
{
  if(!result)
    return;
  free(result->url);
  free(result->method);
  free(result->body);
  curl_slist_free_all(result->headers);
  free(result->range);
  free(result->user_agent);
  free(result->referer);
  memset(result, 0, sizeof(*result));
}
