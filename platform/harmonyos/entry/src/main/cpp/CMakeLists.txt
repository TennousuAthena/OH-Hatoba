cmake_minimum_required(VERSION 3.16)
project(hatoba_ohos LANGUAGES C CXX)
set(CMAKE_CXX_STANDARD 17)
get_filename_component(HATOBA_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../../../../.." ABSOLUTE)
set(HATOBA_RUST_ARCHIVE "${HATOBA_ROOT}/target/aarch64-unknown-linux-ohos/debug/libhatoba_ohos.a" CACHE FILEPATH "Built OHOS Rust library")
if(NOT EXISTS "${HATOBA_RUST_ARCHIVE}")
  message(FATAL_ERROR "Run scripts/harmonyos/build-native.sh before building the HAP")
endif()
add_library(hatoba_rust STATIC IMPORTED)
set_target_properties(hatoba_rust PROPERTIES IMPORTED_LOCATION "${HATOBA_RUST_ARCHIVE}")
add_library(hatoba SHARED bridge.cpp)
target_compile_options(hatoba PRIVATE -Wall -Wextra -Werror -Wno-unused-command-line-argument)
target_link_options(hatoba PRIVATE -Wl,--strip-debug -Wl,--gc-sections
  "-Wl,--version-script=${CMAKE_CURRENT_SOURCE_DIR}/exports.map")
set_property(TARGET hatoba APPEND PROPERTY LINK_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/exports.map")
# chrono -> iana-time-zone uses the OHOS API12 time service. Rust staticlibs do not
# propagate this native link directive to CMake; link it here, not in upstream Rust crates.
target_link_libraries(hatoba PRIVATE hatoba_rust libace_napi.z.so libtime_service_ndk.so c m dl)
