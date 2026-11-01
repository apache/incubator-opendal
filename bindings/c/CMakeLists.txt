# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

cmake_minimum_required(VERSION 3.24)
cmake_policy(SET CMP0135 NEW)

project(opendal-c)

if (NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Debug")
endif()

option(TEST_ENABLE_ASAN "Enable AddressSanitizer for tests" OFF)
set(FEATURES "" CACHE STRING "opendal features")

set(CARGO_FEATURES_FLAG "--features=${FEATURES}")

# force the compiler to support these standards
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# for GoogleTest, it should be no less than C++14
set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# fetch google test via GitHub
include(FetchContent)

set(GOOGLETEST_VERSION 1.15.2)
FetchContent_Declare(
  googletest
  URL https://github.com/google/googletest/archive/refs/tags/v${GOOGLETEST_VERSION}.zip
)
# For Windows: Prevent overriding the parent project's compiler/linker settings
set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(googletest)

set(CARGO_DIST_DIR "${PROJECT_SOURCE_DIR}/target/debug")
if (NOT CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(CARGO_BUILD_TYPE "--release")
    set(CARGO_DIST_DIR "${PROJECT_SOURCE_DIR}/target/release")
endif()

set(OPENDAL_STATIC_LIB "${CARGO_DIST_DIR}/libopendal_c${CMAKE_STATIC_LIBRARY_SUFFIX}")
set(OPENDAL_SHARED_LIB "${CARGO_DIST_DIR}/libopendal_c${CMAKE_SHARED_LIBRARY_SUFFIX}")
message(NOTICE "-- OpenDAL C static lib: ${OPENDAL_STATIC_LIB}")
message(NOTICE "-- OpenDAL C shared lib: ${OPENDAL_SHARED_LIB}")

# custom target for cargo build
add_custom_target(cargo_build
    COMMAND sh -c "cargo build ${CARGO_BUILD_TYPE} ${CARGO_FEATURES_FLAG}"
    BYPRODUCTS  ${OPENDAL_STATIC_LIB} ${OPENDAL_SHARED_LIB}
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}
)

# cmake target for static lib
add_library(opendal_c_static INTERFACE)
target_link_libraries(opendal_c_static INTERFACE ${OPENDAL_STATIC_LIB})
target_include_directories(opendal_c_static INTERFACE include)
add_dependencies(opendal_c_static cargo_build)

# cmake target for shared lib
add_library(opendal_c_shared INTERFACE)
target_link_libraries(opendal_c_shared INTERFACE ${OPENDAL_SHARED_LIB})
target_include_directories(opendal_c_shared INTERFACE include)
add_dependencies(opendal_c_shared cargo_build)

# example targets
add_executable(basic examples/basic.c)
target_link_libraries(basic opendal_c_shared)

add_executable(error_handle examples/error_handle.c)
target_link_libraries(error_handle opendal_c_shared)

add_executable(getting_started examples/getting-started.c)
target_link_libraries(getting_started opendal_c_shared)

# test targets
set(GTEST_SRCS
    tests/bdd.cpp
    tests/error_msg.cpp
    tests/list.cpp
    tests/opinfo.cpp
    tests/reader.cpp
)
add_executable(tests ${GTEST_SRCS})
target_link_libraries(tests PRIVATE opendal_c_shared gtest_main)
if(UNIX AND NOT APPLE)
    target_link_libraries(tests PRIVATE uuid)
endif()
if (TEST_ENABLE_ASAN)
    target_compile_options(tests PRIVATE -fsanitize=address)
    target_link_options(tests PRIVATE -fsanitize=address)
endif()
