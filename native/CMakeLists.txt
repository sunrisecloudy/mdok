cmake_minimum_required(VERSION 3.24)
project(mdok_curl_bridge C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_VISIBILITY_PRESET hidden)

# MDOK owns the curl source used by the native bridge.  Keep the dependency
# surface deliberately small and deterministic; optional protocol stacks are
# disabled until their complete policy and fixture coverage is available.
set(BUILD_CURL_EXE OFF CACHE BOOL "" FORCE)
set(BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
set(BUILD_LIBCURL_DOCS OFF CACHE BOOL "" FORCE)
set(BUILD_MISC_DOCS OFF CACHE BOOL "" FORCE)
set(CURL_DISABLE_INSTALL ON CACHE BOOL "" FORCE)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(BUILD_STATIC_LIBS ON CACHE BOOL "" FORCE)
set(USE_NGHTTP2 OFF CACHE BOOL "" FORCE)
set(USE_NGTCP2 OFF CACHE BOOL "" FORCE)
set(USE_QUICHE OFF CACHE BOOL "" FORCE)
set(USE_LIBIDN2 OFF CACHE BOOL "" FORCE)
set(CURL_DISABLE_LDAP ON CACHE BOOL "" FORCE)
set(CURL_BROTLI OFF CACHE STRING "" FORCE)
set(CURL_ZSTD OFF CACHE STRING "" FORCE)
set(CURL_USE_LIBPSL OFF CACHE BOOL "" FORCE)
set(CURL_USE_LIBSSH2 OFF CACHE BOOL "" FORCE)
set(CURL_USE_LIBSSH OFF CACHE BOOL "" FORCE)
set(CURL_USE_GSASL OFF CACHE BOOL "" FORCE)
set(CURL_USE_GSSAPI OFF CACHE BOOL "" FORCE)
set(CMAKE_DEBUG_POSTFIX "" CACHE STRING "" FORCE)

if(NOT WIN32)
  find_package(OpenSSL REQUIRED)
  find_package(ZLIB REQUIRED)
  set(CURL_USE_OPENSSL ON CACHE BOOL "" FORCE)
else()
  set(CURL_USE_SCHANNEL ON CACHE BOOL "" FORCE)
  set(CURL_WINDOWS_SSPI ON CACHE BOOL "" FORCE)
  set(CURL_USE_OPENSSL OFF CACHE BOOL "" FORCE)
endif()

add_subdirectory(../vendor/curl vendor-curl EXCLUDE_FROM_ALL)

# Build curl's own command-line parser, but keep its process-level operation
# driver out of the MDOK binary.  The bridge supplies the narrow lifecycle
# wrapper and rejects parsed semantics it cannot represent safely.
set(MDOK_CURL_TOOL_SOURCES
  ../vendor/curl/src/config2setopts.c
  ../vendor/curl/src/slist_wc.c
  ../vendor/curl/src/terminal.c
  ../vendor/curl/src/tool_cfgable.c
  ../vendor/curl/src/tool_easysrc.c
  ../vendor/curl/src/tool_filetime.c
  ../vendor/curl/src/tool_findfile.c
  ../vendor/curl/src/tool_formparse.c
  ../vendor/curl/src/tool_getparam.c
  ../vendor/curl/src/tool_getpass.c
  ../vendor/curl/src/tool_help.c
  ../vendor/curl/src/tool_hugehelp.c
  ../vendor/curl/src/tool_helpers.c
  ../vendor/curl/src/tool_libinfo.c
  ../vendor/curl/src/tool_listhelp.c
  ../vendor/curl/src/tool_msgs.c
  ../vendor/curl/src/tool_paramhlp.c
  ../vendor/curl/src/tool_parsecfg.c
  ../vendor/curl/src/tool_setopt.c
  ../vendor/curl/src/tool_stderr.c
  ../vendor/curl/src/tool_urlglob.c
  ../vendor/curl/src/tool_util.c
  ../vendor/curl/src/tool_writeout_json.c
  ../vendor/curl/src/var.c
  ../vendor/curl/lib/curlx/base64.c
  ../vendor/curl/lib/curlx/basename.c
  ../vendor/curl/lib/curlx/dynbuf.c
  ../vendor/curl/lib/curlx/fopen.c
  ../vendor/curl/lib/curlx/multibyte.c
  ../vendor/curl/lib/curlx/nonblock.c
  ../vendor/curl/lib/curlx/strcopy.c
  ../vendor/curl/lib/curlx/strdup.c
  ../vendor/curl/lib/curlx/strerr.c
  ../vendor/curl/lib/curlx/strparse.c
  ../vendor/curl/lib/curlx/timediff.c
  ../vendor/curl/lib/curlx/timeval.c
  ../vendor/curl/lib/curlx/wait.c
  ../vendor/curl/lib/curlx/warnless.c
  src/mdok_curl_tool_parser.c
)
add_library(mdok_curl_tool_parser STATIC ${MDOK_CURL_TOOL_SOURCES})
target_include_directories(mdok_curl_tool_parser PRIVATE
  include
  ../vendor/curl/lib
  ../vendor/curl/src
  ../vendor/curl/include
  ${CMAKE_CURRENT_BINARY_DIR}/vendor-curl/lib
  ${CMAKE_CURRENT_BINARY_DIR}/vendor-curl/src
)
target_link_libraries(mdok_curl_tool_parser PRIVATE libcurl)
target_compile_definitions(mdok_curl_tool_parser PRIVATE
  CURL_STATICLIB=1
  HAVE_CONFIG_H
)
if(NOT MSVC)
  target_compile_options(mdok_curl_tool_parser PRIVATE -Wall -Wextra -Wpedantic -Werror)
else()
  target_compile_options(mdok_curl_tool_parser PRIVATE /W4 /WX)
endif()

# The Rust linker sees the bridge archive, not CMake's target graph. Install
# the pinned static libcurl and a generated list of its resolved link inputs so
# build.rs can preserve CMake's platform-specific dependency selection.
if(TARGET libcurl_static)
  install(TARGETS libcurl_static ARCHIVE DESTINATION lib)
  get_target_property(MDOK_CURL_LINK_LIBRARIES libcurl_static LINK_LIBRARIES)
  set(MDOK_CURL_LINK_FILES "")
  foreach(MDOK_CURL_LINK_LIBRARY IN LISTS MDOK_CURL_LINK_LIBRARIES)
    if(TARGET "${MDOK_CURL_LINK_LIBRARY}")
      string(APPEND MDOK_CURL_LINK_FILES
        "$<TARGET_FILE:${MDOK_CURL_LINK_LIBRARY}>\n")
    elseif(MDOK_CURL_LINK_LIBRARY MATCHES "::")
      # Imported targets created in curl's subdirectory are not always
      # visible from this directory.  The parent find_package calls above
      # make the common OpenSSL/Zlib targets visible on supported hosts; an
      # unresolved optional target is intentionally omitted here.
      continue()
    else()
      string(APPEND MDOK_CURL_LINK_FILES "${MDOK_CURL_LINK_LIBRARY}\n")
    endif()
  endforeach()
  file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/mdok-curl-link-files.txt"
       CONTENT "${MDOK_CURL_LINK_FILES}")
  install(FILES "${CMAKE_CURRENT_BINARY_DIR}/mdok-curl-link-files.txt" DESTINATION .)
endif()

add_library(mdok_curl_bridge STATIC src/mdok_curl_bridge.c)
target_include_directories(mdok_curl_bridge PUBLIC include PRIVATE
  ../vendor/curl/lib
  ../vendor/curl/src
  ../vendor/curl/include
  ${CMAKE_CURRENT_BINARY_DIR}/vendor-curl/lib
  ${CMAKE_CURRENT_BINARY_DIR}/vendor-curl/src
)
target_link_libraries(mdok_curl_bridge PRIVATE mdok_curl_tool_parser libcurl)
target_compile_definitions(mdok_curl_bridge PRIVATE CURL_STATICLIB=1 HAVE_CONFIG_H)
if(NOT MSVC)
  target_compile_options(mdok_curl_bridge PRIVATE -Wall -Wextra -Wpedantic -Werror)
else()
  target_compile_options(mdok_curl_bridge PRIVATE /W4 /WX)
endif()

install(TARGETS mdok_curl_bridge ARCHIVE DESTINATION lib)
install(TARGETS mdok_curl_tool_parser ARCHIVE DESTINATION lib)
install(FILES include/mdok_curl.h DESTINATION include)
