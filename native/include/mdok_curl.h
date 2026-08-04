#ifndef MDOK_CURL_H
#define MDOK_CURL_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum mdok_curl_status {
  MDOK_CURL_OK = 0,
  MDOK_CURL_PARSE_ERROR = 1,
  MDOK_CURL_POLICY_ERROR = 2,
  MDOK_CURL_TRANSFER_ERROR = 3,
  MDOK_CURL_CANCELLED = 4,
  MDOK_CURL_INTERNAL_ERROR = 5
} mdok_curl_status;

typedef struct mdok_curl_slice {
  const uint8_t *ptr;
  size_t len;
} mdok_curl_slice;

typedef struct mdok_curl_argv {
  size_t argc;
  const mdok_curl_slice *argv;
} mdok_curl_argv;

typedef struct mdok_curl_plan mdok_curl_plan;
typedef struct mdok_curl_session mdok_curl_session;

typedef struct mdok_curl_error {
  int32_t code;
  size_t argv_index;
  mdok_curl_slice message;
} mdok_curl_error;

typedef size_t (*mdok_curl_write_cb)(const uint8_t *data, size_t len, void *userdata);
typedef int (*mdok_curl_cancel_cb)(void *userdata);

typedef struct mdok_curl_callbacks {
  mdok_curl_write_cb body;
  mdok_curl_write_cb header;
  mdok_curl_cancel_cb cancelled;
} mdok_curl_callbacks;

/* Transfer metadata is borrowed from libcurl and remains valid until the
 * next execution on the same session. Callers must copy any string slices
 * before issuing another bridge call. Numeric values use -1 for unavailable
 * values; byte counts and redirect counts use zero when unavailable. */
typedef struct mdok_curl_transfer_info {
  int64_t response_code;
  int64_t http_version;
  int64_t total_time_us;
  int64_t name_lookup_time_us;
  int64_t connect_time_us;
  int64_t appconnect_time_us;
  int64_t pretransfer_time_us;
  int64_t starttransfer_time_us;
  int64_t redirect_time_us;
  int64_t uploaded_bytes;
  int64_t downloaded_bytes;
  int64_t request_header_bytes;
  int64_t response_header_bytes;
  int64_t redirect_count;
  int64_t num_connects;
  int64_t ssl_verify_result;
  int64_t used_proxy;
  int64_t primary_port;
  int64_t local_port;
  mdok_curl_slice effective_url;
  mdok_curl_slice primary_ip;
  mdok_curl_slice local_ip;
  mdok_curl_slice http_version_name;
} mdok_curl_transfer_info;

mdok_curl_status mdok_curl_global_init(void);
void mdok_curl_global_cleanup(void);
const char *mdok_curl_last_error_message(void);
void mdok_curl_reserved(void *userdata);

/* Additive lifecycle helpers for callers that want to reuse one easy handle.
 * Existing callers may continue to pass NULL to mdok_curl_execute. A session
 * must not be used concurrently or after mdok_curl_global_cleanup. */
mdok_curl_session *mdok_curl_session_new(void);
void mdok_curl_session_free(mdok_curl_session *session);

mdok_curl_status mdok_curl_parse(
  const mdok_curl_argv *argv,
  mdok_curl_plan **out_plan,
  mdok_curl_error *out_error);

mdok_curl_status mdok_curl_execute(
  mdok_curl_session *session,
  const mdok_curl_plan *plan,
  const mdok_curl_callbacks *callbacks,
  void *userdata,
  mdok_curl_error *out_error);

/* Additive metadata variant. The existing execute symbol remains available
 * for callers that do not need transfer information. */
mdok_curl_status mdok_curl_execute_with_info(
  mdok_curl_session *session,
  const mdok_curl_plan *plan,
  const mdok_curl_callbacks *callbacks,
  void *userdata,
  mdok_curl_transfer_info *out_info,
  mdok_curl_error *out_error);

void mdok_curl_plan_free(mdok_curl_plan *plan);

#ifdef __cplusplus
}
#endif
#endif
