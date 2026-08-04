#include "mdok_curl.h"

#include <curl/curl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct mdok_curl_plan {
  char *url;
  char *method;
  unsigned char *body;
  size_t body_len;
  struct curl_slist *headers;
  long timeout_ms;
  long connect_timeout_ms;
  long max_redirs;
  long follow;
  long insecure;
  long compressed;
  char *range;
  char *user_agent;
  char *referer;
};

struct mdok_curl_session { unsigned char reserved; };

static char last_error[512];

static char *duplicate_string(const char *value) {
  size_t length = strlen(value);
  char *copy = (char *)malloc(length + 1);
  if (copy == NULL) return NULL;
  memcpy(copy, value, length + 1);
  return copy;
}

static void set_error(mdok_curl_error *error, int32_t code, const char *message) {
  size_t length = strlen(message);
  if (length >= sizeof(last_error)) length = sizeof(last_error) - 1;
  memcpy(last_error, message, length);
  last_error[length] = 0;
  if (error != NULL) {
    error->code = code;
    error->argv_index = 0;
    error->message.ptr = (const uint8_t *)last_error;
    error->message.len = length;
  }
}

static char *copy_slice(mdok_curl_slice slice) {
  char *copy = (char *)malloc(slice.len + 1);
  if (copy == NULL) return NULL;
  memcpy(copy, slice.ptr, slice.len);
  copy[slice.len] = 0;
  return copy;
}

static int is_option(const char *value, const char *long_name, const char *short_name) {
  return strcmp(value, long_name) == 0 || (short_name != NULL && strcmp(value, short_name) == 0);
}

static int needs_argument(const char *option) {
  return is_option(option, "--request", "-X") || is_option(option, "--header", "-H") ||
         is_option(option, "--data", "-d") || is_option(option, "--data-raw", NULL) ||
         is_option(option, "--data-binary", NULL) || is_option(option, "--data-urlencode", NULL) ||
         is_option(option, "--json", NULL) || is_option(option, "--form", "-F") ||
         is_option(option, "--upload-file", "-T") || is_option(option, "--user", "-u") ||
         is_option(option, "--oauth2-bearer", NULL) || is_option(option, "--cookie", "-b") ||
         is_option(option, "--cookie-jar", "-c") || is_option(option, "--max-redirs", NULL) ||
         is_option(option, "--connect-timeout", NULL) || is_option(option, "--max-time", "-m") ||
         is_option(option, "--retry", NULL) || is_option(option, "--retry-delay", NULL) ||
         is_option(option, "--retry-max-time", NULL) || is_option(option, "--range", "-r") ||
         is_option(option, "--user-agent", "-A") || is_option(option, "--referer", "-e") ||
         is_option(option, "--cacert", NULL) || is_option(option, "--cert", NULL) ||
         is_option(option, "--key", NULL) || is_option(option, "--proxy", "-x") ||
         is_option(option, "--resolve", NULL) || is_option(option, "--connect-to", NULL) ||
         is_option(option, "--output", "-o") || is_option(option, "--write-out", "-w") ||
         is_option(option, "--config", "-K") || is_option(option, "--libcurl", NULL);
}

mdok_curl_status mdok_curl_global_init(void) {
  CURLcode result = curl_global_init(CURL_GLOBAL_DEFAULT);
  if (result != CURLE_OK) {
    set_error(NULL, (int32_t)result, curl_easy_strerror(result));
    return MDOK_CURL_INTERNAL_ERROR;
  }
  return MDOK_CURL_OK;
}

void mdok_curl_global_cleanup(void) { curl_global_cleanup(); }

const char *mdok_curl_last_error_message(void) { return last_error; }

void mdok_curl_reserved(void *userdata) { (void)userdata; }

mdok_curl_status mdok_curl_parse(const mdok_curl_argv *argv, mdok_curl_plan **out_plan, mdok_curl_error *out_error) {
  mdok_curl_plan *plan;
  size_t index;
  size_t url_count = 0;
  if (out_plan == NULL || argv == NULL || argv->argc == 0 || argv->argv == NULL) {
    set_error(out_error, 1, "invalid curl argv");
    return MDOK_CURL_PARSE_ERROR;
  }
  if (argv->argv[0].len != 4 || memcmp(argv->argv[0].ptr, "curl", 4) != 0) {
    set_error(out_error, 1, "argv must begin with curl");
    return MDOK_CURL_PARSE_ERROR;
  }
  plan = (mdok_curl_plan *)calloc(1, sizeof(*plan));
  if (plan == NULL) { set_error(out_error, 0, "allocation failed"); return MDOK_CURL_INTERNAL_ERROR; }
  plan->method = duplicate_string("GET");
  plan->max_redirs = 50;
  if (plan->method == NULL) { mdok_curl_plan_free(plan); set_error(out_error, 0, "allocation failed"); return MDOK_CURL_INTERNAL_ERROR; }
  for (index = 1; index < argv->argc; index++) {
    char *value = copy_slice(argv->argv[index]);
    if (value == NULL) { mdok_curl_plan_free(plan); set_error(out_error, 0, "allocation failed"); return MDOK_CURL_INTERNAL_ERROR; }
    if (strcmp(value, "-q") == 0 || strcmp(value, "--silent") == 0 || strcmp(value, "-s") == 0 ||
        strcmp(value, "--show-error") == 0 || strcmp(value, "-S") == 0 || strcmp(value, "--compressed") == 0 ||
        strcmp(value, "--no-buffer") == 0 || strcmp(value, "--location") == 0 || strcmp(value, "-L") == 0 ||
        strcmp(value, "--http1.0") == 0 || strcmp(value, "--http1.1") == 0 || strcmp(value, "--http2") == 0 ||
        strcmp(value, "--insecure") == 0 || strcmp(value, "-k") == 0) {
      if (strcmp(value, "--compressed") == 0) plan->compressed = 1;
      if (strcmp(value, "--location") == 0 || strcmp(value, "-L") == 0) plan->follow = 1;
      if (strcmp(value, "--insecure") == 0 || strcmp(value, "-k") == 0) plan->insecure = 1;
      free(value);
      continue;
    }
    if (strcmp(value, "--parallel") == 0 || strcmp(value, "--parallel-immediate") == 0 || strcmp(value, "--next") == 0 ||
        strcmp(value, "--output") == 0 || strcmp(value, "-o") == 0 || strcmp(value, "--remote-name") == 0 ||
        strcmp(value, "-O") == 0 || strcmp(value, "--write-out") == 0 || strcmp(value, "-w") == 0 ||
        strcmp(value, "--libcurl") == 0 || strcmp(value, "--trace") == 0 || strcmp(value, "--trace-ascii") == 0 ||
        strcmp(value, "--config") == 0 || strcmp(value, "-K") == 0) {
      free(value); mdok_curl_plan_free(plan); set_error(out_error, 301, "unsupported or multiple-transfer option"); return MDOK_CURL_PARSE_ERROR;
    }
    if (value[0] == '-' && !needs_argument(value)) {
      free(value); mdok_curl_plan_free(plan); set_error(out_error, 300, "unknown curl option"); return MDOK_CURL_PARSE_ERROR;
    }
    if (needs_argument(value)) {
      char *argument;
      if (index + 1 >= argv->argc) { free(value); mdok_curl_plan_free(plan); set_error(out_error, 300, "missing option argument"); return MDOK_CURL_PARSE_ERROR; }
      argument = copy_slice(argv->argv[++index]);
      if (argument == NULL) { free(value); mdok_curl_plan_free(plan); set_error(out_error, 0, "allocation failed"); return MDOK_CURL_INTERNAL_ERROR; }
      if (is_option(value, "--request", "-X")) { free(plan->method); plan->method = argument; argument = NULL; }
      else if (is_option(value, "--header", "-H")) { plan->headers = curl_slist_append(plan->headers, argument); }
      else if (is_option(value, "--data", "-d") || is_option(value, "--data-raw", NULL) || is_option(value, "--data-binary", NULL) || is_option(value, "--json", NULL) || is_option(value, "--data-urlencode", NULL)) {
        size_t argument_len = strlen(argument);
        unsigned char *body = (unsigned char *)realloc(plan->body, plan->body_len + argument_len + (plan->body_len ? 1 : 0));
        if (body == NULL) { free(argument); free(value); mdok_curl_plan_free(plan); set_error(out_error, 0, "allocation failed"); return MDOK_CURL_INTERNAL_ERROR; }
        plan->body = body;
        if (plan->body_len) plan->body[plan->body_len++] = '&';
        memcpy(plan->body + plan->body_len, argument, argument_len); plan->body_len += argument_len;
        free(plan->method); plan->method = duplicate_string("POST");
      } else if (is_option(value, "--range", "-r")) { plan->range = argument; argument = NULL; }
      else if (is_option(value, "--max-time", "-m")) { plan->timeout_ms = (long)(atof(argument) * 1000.0); }
      else if (is_option(value, "--connect-timeout", NULL)) { plan->connect_timeout_ms = (long)(atof(argument) * 1000.0); }
      else if (is_option(value, "--max-redirs", NULL)) { plan->max_redirs = atol(argument); }
      else if (is_option(value, "--user-agent", "-A")) { free(plan->user_agent); plan->user_agent = argument; argument = NULL; }
      else if (is_option(value, "--referer", "-e")) { free(plan->referer); plan->referer = argument; argument = NULL; }
      free(argument); free(value);
      continue;
    }
    if (strstr(value, "://") == NULL) { free(value); mdok_curl_plan_free(plan); set_error(out_error, 302, "only HTTP and HTTPS URLs are allowed"); return MDOK_CURL_POLICY_ERROR; }
    if (url_count++ != 0) { free(value); mdok_curl_plan_free(plan); set_error(out_error, 304, "multiple URLs are not allowed"); return MDOK_CURL_POLICY_ERROR; }
    plan->url = value;
  }
  if (url_count != 1) { mdok_curl_plan_free(plan); set_error(out_error, 304, "exactly one URL is required"); return MDOK_CURL_POLICY_ERROR; }
  *out_plan = plan;
  return MDOK_CURL_OK;
}

struct callback_context { const mdok_curl_callbacks *callbacks; void *userdata; };

static size_t deliver(mdok_curl_write_cb callback, const void *data, size_t length, void *userdata) {
  if (callback == NULL) return length;
  return callback((const uint8_t *)data, length, userdata);
}

static size_t body_deliver(const char *data, size_t size, size_t count, void *userdata) {
  struct callback_context *context = (struct callback_context *)userdata;
  return deliver(context->callbacks->body, data, size * count, context->userdata);
}

static size_t header_deliver(const char *data, size_t size, size_t count, void *userdata) {
  struct callback_context *context = (struct callback_context *)userdata;
  return deliver(context->callbacks->header, data, size * count, context->userdata);
}

static int progress_cancel(void *userdata, curl_off_t download_total, curl_off_t download_now,
                           curl_off_t upload_total, curl_off_t upload_now) {
  struct callback_context *context = (struct callback_context *)userdata;
  (void)download_total; (void)download_now; (void)upload_total; (void)upload_now;
  return context->callbacks->cancelled == NULL ? 0 : context->callbacks->cancelled(context->userdata);
}

mdok_curl_status mdok_curl_execute(mdok_curl_session *session, const mdok_curl_plan *plan, const mdok_curl_callbacks *callbacks, void *userdata, mdok_curl_error *out_error) {
  CURL *easy;
  CURLcode result;
  struct callback_context context;
  (void)session;
  if (plan == NULL || plan->url == NULL) { set_error(out_error, 0, "invalid curl plan"); return MDOK_CURL_INTERNAL_ERROR; }
  easy = curl_easy_init();
  if (easy == NULL) { set_error(out_error, 0, "curl_easy_init failed"); return MDOK_CURL_INTERNAL_ERROR; }
  curl_easy_setopt(easy, CURLOPT_URL, plan->url);
  curl_easy_setopt(easy, CURLOPT_CUSTOMREQUEST, plan->method);
  curl_easy_setopt(easy, CURLOPT_HTTPHEADER, plan->headers);
  curl_easy_setopt(easy, CURLOPT_FOLLOWLOCATION, plan->follow);
  curl_easy_setopt(easy, CURLOPT_MAXREDIRS, plan->max_redirs);
  curl_easy_setopt(easy, CURLOPT_SSL_VERIFYPEER, plan->insecure ? 0L : 1L);
  curl_easy_setopt(easy, CURLOPT_SSL_VERIFYHOST, plan->insecure ? 0L : 2L);
  curl_easy_setopt(easy, CURLOPT_ACCEPT_ENCODING, plan->compressed ? "" : NULL);
  if (plan->timeout_ms > 0) curl_easy_setopt(easy, CURLOPT_TIMEOUT_MS, plan->timeout_ms);
  if (plan->connect_timeout_ms > 0) curl_easy_setopt(easy, CURLOPT_CONNECTTIMEOUT_MS, plan->connect_timeout_ms);
  if (plan->range != NULL) curl_easy_setopt(easy, CURLOPT_RANGE, plan->range);
  if (plan->user_agent != NULL) curl_easy_setopt(easy, CURLOPT_USERAGENT, plan->user_agent);
  if (plan->referer != NULL) curl_easy_setopt(easy, CURLOPT_REFERER, plan->referer);
  if (plan->body != NULL) { curl_easy_setopt(easy, CURLOPT_POSTFIELDS, plan->body); curl_easy_setopt(easy, CURLOPT_POSTFIELDSIZE_LARGE, (curl_off_t)plan->body_len); }
  if (callbacks != NULL) {
    context.callbacks = callbacks;
    context.userdata = userdata;
    curl_easy_setopt(easy, CURLOPT_WRITEFUNCTION, body_deliver);
    curl_easy_setopt(easy, CURLOPT_WRITEDATA, &context);
    curl_easy_setopt(easy, CURLOPT_HEADERFUNCTION, header_deliver);
    curl_easy_setopt(easy, CURLOPT_HEADERDATA, &context);
    curl_easy_setopt(easy, CURLOPT_XFERINFOFUNCTION, progress_cancel);
    curl_easy_setopt(easy, CURLOPT_XFERINFODATA, &context);
    curl_easy_setopt(easy, CURLOPT_NOPROGRESS, 0L);
  }
  result = curl_easy_perform(easy);
  curl_easy_cleanup(easy);
  if (result != CURLE_OK) { set_error(out_error, (int32_t)result, curl_easy_strerror(result)); return result == CURLE_ABORTED_BY_CALLBACK ? MDOK_CURL_CANCELLED : MDOK_CURL_TRANSFER_ERROR; }
  return MDOK_CURL_OK;
}

void mdok_curl_plan_free(mdok_curl_plan *plan) {
  if (plan == NULL) return;
  free(plan->url); free(plan->method); free(plan->body); free(plan->range); free(plan->user_agent); free(plan->referer);
  curl_slist_free_all(plan->headers); free(plan);
}
