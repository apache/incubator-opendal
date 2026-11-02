/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

#ifndef _OPENDAL_TEST_FRAMEWORK_H
#define _OPENDAL_TEST_FRAMEWORK_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <time.h>
#include <unistd.h>
#include <ctype.h>
#include <curl/curl.h>

#ifdef __cplusplus
extern "C" {
#endif

#include "opendal.h"

#ifdef __cplusplus
}
#endif

// Test framework globals
extern int total_tests;
extern int passed_tests;
extern int failed_tests;

// Test configuration structure
typedef struct opendal_test_config {
    const char* scheme;
    opendal_operator_options* options;
    const opendal_operator* operator_instance;
    char* random_root;
} opendal_test_config;

// Test context structure
typedef struct opendal_test_context {
    opendal_test_config* config;
    const char* test_name;
    const char* suite_name;
} opendal_test_context;

// Capability checking
typedef struct opendal_required_capability {
    bool stat;
    bool read;
    bool write;
    bool delete_;
    bool list;
    bool list_with_start_after;
    bool list_with_recursive;
    bool copy;
    bool rename;
    bool create_dir;
    bool presign;
    bool presign_read;
    bool presign_write;
    bool presign_stat;
    bool presign_delete;
} opendal_required_capability;

// Test function pointer type
typedef void (*opendal_test_func)(opendal_test_context* ctx);

// Test case structure
typedef struct opendal_test_case {
    const char* name;
    opendal_test_func func;
    opendal_required_capability capability;
} opendal_test_case;

// Test suite structure
typedef struct opendal_test_suite {
    const char* name;
    opendal_test_case* tests;
    size_t test_count;
} opendal_test_suite;

// Test assertion macros
#define OPENDAL_ASSERT(condition, message) \
    do { \
        if (!(condition)) { \
            printf("ASSERTION FAILED: %s:%d: %s\n", __FILE__, __LINE__, message); \
            printf("  Condition: %s\n", #condition); \
            failed_tests++; \
            return; \
        } \
    } while(0)

#define OPENDAL_ASSERT_EQ(expected, actual, message) \
    do { \
        if ((expected) != (actual)) { \
            printf("ASSERTION FAILED: %s:%d: %s\n", __FILE__, __LINE__, message); \
            printf("  Expected: %ld, Actual: %ld\n", (long)(expected), (long)(actual)); \
            failed_tests++; \
            return; \
        } \
    } while(0)

#define OPENDAL_ASSERT_STR_EQ(expected, actual, message) \
    do { \
        if (strcmp((expected), (actual)) != 0) { \
            printf("ASSERTION FAILED: %s:%d: %s\n", __FILE__, __LINE__, message); \
            printf("  Expected: '%s', Actual: '%s'\n", (expected), (actual)); \
            failed_tests++; \
            return; \
        } \
    } while(0)

#define OPENDAL_ASSERT_NULL(ptr, message) \
    do { \
        if ((ptr) != NULL) { \
            printf("ASSERTION FAILED: %s:%d: %s\n", __FILE__, __LINE__, message); \
            printf("  Expected NULL, but got non-NULL pointer\n"); \
            failed_tests++; \
            return; \
        } \
    } while(0)

#define OPENDAL_ASSERT_NOT_NULL(ptr, message) \
    do { \
        if ((ptr) == NULL) { \
            printf("ASSERTION FAILED: %s:%d: %s\n", __FILE__, __LINE__, message); \
            printf("  Expected non-NULL pointer, but got NULL\n"); \
            failed_tests++; \
            return; \
        } \
    } while(0)

#define OPENDAL_ASSERT_NO_ERROR(error_param, msg) \
    do { \
        opendal_error* _err = (error_param); \
        if (_err != NULL) { \
            printf("ASSERTION FAILED: %s:%d: %s\n", __FILE__, __LINE__, (msg)); \
            printf("  Expected no error, but got error code: %d\n", _err->code); \
            if (_err->message.data) { \
                printf("  Error message: %.*s\n", (int)_err->message.len, (char*)_err->message.data); \
            } \
            failed_tests++; \
            return; \
        } \
    } while(0)

// Utility macros
#define NO_CAPABILITY { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }

// Helper functions for capability creation (C++ compatible)
inline opendal_required_capability make_capability_read_write() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.read = true;
    cap.write = true;
    return cap;
}

inline opendal_required_capability make_capability_write() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.write = true;
    return cap;
}

inline opendal_required_capability make_capability_write_stat() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.write = true;
    cap.stat = true;
    return cap;
}

inline opendal_required_capability make_capability_write_delete() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.write = true;
    cap.delete_ = true;
    return cap;
}

inline opendal_required_capability make_capability_create_dir() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.create_dir = true;
    return cap;
}

inline opendal_required_capability make_capability_write_create_dir_list() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.write = true;
    cap.create_dir = true;
    cap.list = true;
    return cap;
}

inline opendal_required_capability make_capability_create_dir_list() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.create_dir = true;
    cap.list = true;
    return cap;
}

inline opendal_required_capability make_capability_write_create_dir_list_recursive() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.write = true;
    cap.create_dir = true;
    cap.list = true;
    cap.list_with_recursive = true;
    return cap;
}

inline opendal_required_capability make_capability_presign() {
    opendal_required_capability cap = NO_CAPABILITY;
    cap.read = true;
    cap.write = true;
    cap.stat = true;
    cap.presign = true;
    cap.presign_read = true;
    cap.presign_write = true;
    cap.presign_stat = true;
    return cap;
}

// Function declarations
opendal_test_config* opendal_test_config_new();
void opendal_test_config_free(opendal_test_config* config);
bool opendal_check_capability(const opendal_operator* op, opendal_required_capability required);
char* opendal_generate_random_path(const char* base_root);
char* opendal_generate_random_content(size_t length);
void opendal_run_test_suite(opendal_test_suite* suite, opendal_test_config* config);
void opendal_run_test_case(opendal_test_case* test_case, opendal_test_config* config, const char* suite_name);
void opendal_print_test_summary();

// Test data utilities
typedef struct opendal_test_data {
    char* path;
    opendal_bytes content;
} opendal_test_data;

opendal_test_data* opendal_test_data_new(const char* path, const char* content);
void opendal_test_data_free(opendal_test_data* data);

#endif // _OPENDAL_TEST_FRAMEWORK_H
