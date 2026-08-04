#include "mdok_curl.h"

/*
 * This is intentionally a compile-time skeleton. The Phase 0 spike must patch
 * the pinned curl tool parser to return errors instead of exiting and expose a
 * plan that can be executed through libcurl. Do not replace it with an ad-hoc
 * curl option parser.
 */

mdok_curl_status mdok_curl_global_init(void) {
  return MDOK_CURL_INTERNAL_ERROR;
}

void mdok_curl_global_cleanup(void) {}

mdok_curl_status mdok_curl_parse(
  const mdok_curl_argv *argv,
  mdok_curl_plan **out_plan,
  mdok_curl_error *out_error) {
  (void)argv; (void)out_plan; (void)out_error;
  return MDOK_CURL_INTERNAL_ERROR;
}

mdok_curl_status mdok_curl_execute(
  mdok_curl_session *session,
  const mdok_curl_plan *plan,
  const mdok_curl_callbacks *callbacks,
  void *userdata,
  mdok_curl_error *out_error) {
  (void)session; (void)plan; (void)callbacks; (void)userdata; (void)out_error;
  return MDOK_CURL_INTERNAL_ERROR;
}

void mdok_curl_plan_free(mdok_curl_plan *plan) { (void)plan; }
