#include "mdok_curl.h"
#include "mdok_curl_tool_parser.h"
#include "tool_libinfo.h"

#include <curl/curl.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MDOK_CURL_MAX_ARGC ((size_t)4096)
#define MDOK_CURL_MAX_ARG_BYTES ((size_t)(64u * 1024u * 1024u))
#define MDOK_CURL_MAX_BODY_BYTES ((size_t)(128u * 1024u * 1024u))

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

struct mdok_curl_session {
  CURL *easy;
  CURLM *multi;
  CURLSH *share;
  atomic_flag in_use;
};

static _Thread_local char last_error[512];

static char *duplicate_string(const char *value) {
  size_t length;
  char *copy;
  if (value == NULL) return NULL;
  length = strlen(value);
  if (length >= MDOK_CURL_MAX_ARG_BYTES) return NULL;
  copy = (char *)malloc(length + 1);
  if (copy == NULL) return NULL;
  memcpy(copy, value, length + 1);
  return copy;
}

static void clear_error(mdok_curl_error *error) {
  if (error == NULL) return;
  error->code = 0;
  error->argv_index = 0;
  error->message.ptr = NULL;
  error->message.len = 0;
}

static void set_error_at(mdok_curl_error *error, int32_t code, const char *message, size_t argv_index) {
  size_t length;
  if (message == NULL) message = "unknown bridge error";
  length = strlen(message);
  if (length >= sizeof(last_error)) length = sizeof(last_error) - 1;
  memcpy(last_error, message, length);
  last_error[length] = 0;
  if (error != NULL) {
    error->code = code;
    error->argv_index = argv_index;
    error->message.ptr = (const uint8_t *)last_error;
    error->message.len = length;
  }
}

static void set_error(mdok_curl_error *error, int32_t code, const char *message) {
  set_error_at(error, code, message, 0);
}

static int valid_slice(mdok_curl_slice slice) {
  return slice.len <= MDOK_CURL_MAX_ARG_BYTES && (slice.len == 0 || slice.ptr != NULL);
}

static char *copy_slice(mdok_curl_slice slice) {
  char *copy;
  if (!valid_slice(slice)) return NULL;
  copy = (char *)malloc(slice.len + 1);
  if (copy == NULL) return NULL;
  if (slice.len != 0) memcpy(copy, slice.ptr, slice.len);
  copy[slice.len] = 0;
  return copy;
}

static int slice_equals_literal(mdok_curl_slice slice, const char *literal) {
  size_t length;
  if (!valid_slice(slice) || literal == NULL) return 0;
  length = strlen(literal);
  return slice.len == length && (length == 0 || memcmp(slice.ptr, literal, length) == 0);
}

static int is_option(const char *value, const char *long_name, const char *short_name) {
  return strcmp(value, long_name) == 0 || (short_name != NULL && strcmp(value, short_name) == 0);
}

mdok_curl_status mdok_curl_global_init(void) {
  CURLcode result = curl_global_init(CURL_GLOBAL_DEFAULT);
  if (result == CURLE_OK) {
    result = get_libcurl_info();
  }
  if (result != CURLE_OK) {
    set_error(NULL, (int32_t)result, curl_easy_strerror(result));
    return MDOK_CURL_INTERNAL_ERROR;
  }
  return MDOK_CURL_OK;
}

void mdok_curl_global_cleanup(void) { curl_global_cleanup(); }

const char *mdok_curl_last_error_message(void) { return last_error; }

void mdok_curl_reserved(void *userdata) { (void)userdata; }

mdok_curl_session *mdok_curl_session_new(void) {
  mdok_curl_session *session = (mdok_curl_session *)calloc(1, sizeof(*session));
  CURLSHcode share_result;
  if (session == NULL) {
    set_error(NULL, 0, "session allocation failed");
    return NULL;
  }
  atomic_flag_clear(&session->in_use);
  session->easy = curl_easy_init();
  if (session->easy == NULL) {
    free(session);
    set_error(NULL, 0, "curl_easy_init failed");
    return NULL;
  }
  session->multi = curl_multi_init();
  if (session->multi == NULL) {
    curl_easy_cleanup(session->easy);
    free(session);
    set_error(NULL, 0, "curl_multi_init failed");
    return NULL;
  }
  session->share = curl_share_init();
  if (session->share == NULL) {
    curl_multi_cleanup(session->multi);
    curl_easy_cleanup(session->easy);
    free(session);
    set_error(NULL, 0, "curl_share_init failed");
    return NULL;
  }
  share_result = curl_share_setopt(session->share, CURLSHOPT_SHARE, CURL_LOCK_DATA_COOKIE);
  if (share_result != CURLSHE_OK) {
    curl_share_cleanup(session->share);
    curl_multi_cleanup(session->multi);
    curl_easy_cleanup(session->easy);
    free(session);
    set_error(NULL, (int32_t)share_result, curl_share_strerror(share_result));
    return NULL;
  }
  return session;
}

void mdok_curl_session_free(mdok_curl_session *session) {
  if (session == NULL) return;
  if (atomic_flag_test_and_set(&session->in_use)) {
    set_error(NULL, 0, "curl session is busy");
    return;
  }
  curl_easy_cleanup(session->easy);
  session->easy = NULL;
  curl_multi_cleanup(session->multi);
  session->multi = NULL;
  curl_share_cleanup(session->share);
  session->share = NULL;
  free(session);
}

mdok_curl_status mdok_curl_parse(const mdok_curl_argv *argv, mdok_curl_plan **out_plan, mdok_curl_error *out_error) {
  mdok_curl_plan *plan = NULL;
  mdok_curl_tool_result parsed;
  mdok_curl_status status;
  if(out_plan != NULL)
    *out_plan = NULL;
  clear_error(out_error);
  if(!out_plan) {
    set_error(out_error, 1, "missing curl plan output");
    return MDOK_CURL_PARSE_ERROR;
  }
  status = mdok_curl_tool_parse(argv, &parsed, out_error);
  if(status != MDOK_CURL_OK)
    return status;
  if(!parsed.url || strstr(parsed.url, "://") == NULL) {
    mdok_curl_tool_result_free(&parsed);
    set_error(out_error, 302, "only HTTP and HTTPS URLs are allowed");
    return MDOK_CURL_POLICY_ERROR;
  }
  plan = (mdok_curl_plan *)calloc(1, sizeof(*plan));
  if(!plan) {
    mdok_curl_tool_result_free(&parsed);
    set_error(out_error, 0, "allocation failed");
    return MDOK_CURL_INTERNAL_ERROR;
  }
  plan->url = parsed.url;
  plan->method = parsed.method;
  plan->body = parsed.body;
  plan->body_len = parsed.body_len;
  plan->headers = parsed.headers;
  plan->timeout_ms = parsed.timeout_ms;
  plan->connect_timeout_ms = parsed.connect_timeout_ms;
  plan->max_redirs = parsed.max_redirs;
  plan->follow = parsed.follow;
  plan->insecure = parsed.insecure;
  plan->compressed = parsed.compressed;
  plan->range = parsed.range;
  plan->user_agent = parsed.user_agent;
  plan->referer = parsed.referer;
  memset(&parsed, 0, sizeof(parsed));
  *out_plan = plan;
  return MDOK_CURL_OK;
}

struct callback_context {
  const mdok_curl_callbacks *callbacks;
  void *userdata;
  int callback_failed;
  int cancelled;
};

static size_t deliver(struct callback_context *context, mdok_curl_write_cb callback, const void *data, size_t length) {
  size_t written;
  if (context == NULL || context->callbacks == NULL) return 0;
  if (callback == NULL) return length;
  if (length != 0 && data == NULL) {
    context->callback_failed = 1;
    return 0;
  }
  written = callback((const uint8_t *)data, length, context->userdata);
  if (written != length) {
    context->callback_failed = 1;
    return 0;
  }
  return written;
}

static size_t body_deliver(const char *data, size_t size, size_t count, void *userdata) {
  struct callback_context *context = (struct callback_context *)userdata;
  if (size != 0 && count > SIZE_MAX / size) {
    if (context != NULL) context->callback_failed = 1;
    return 0;
  }
  return deliver(context, context == NULL || context->callbacks == NULL ? NULL : context->callbacks->body, data, size * count);
}

static size_t header_deliver(const char *data, size_t size, size_t count, void *userdata) {
  struct callback_context *context = (struct callback_context *)userdata;
  if (size != 0 && count > SIZE_MAX / size) {
    if (context != NULL) context->callback_failed = 1;
    return 0;
  }
  return deliver(context, context == NULL || context->callbacks == NULL ? NULL : context->callbacks->header, data, size * count);
}

static int progress_cancel(void *userdata, curl_off_t download_total, curl_off_t download_now,
                           curl_off_t upload_total, curl_off_t upload_now) {
  struct callback_context *context = (struct callback_context *)userdata;
  (void)download_total; (void)download_now; (void)upload_total; (void)upload_now;
  if (context == NULL || context->callbacks == NULL || context->callbacks->cancelled == NULL) return 0;
  if (context->callbacks->cancelled(context->userdata) != 0) {
    context->cancelled = 1;
    return 1;
  }
  return 0;
}

static const char *http_version_name(long version) {
  switch (version) {
    case CURL_HTTP_VERSION_1_0: return "1.0";
    case CURL_HTTP_VERSION_1_1: return "1.1";
    case CURL_HTTP_VERSION_2_0: return "2";
    case CURL_HTTP_VERSION_2TLS: return "2";
    case CURL_HTTP_VERSION_2_PRIOR_KNOWLEDGE: return "2";
#ifdef CURL_HTTP_VERSION_3
    case CURL_HTTP_VERSION_3: return "3";
#endif
    default: return NULL;
  }
}

static void clear_transfer_info(mdok_curl_transfer_info *info) {
  if (info == NULL) return;
  memset(info, 0, sizeof(*info));
  info->response_code = -1;
  info->http_version = -1;
  info->total_time_us = -1;
  info->name_lookup_time_us = -1;
  info->connect_time_us = -1;
  info->appconnect_time_us = -1;
  info->pretransfer_time_us = -1;
  info->starttransfer_time_us = -1;
  info->redirect_time_us = -1;
  info->uploaded_bytes = -1;
  info->downloaded_bytes = -1;
  info->request_header_bytes = -1;
  info->response_header_bytes = -1;
  info->redirect_count = -1;
  info->num_connects = -1;
  info->ssl_verify_result = -1;
  info->primary_port = -1;
  info->local_port = -1;
}

static void set_info_string(mdok_curl_slice *out, const char *value) {
  if (out == NULL || value == NULL) return;
  out->ptr = (const uint8_t *)value;
  out->len = strlen(value);
}

static void fill_transfer_info(CURL *easy, mdok_curl_transfer_info *info) {
  long long_value;
  curl_off_t off_value;
  const char *string_value;
  if (easy == NULL || info == NULL) return;
  clear_transfer_info(info);
  if (curl_easy_getinfo(easy, CURLINFO_RESPONSE_CODE, &long_value) == CURLE_OK)
    info->response_code = (int64_t)long_value;
  if (curl_easy_getinfo(easy, CURLINFO_HTTP_VERSION, &long_value) == CURLE_OK) {
    info->http_version = (int64_t)long_value;
    set_info_string(&info->http_version_name, http_version_name(long_value));
  }
  if (curl_easy_getinfo(easy, CURLINFO_TOTAL_TIME_T, &off_value) == CURLE_OK)
    info->total_time_us = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_NAMELOOKUP_TIME_T, &off_value) == CURLE_OK)
    info->name_lookup_time_us = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_CONNECT_TIME_T, &off_value) == CURLE_OK)
    info->connect_time_us = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_APPCONNECT_TIME_T, &off_value) == CURLE_OK)
    info->appconnect_time_us = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_PRETRANSFER_TIME_T, &off_value) == CURLE_OK)
    info->pretransfer_time_us = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_STARTTRANSFER_TIME_T, &off_value) == CURLE_OK)
    info->starttransfer_time_us = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_REDIRECT_TIME_T, &off_value) == CURLE_OK)
    info->redirect_time_us = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_SIZE_UPLOAD_T, &off_value) == CURLE_OK)
    info->uploaded_bytes = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_SIZE_DOWNLOAD_T, &off_value) == CURLE_OK)
    info->downloaded_bytes = (int64_t)off_value;
  if (curl_easy_getinfo(easy, CURLINFO_HEADER_SIZE, &long_value) == CURLE_OK)
    info->response_header_bytes = (int64_t)long_value;
  if (curl_easy_getinfo(easy, CURLINFO_REQUEST_SIZE, &long_value) == CURLE_OK)
    info->request_header_bytes = (int64_t)long_value;
  if (curl_easy_getinfo(easy, CURLINFO_REDIRECT_COUNT, &long_value) == CURLE_OK)
    info->redirect_count = (int64_t)long_value;
  if (curl_easy_getinfo(easy, CURLINFO_NUM_CONNECTS, &long_value) == CURLE_OK)
    info->num_connects = (int64_t)long_value;
  if (curl_easy_getinfo(easy, CURLINFO_SSL_VERIFYRESULT, &long_value) == CURLE_OK)
    info->ssl_verify_result = (int64_t)long_value;
  if (curl_easy_getinfo(easy, CURLINFO_PRIMARY_PORT, &long_value) == CURLE_OK)
    info->primary_port = (int64_t)long_value;
  if (curl_easy_getinfo(easy, CURLINFO_LOCAL_PORT, &long_value) == CURLE_OK)
    info->local_port = (int64_t)long_value;
  if (curl_easy_getinfo(easy, CURLINFO_EFFECTIVE_URL, &string_value) == CURLE_OK)
    set_info_string(&info->effective_url, string_value);
  if (curl_easy_getinfo(easy, CURLINFO_PRIMARY_IP, &string_value) == CURLE_OK)
    set_info_string(&info->primary_ip, string_value);
  if (curl_easy_getinfo(easy, CURLINFO_LOCAL_IP, &string_value) == CURLE_OK)
    set_info_string(&info->local_ip, string_value);
  /* The native policy rejects explicit proxies and configure_easy disables
     inherited proxy settings. Keep this field explicit for callers. */
  info->used_proxy = 0;
}

static CURLcode configure_easy(mdok_curl_session *session, CURL *easy, const mdok_curl_plan *plan, const mdok_curl_callbacks *callbacks, struct callback_context *context) {
  CURLcode result;
#define MDOK_SETOPT(option, value) do { \
    result = curl_easy_setopt(easy, option, value); \
    if (result != CURLE_OK) return result; \
  } while (0)
  MDOK_SETOPT(CURLOPT_URL, plan->url);
  MDOK_SETOPT(CURLOPT_CUSTOMREQUEST, plan->method);
  MDOK_SETOPT(CURLOPT_NOBODY, strcmp(plan->method, "HEAD") == 0);
  MDOK_SETOPT(CURLOPT_HTTPHEADER, plan->headers);
  MDOK_SETOPT(CURLOPT_FOLLOWLOCATION, plan->follow);
  MDOK_SETOPT(CURLOPT_MAXREDIRS, plan->max_redirs);
  if (session != NULL && session->share != NULL) {
    MDOK_SETOPT(CURLOPT_SHARE, session->share);
    /* An empty cookie file enables the in-memory cookie engine without
       reading a filesystem path. The share handle keeps it document-scoped. */
    MDOK_SETOPT(CURLOPT_COOKIEFILE, "");
  }
  /* Explicitly disable inherited proxy environment variables.  MDOK only
     reaches this native path for plans whose policy has already rejected
     caller-selected proxies. */
  MDOK_SETOPT(CURLOPT_PROXY, "");
  MDOK_SETOPT(CURLOPT_NOPROXY, "*");
  MDOK_SETOPT(CURLOPT_SSL_VERIFYPEER, plan->insecure ? 0L : 1L);
  MDOK_SETOPT(CURLOPT_SSL_VERIFYHOST, plan->insecure ? 0L : 2L);
  MDOK_SETOPT(CURLOPT_ACCEPT_ENCODING, plan->compressed ? "" : NULL);
  MDOK_SETOPT(CURLOPT_NOSIGNAL, 1L);
  if (plan->timeout_ms > 0) MDOK_SETOPT(CURLOPT_TIMEOUT_MS, plan->timeout_ms);
  if (plan->connect_timeout_ms > 0) MDOK_SETOPT(CURLOPT_CONNECTTIMEOUT_MS, plan->connect_timeout_ms);
  if (plan->range != NULL) MDOK_SETOPT(CURLOPT_RANGE, plan->range);
  if (plan->user_agent != NULL) MDOK_SETOPT(CURLOPT_USERAGENT, plan->user_agent);
  if (plan->referer != NULL) MDOK_SETOPT(CURLOPT_REFERER, plan->referer);
  if (plan->body != NULL) {
    MDOK_SETOPT(CURLOPT_POSTFIELDS, plan->body);
    MDOK_SETOPT(CURLOPT_POSTFIELDSIZE_LARGE, (curl_off_t)plan->body_len);
  }
  if (callbacks != NULL) {
    MDOK_SETOPT(CURLOPT_WRITEFUNCTION, body_deliver);
    MDOK_SETOPT(CURLOPT_WRITEDATA, context);
    MDOK_SETOPT(CURLOPT_HEADERFUNCTION, header_deliver);
    MDOK_SETOPT(CURLOPT_HEADERDATA, context);
    MDOK_SETOPT(CURLOPT_XFERINFOFUNCTION, progress_cancel);
    MDOK_SETOPT(CURLOPT_XFERINFODATA, context);
    MDOK_SETOPT(CURLOPT_NOPROGRESS, 0L);
  }
#undef MDOK_SETOPT
  return CURLE_OK;
}

static CURLMcode perform_multi(CURLM *multi, CURL *easy, CURLcode *out_transfer, int *out_added) {
  CURLMcode multi_result;
  int running = 0;
  int numfds = 0;
  int messages = 0;
  CURLMsg *message;
  if (out_transfer != NULL) *out_transfer = CURLE_FAILED_INIT;
  if (out_added != NULL) *out_added = 0;
  if (multi == NULL || easy == NULL) return CURLM_BAD_HANDLE;
  multi_result = curl_multi_add_handle(multi, easy);
  if (multi_result != CURLM_OK) return multi_result;
  if (out_added != NULL) *out_added = 1;
  do {
    do {
      multi_result = curl_multi_perform(multi, &running);
    } while (multi_result == CURLM_CALL_MULTI_PERFORM);
    if (multi_result != CURLM_OK || !running) break;
    multi_result = curl_multi_poll(multi, NULL, 0, 100, &numfds);
  } while (multi_result == CURLM_OK);
  if (multi_result != CURLM_OK) return multi_result;
  while ((message = curl_multi_info_read(multi, &messages)) != NULL) {
    if (message->msg == CURLMSG_DONE && message->easy_handle == easy && out_transfer != NULL)
      *out_transfer = message->data.result;
  }
  return CURLM_OK;
}

mdok_curl_status mdok_curl_execute_with_info(mdok_curl_session *session, const mdok_curl_plan *plan, const mdok_curl_callbacks *callbacks, void *userdata, mdok_curl_transfer_info *out_info, mdok_curl_error *out_error) {
  CURL *easy;
  CURLcode result;
  CURLMcode multi_result = CURLM_OK;
  struct callback_context context = {callbacks, userdata, 0, 0};
  int session_acquired = 0;
  int owns_easy = 0;
  int added_to_multi = 0;
  clear_transfer_info(out_info);
  clear_error(out_error);
  if (plan == NULL || plan->url == NULL || plan->method == NULL) { set_error(out_error, 0, "invalid curl plan"); return MDOK_CURL_INTERNAL_ERROR; }
  if (session != NULL) {
    if (atomic_flag_test_and_set(&session->in_use)) {
      set_error(out_error, 0, "curl session is busy");
      return MDOK_CURL_INTERNAL_ERROR;
    }
    session_acquired = 1;
    easy = session->easy;
    if (easy == NULL) {
      atomic_flag_clear(&session->in_use);
      set_error(out_error, 0, "curl session has no easy handle");
      return MDOK_CURL_INTERNAL_ERROR;
    }
    curl_easy_reset(easy);
  } else {
    easy = curl_easy_init();
    owns_easy = 1;
  }
  if (easy == NULL) {
    if (session_acquired) atomic_flag_clear(&session->in_use);
    set_error(out_error, 0, "curl_easy_init failed");
    return MDOK_CURL_INTERNAL_ERROR;
  }
  result = configure_easy(session, easy, plan, callbacks, &context);
  if (result == CURLE_OK && session != NULL) {
    if (session->multi == NULL) {
      set_error(out_error, 0, "curl session has no multi handle");
      result = CURLE_FAILED_INIT;
    } else {
      multi_result = perform_multi(session->multi, easy, &result, &added_to_multi);
    }
  } else if (result == CURLE_OK) {
    result = curl_easy_perform(easy);
  }
  if (out_info != NULL && result != CURLE_FAILED_INIT) fill_transfer_info(easy, out_info);
  if (added_to_multi) curl_multi_remove_handle(session->multi, easy);
  if (owns_easy) curl_easy_cleanup(easy);
  if (session_acquired) atomic_flag_clear(&session->in_use);
  if (multi_result != CURLM_OK) {
    set_error(out_error, (int32_t)multi_result, curl_multi_strerror(multi_result));
    return MDOK_CURL_TRANSFER_ERROR;
  }
  if (result != CURLE_OK) {
    if (context.callback_failed) {
      set_error(out_error, (int32_t)CURLE_WRITE_ERROR, "callback did not consume the complete buffer");
    } else {
      set_error(out_error, (int32_t)result, curl_easy_strerror(result));
    }
    if (result == CURLE_ABORTED_BY_CALLBACK && context.cancelled) return MDOK_CURL_CANCELLED;
    return MDOK_CURL_TRANSFER_ERROR;
  }
  return MDOK_CURL_OK;
}

mdok_curl_status mdok_curl_execute(mdok_curl_session *session, const mdok_curl_plan *plan, const mdok_curl_callbacks *callbacks, void *userdata, mdok_curl_error *out_error) {
  return mdok_curl_execute_with_info(session, plan, callbacks, userdata, NULL, out_error);
}

void mdok_curl_plan_free(mdok_curl_plan *plan) {
  if (plan == NULL) return;
  free(plan->url); free(plan->method); free(plan->body); free(plan->range); free(plan->user_agent); free(plan->referer);
  curl_slist_free_all(plan->headers); free(plan);
}
