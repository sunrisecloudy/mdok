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

mdok_curl_status mdok_curl_global_init(void);
void mdok_curl_global_cleanup(void);

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

void mdok_curl_plan_free(mdok_curl_plan *plan);

#ifdef __cplusplus
}
#endif
#endif
