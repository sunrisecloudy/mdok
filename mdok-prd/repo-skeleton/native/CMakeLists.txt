cmake_minimum_required(VERSION 3.24)
project(mdok_curl_bridge C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_VISIBILITY_PRESET hidden)

add_library(mdok_curl_bridge STATIC src/mdok_curl_bridge.c)
target_include_directories(mdok_curl_bridge PUBLIC include)

# Phase 0:
# 1. Verify vendor/curl matches vendor/curl.version and curl.sha256.
# 2. Apply vendor/patches/curl/*.patch.
# 3. Build libcurl and the exported curl-tool parser target.
# 4. Link both targets here with only mdok_curl.h symbols visible.
install(TARGETS mdok_curl_bridge ARCHIVE DESTINATION lib)
install(FILES include/mdok_curl.h DESTINATION include)
