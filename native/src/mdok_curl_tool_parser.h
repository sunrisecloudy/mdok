#ifndef MDOK_CURL_TOOL_PARSER_H
#define MDOK_CURL_TOOL_PARSER_H

#include "mdok_curl.h"

#include <curl/curl.h>

/* This is deliberately private to the native bridge.  The Rust API exposes
 * only the opaque mdok_curl_plan; callers cannot depend on curl's internal
 * OperationConfig layout. */
typedef struct mdok_curl_tool_result {
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
} mdok_curl_tool_result;

mdok_curl_status mdok_curl_tool_parse(
    const mdok_curl_argv *argv,
    mdok_curl_tool_result *out_result,
    mdok_curl_error *out_error);

void mdok_curl_tool_result_free(mdok_curl_tool_result *result);

#endif
