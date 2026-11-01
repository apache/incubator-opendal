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

cmake_minimum_required(VERSION 3.22)

# CMP0135: set the timestamps of all extracted contents
# to the time of the extraction in FetchContent
if (POLICY CMP0135)
    cmake_policy(SET CMP0135 NEW)
endif()

project(opendal-cpp LANGUAGES CXX)

include(FetchContent)
set(OPENDAL_GOOGLETEST_VERSION 1.15.2 CACHE STRING "version of GoogleTest, 'external' to fallback to find_package()")
set(OPENDAL_CPPCORO_VERSION a4ef65281814b18fdd1ac5457d3e219347ec6cb8 CACHE STRING "version of cppcoro")
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

if (NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Debug)
endif()

option(OPENDAL_ENABLE_ADDRESS_SANITIZER "Enable address sanitizer" OFF)
option(OPENDAL_ENABLE_DOCUMENTATION "Enable generating document for opendal" OFF)
option(OPENDAL_DOCS_ONLY "Only build documentation (dev only for quick ci)" OFF)
option(OPENDAL_ENABLE_TESTING "Enable building test binary for opendal" OFF)
option(OPENDAL_DEV "Enable dev mode" OFF)
option(OPENDAL_ENABLE_ASYNC "Enable async mode (requires C++20)" OFF)
set(OPENDAL_FEATURES "" CACHE STRING "opendal features")

if(OPENDAL_ENABLE_ASYNC)
    set(CMAKE_CXX_STANDARD 20)

    if (NOT ((CMAKE_CXX_COMPILER_ID STREQUAL "Clang") OR (CMAKE_CXX_COMPILER_ID STREQUAL "AppleClang")))
        message(FATAL_ERROR "currently C++ compiler must be clang for async mode")
    endif()
else()
    set(CMAKE_CXX_STANDARD 17)
endif()
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if (OPENDAL_DEV)
    set(OPENDAL_ENABLE_ADDRESS_SANITIZER ON)
    set(OPENDAL_ENABLE_DOCUMENTATION ON)
    set(OPENDAL_ENABLE_TESTING ON)
endif()

if (OPENDAL_ENABLE_TESTING)
    if (OPENDAL_FEATURES)
        set(OPENDAL_FEATURES "${OPENDAL_FEATURES},testing")
    else()
        set(OPENDAL_FEATURES "testing")
    endif()
endif()

# Documentation
if (OPENDAL_ENABLE_DOCUMENTATION OR OPENDAL_DOCS_ONLY)
    set(PROJECT_DOCUMENT_SOURCE ${PROJECT_SOURCE_DIR}/include ${PROJECT_SOURCE_DIR}/README.md)
    string(REPLACE ";" " " PROJECT_DOCUMENT_SOURCE "${PROJECT_DOCUMENT_SOURCE}")
    file(DOWNLOAD https://cdn.jsdelivr.net/gh/jothepro/doxygen-awesome-css@2.2.1/doxygen-awesome.min.css ${CMAKE_BINARY_DIR}/doxygen-awesome.css)
    find_package(Doxygen REQUIRED)
    set(DOXYGEN_IN ${PROJECT_SOURCE_DIR}/Doxyfile)
    set(DOXYGEN_OUT ${CMAKE_BINARY_DIR}/Doxyfile.out)
    configure_file(${DOXYGEN_IN} ${DOXYGEN_OUT} @ONLY)
    add_custom_target(docs
        COMMAND ${DOXYGEN_EXECUTABLE} ${DOXYGEN_OUT}
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
        COMMENT "Generating API documentation with Doxygen"
        VERBATIM)

    if (OPENDAL_DOCS_ONLY)
        return()
    endif()
endif()

# get cargo target dir using cargo locate-project
# we should this because the target dir is different for development and release
execute_process(COMMAND cargo locate-project --workspace --message-format plain
    OUTPUT_VARIABLE CARGO_TARGET_DIR
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR})
string(REGEX REPLACE "/Cargo.toml\n$" "/target" CARGO_TARGET_DIR "${CARGO_TARGET_DIR}")
set(CARGO_MANIFEST ${PROJECT_SOURCE_DIR}/Cargo.toml)
set(RUST_SOURCE_FILE ${PROJECT_SOURCE_DIR}/src/lib.rs)
list(APPEND RUST_BRIDGE_CPP ${CARGO_TARGET_DIR}/cxxbridge/opendal-cpp/src/lib.rs.cc)
list(APPEND RUST_HEADER_FILE ${CARGO_TARGET_DIR}/cxxbridge/opendal-cpp/src/lib.rs.h)
if (OPENDAL_ENABLE_ASYNC)
    list(APPEND RUST_BRIDGE_CPP ${CARGO_TARGET_DIR}/cxxbridge/opendal-cpp/src/async.rs.cc)
    list(APPEND RUST_HEADER_FILE ${CARGO_TARGET_DIR}/cxxbridge/opendal-cpp/src/async.rs.h)
endif()
if (CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(RUST_LIB ${CARGO_TARGET_DIR}/debug/${CMAKE_STATIC_LIBRARY_PREFIX}opendal_cpp${CMAKE_STATIC_LIBRARY_SUFFIX})
else()
    set(RUST_LIB ${CARGO_TARGET_DIR}/release/${CMAKE_STATIC_LIBRARY_PREFIX}opendal_cpp${CMAKE_STATIC_LIBRARY_SUFFIX})
endif()

set(CPP_INCLUDE_DIR ${PROJECT_SOURCE_DIR}/include
                    ${PROJECT_SOURCE_DIR}/src
                    ${CARGO_TARGET_DIR}/cxxbridge
                    ${CARGO_TARGET_DIR}/cxxbridge/opendal-cpp/src)
file(GLOB CPP_SOURCE_FILE
    "src/*.cpp"
    "src/utils/*.cpp"
)
file(GLOB CPP_HEADER_FILE
    "include/*.hpp"
    "src/*.hpp"
    "src/utils/*.hpp"
)
if (NOT OPENDAL_ENABLE_ASYNC)
    file (GLOB ASYNC_SOURCE_FILE "src/*async*.cpp")
    list(REMOVE_ITEM CPP_SOURCE_FILE ${ASYNC_SOURCE_FILE})

    file(GLOB ASYNC_HEADER_FILE "include/*async*.hpp")
    list(REMOVE_ITEM CPP_HEADER_FILE ${ASYNC_HEADER_FILE})
endif()

if (NOT CMAKE_BUILD_TYPE STREQUAL "Debug")
    list(APPEND CARGO_BUILD_FLAGS "--release")
endif()

if (OPENDAL_ENABLE_ASYNC)
    if (OPENDAL_FEATURES)
        set(OPENDAL_FEATURES "${OPENDAL_FEATURES},async")
    else()
        set(OPENDAL_FEATURES "async")
    endif ()
endif()

if (OPENDAL_FEATURES)
    list(APPEND CARGO_BUILD_FLAGS "--features" "${OPENDAL_FEATURES}")
endif ()

add_custom_target(cargo_build
        COMMAND cargo build --manifest-path ${CARGO_MANIFEST} ${CARGO_BUILD_FLAGS}
        BYPRODUCTS ${RUST_BRIDGE_CPP} ${RUST_LIB} ${RUST_HEADER_FILE}
        DEPENDS ${RUST_SOURCE_FILE}
        USES_TERMINAL
        COMMENT "Running cargo..."
)

add_library(opendal_cpp STATIC ${CPP_SOURCE_FILE} ${RUST_BRIDGE_CPP})
target_sources(opendal_cpp PUBLIC ${CPP_HEADER_FILE})
target_sources(opendal_cpp PRIVATE ${RUST_HEADER_FILE})
target_include_directories(opendal_cpp PUBLIC ${CPP_INCLUDE_DIR})
if (OPENDAL_ENABLE_ASYNC)
    target_include_directories(opendal_cpp PUBLIC ${CARGO_TARGET_DIR}/cxxbridge)
    target_compile_options(opendal_cpp PUBLIC -include ${PROJECT_SOURCE_DIR}/include/async_defs.hpp)
endif()
target_link_libraries(opendal_cpp PUBLIC ${RUST_LIB})
target_link_libraries(opendal_cpp PRIVATE ${CMAKE_DL_LIBS})

set_target_properties(opendal_cpp
        PROPERTIES ADDITIONAL_CLEAN_FILES ${CARGO_TARGET_DIR}
)
add_dependencies(opendal_cpp cargo_build)

if (OPENDAL_ENABLE_ADDRESS_SANITIZER)
    target_compile_options(opendal_cpp PRIVATE -fsanitize=leak,address,undefined -fno-omit-frame-pointer -fno-common -O1)
    target_link_options(opendal_cpp PRIVATE -fsanitize=leak,address,undefined)
endif()

# Platform-specific test configuration
if(WIN32)
    target_link_libraries(opendal_cpp userenv ws2_32 bcrypt)
    set_target_properties(
            opendal_cpp
            PROPERTIES
            MSVC_RUNTIME_LIBRARY "MultiThreadedDLL"
            RUNTIME_OUTPUT_DIRECTORY_DEBUG ${CMAKE_BINARY_DIR}
            RUNTIME_OUTPUT_DIRECTORY_RELEASE ${CMAKE_BINARY_DIR}
    )
endif()

# Tests
if (OPENDAL_ENABLE_TESTING)
    enable_testing()

    if(OPENDAL_GOOGLETEST_VERSION STREQUAL "external")
        find_package(GTest REQUIRED)
    else()
        # fetch GoogleTest
        FetchContent_Declare(
            googletest
            URL https://github.com/google/googletest/archive/refs/tags/v${OPENDAL_GOOGLETEST_VERSION}.zip
        )
        # For Windows: Prevent overriding the parent project's compiler/linker settings
        set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
        FetchContent_MakeAvailable(googletest)
    endif()

    if (OPENDAL_ENABLE_ASYNC)
        FetchContent_Declare(
            cppcoro
            URL https://github.com/andreasbuhr/cppcoro/archive/${OPENDAL_CPPCORO_VERSION}.zip
        )
        FetchContent_MakeAvailable(cppcoro)
        target_compile_options(cppcoro PRIVATE
            "-Wno-deprecated-this-capture"
        )
    endif()

    list(APPEND TEST_SOURCE_FILE tests/basic_test.cpp tests/layer_test.cpp tests/metadata_test.cpp tests/test_main.cpp)
    if (OPENDAL_ENABLE_ASYNC)
        list(APPEND TEST_SOURCE_FILE tests/async_test.cpp)
    endif()

    # Behavior tests
    list(APPEND TEST_SOURCE_FILE tests/behavior/read_test.cpp)
    list(APPEND TEST_SOURCE_FILE tests/behavior/write_test.cpp)
    list(APPEND TEST_SOURCE_FILE tests/behavior/delete_test.cpp)
    list(APPEND TEST_SOURCE_FILE tests/behavior/list_test.cpp)

    # Async behavior tests (only if async is enabled)
    if (OPENDAL_ENABLE_ASYNC)
        list(APPEND TEST_SOURCE_FILE tests/behavior/async_read_test.cpp)
        list(APPEND TEST_SOURCE_FILE tests/behavior/async_write_test.cpp)
        list(APPEND TEST_SOURCE_FILE tests/behavior/async_list_test.cpp)
    endif()

    add_executable(opendal_cpp_test ${TEST_SOURCE_FILE})
    target_include_directories(opendal_cpp_test PUBLIC ${CPP_INCLUDE_DIR} ${GTEST_INCLUDE_DIRS} tests)
    target_link_libraries(opendal_cpp_test ${GTEST_LDFLAGS} GTest::gtest GTest::gmock opendal_cpp)
    target_compile_options(opendal_cpp_test PRIVATE ${GTEST_CFLAGS})
    if (OPENDAL_ENABLE_ASYNC)
        target_link_libraries(opendal_cpp_test cppcoro)
    endif()

    # enable address sanitizers
    if (OPENDAL_ENABLE_ADDRESS_SANITIZER)
        target_compile_options(opendal_cpp_test PRIVATE -fsanitize=leak,address,undefined -fno-omit-frame-pointer -fno-common -O1)
        target_link_options(opendal_cpp_test PRIVATE -fsanitize=leak,address,undefined)
    endif()

    # Platform-specific test configuration
    if(WIN32)
        target_link_libraries(opendal_cpp_test userenv ws2_32 bcrypt)
    endif()
    if(APPLE)
        target_link_libraries(opendal_cpp_test "-framework CoreFoundation -framework Security")
    endif()

    include(GoogleTest)
    gtest_discover_tests(opendal_cpp_test)
endif()
