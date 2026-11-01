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

#include "test_framework.h"
#include <set>
#include <string>
#include <unordered_set>

// Test: Basic list operation
void test_list_basic(opendal_test_context* ctx)
{
    const char* dir_path = "test_list_dir/";

    // Create directory
    opendal_error* error = opendal_operator_create_dir(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(error, "Create dir operation should succeed");

    // Create some test files
    const char* test_files[] = { "test_list_dir/file1.txt",
        "test_list_dir/file2.txt",
        "test_list_dir/file3.txt" };
    const size_t num_files = sizeof(test_files) / sizeof(test_files[0]);

    for (size_t i = 0; i < num_files; i++) {
        opendal_bytes data;
        data.data = (uint8_t*)"test content";
        data.len = 12;
        data.capacity = 12;
        error = opendal_operator_write(ctx->config->operator_instance,
            test_files[i], &data);
        OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");
    }

    // List directory
    opendal_result_list list_result = opendal_operator_list(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(list_result.error, "List operation should succeed");
    OPENDAL_ASSERT_NOT_NULL(list_result.lister, "Lister should not be null");

    // Collect all entries
    std::set<std::string> found_paths;
    while (true) {
        opendal_result_lister_next next_result = opendal_lister_next(list_result.lister);
        if (next_result.error) {
            OPENDAL_ASSERT_NO_ERROR(next_result.error, "Lister next should not fail");
            break;
        }

        if (!next_result.entry) {
            // End of list
            break;
        }

        char* path = opendal_entry_path(next_result.entry);
        OPENDAL_ASSERT_NOT_NULL(path, "Entry path should not be null");
        found_paths.insert(std::string(path));

        opendal_string_free(path);
        opendal_entry_free(next_result.entry);
    }

    // Check if the directory itself is included in the listing
    bool dir_included = found_paths.count(dir_path) > 0;

    // Verify we found all files, and optionally the directory itself
    size_t expected_count = num_files + (dir_included ? 1 : 0);
    OPENDAL_ASSERT_EQ(expected_count, found_paths.size(),
        "Should find all files and optionally the directory");

    // All files should be present
    for (size_t i = 0; i < num_files; i++) {
        OPENDAL_ASSERT(found_paths.count(test_files[i]) > 0,
            "Should find all test files");
    }

    // Cleanup
    opendal_lister_free(list_result.lister);
    for (size_t i = 0; i < num_files; i++) {
        opendal_operator_delete(ctx->config->operator_instance, test_files[i]);
    }
    opendal_operator_delete(ctx->config->operator_instance, dir_path);
}

// Test: List empty directory
void test_list_empty_dir(opendal_test_context* ctx)
{
    const char* dir_path = "test_empty_dir/";

    // Create directory
    opendal_error* error = opendal_operator_create_dir(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(error, "Create dir operation should succeed");

    // List directory
    opendal_result_list list_result = opendal_operator_list(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(list_result.error, "List operation should succeed");
    OPENDAL_ASSERT_NOT_NULL(list_result.lister, "Lister should not be null");

    // Collect entries
    std::set<std::string> found_paths;
    while (true) {
        opendal_result_lister_next next_result = opendal_lister_next(list_result.lister);
        if (next_result.error) {
            OPENDAL_ASSERT_NO_ERROR(next_result.error, "Lister next should not fail");
            break;
        }

        if (!next_result.entry) {
            break;
        }

        char* path = opendal_entry_path(next_result.entry);
        found_paths.insert(std::string(path));
        opendal_string_free(path);
        opendal_entry_free(next_result.entry);
    }

    // Some services include the directory itself, others don't
    bool dir_included = found_paths.count(dir_path) > 0;
    size_t expected_count = dir_included ? 1 : 0;
    OPENDAL_ASSERT_EQ(expected_count, found_paths.size(),
        "Should find empty listing or just the directory");

    if (dir_included) {
        OPENDAL_ASSERT(found_paths.count(dir_path) > 0,
            "Should find the directory itself if it's included");
    }

    // Cleanup
    opendal_lister_free(list_result.lister);
    opendal_operator_delete(ctx->config->operator_instance, dir_path);
}

// Test: List nested directories
void test_list_nested(opendal_test_context* ctx)
{
    const char* base_dir = "test_nested/";
    const char* sub_dir = "test_nested/subdir/";
    const char* file_in_base = "test_nested/base_file.txt";
    const char* file_in_sub = "test_nested/subdir/sub_file.txt";

    // Create directories
    opendal_error* error = opendal_operator_create_dir(ctx->config->operator_instance, base_dir);
    OPENDAL_ASSERT_NO_ERROR(error, "Create base dir should succeed");

    error = opendal_operator_create_dir(ctx->config->operator_instance, sub_dir);
    OPENDAL_ASSERT_NO_ERROR(error, "Create sub dir should succeed");

    // Create files
    opendal_bytes data;
    data.data = (uint8_t*)"test content";
    data.len = 12;
    data.capacity = 12;

    error = opendal_operator_write(ctx->config->operator_instance, file_in_base,
        &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write to base dir should succeed");

    error = opendal_operator_write(ctx->config->operator_instance, file_in_sub,
        &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write to sub dir should succeed");

    // List base directory
    opendal_result_list list_result = opendal_operator_list(ctx->config->operator_instance, base_dir);
    OPENDAL_ASSERT_NO_ERROR(list_result.error, "List operation should succeed");

    std::set<std::string> found_paths;
    while (true) {
        opendal_result_lister_next next_result = opendal_lister_next(list_result.lister);
        if (next_result.error) {
            OPENDAL_ASSERT_NO_ERROR(next_result.error, "Lister next should not fail");
            break;
        }

        if (!next_result.entry) {
            break;
        }

        char* path = opendal_entry_path(next_result.entry);
        found_paths.insert(std::string(path));
        opendal_string_free(path);
        opendal_entry_free(next_result.entry);
    }

    // Should find base dir, sub dir, and file in base
    bool base_dir_included = found_paths.count(base_dir) > 0;
    size_t expected_count = 2 + (base_dir_included ? 1 : 0); // sub_dir + file_in_base + optionally base_dir
    OPENDAL_ASSERT_EQ(expected_count, found_paths.size(),
        "Should find correct number of items in base directory");

    // These should always be present
    OPENDAL_ASSERT(found_paths.count(sub_dir) > 0, "Should find sub directory");
    OPENDAL_ASSERT(found_paths.count(file_in_base) > 0,
        "Should find file in base directory");

    // Base directory may or may not be included depending on the service
    if (base_dir_included) {
        OPENDAL_ASSERT(found_paths.count(base_dir) > 0,
            "Should find base directory if it's included");
    }

    // Should NOT find file in subdirectory when listing base directory
    // non-recursively
    OPENDAL_ASSERT(found_paths.count(file_in_sub) == 0,
        "Should not find file in subdirectory");

    // Cleanup
    opendal_lister_free(list_result.lister);
    opendal_operator_delete(ctx->config->operator_instance, file_in_sub);
    opendal_operator_delete(ctx->config->operator_instance, file_in_base);
    opendal_operator_delete(ctx->config->operator_instance, sub_dir);
    opendal_operator_delete(ctx->config->operator_instance, base_dir);
}

// Test: Entry name vs path
void test_entry_name_path(opendal_test_context* ctx)
{
    const char* dir_path = "test_entry_names/";
    const char* file_path = "test_entry_names/test_file.txt";

    // Create directory and file
    opendal_error* error = opendal_operator_create_dir(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(error, "Create dir should succeed");

    opendal_bytes data;
    data.data = (uint8_t*)"test";
    data.len = 4;
    data.capacity = 4;
    error = opendal_operator_write(ctx->config->operator_instance, file_path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write should succeed");

    // List directory
    opendal_result_list list_result = opendal_operator_list(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(list_result.error, "List operation should succeed");

    bool found_file = false;
    while (true) {
        opendal_result_lister_next next_result = opendal_lister_next(list_result.lister);
        if (next_result.error) {
            OPENDAL_ASSERT_NO_ERROR(next_result.error, "Lister next should not fail");
            break;
        }

        if (!next_result.entry) {
            break;
        }

        char* path = opendal_entry_path(next_result.entry);
        char* name = opendal_entry_name(next_result.entry);

        if (strcmp(path, file_path) == 0) {
            found_file = true;
            OPENDAL_ASSERT_STR_EQ("test_file.txt", name,
                "Entry name should be just the filename");
            OPENDAL_ASSERT_STR_EQ(file_path, path,
                "Entry path should be the full path");
        }

        opendal_string_free(path);
        opendal_string_free(name);
        opendal_entry_free(next_result.entry);
    }

    OPENDAL_ASSERT(found_file, "Should have found the test file");

    // Cleanup
    opendal_lister_free(list_result.lister);
    opendal_operator_delete(ctx->config->operator_instance, file_path);
    opendal_operator_delete(ctx->config->operator_instance, dir_path);
}

// Test: Entry metadata from lister
void test_entry_metadata(opendal_test_context* ctx)
{
    const char* dir_path = "test_entry_meta/";
    const char* file_path = "test_entry_meta/file.txt";
    const char* content_str = "hello, metadata!";
    const size_t content_len = strlen(content_str);

    // Create directory and file
    opendal_error* error = opendal_operator_create_dir(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(error, "Create dir should succeed");

    opendal_bytes data;
    data.data = (uint8_t*)content_str;
    data.len = content_len;
    data.capacity = content_len;
    error = opendal_operator_write(ctx->config->operator_instance, file_path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write should succeed");

    // List directory
    opendal_result_list list_result = opendal_operator_list(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(list_result.error, "List operation should succeed");

    bool found_file = false;
    bool found_dir = false;
    while (true) {
        opendal_result_lister_next next_result = opendal_lister_next(list_result.lister);
        if (next_result.error) {
            OPENDAL_ASSERT_NO_ERROR(next_result.error, "Lister next should not fail");
            break;
        }

        if (!next_result.entry) {
            break;
        }

        char* path = opendal_entry_path(next_result.entry);
        opendal_metadata* meta = opendal_entry_metadata(next_result.entry);
        OPENDAL_ASSERT_NOT_NULL(meta, "Entry metadata should not be null");

        if (strcmp(path, file_path) == 0) {
            found_file = true;
            OPENDAL_ASSERT(opendal_metadata_is_file(meta), "File entry should be a file");
            OPENDAL_ASSERT(!opendal_metadata_is_dir(meta), "File entry should not be a dir");
            OPENDAL_ASSERT_EQ((long)content_len, (long)opendal_metadata_content_length(meta),
                "Content length should match written data");
        } else if (strcmp(path, dir_path) == 0) {
            found_dir = true;
            OPENDAL_ASSERT(opendal_metadata_is_dir(meta), "Dir entry should be a dir");
            OPENDAL_ASSERT(!opendal_metadata_is_file(meta), "Dir entry should not be a file");
        }

        opendal_metadata_free(meta);
        opendal_string_free(path);
        opendal_entry_free(next_result.entry);
    }

    OPENDAL_ASSERT(found_file, "Should have found the test file");

    // Cleanup
    opendal_lister_free(list_result.lister);
    opendal_operator_delete(ctx->config->operator_instance, file_path);
    opendal_operator_delete(ctx->config->operator_instance, dir_path);
}

// Test: list_with default options (null opts behaves like list)
void test_list_with_default_options(opendal_test_context* ctx)
{
    const char* dir_path = "test_list_with_default/";
    const char* file_path = "test_list_with_default/file.txt";

    opendal_error* error = opendal_operator_create_dir(ctx->config->operator_instance, dir_path);
    OPENDAL_ASSERT_NO_ERROR(error, "Create dir should succeed");

    opendal_bytes data;
    data.data = (uint8_t*)"content";
    data.len = 7;
    data.capacity = 7;
    error = opendal_operator_write(ctx->config->operator_instance, file_path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write should succeed");

    // Pass NULL opts — should behave identically to opendal_operator_list
    opendal_result_list list_result = opendal_operator_list_with(
        ctx->config->operator_instance, dir_path, NULL);
    OPENDAL_ASSERT_NO_ERROR(list_result.error, "list_with(NULL opts) should succeed");
    OPENDAL_ASSERT_NOT_NULL(list_result.lister, "Lister should not be null");

    bool found_file = false;
    while (true) {
        opendal_result_lister_next next = opendal_lister_next(list_result.lister);
        if (next.error) {
            OPENDAL_ASSERT_NO_ERROR(next.error, "lister_next should not fail");
            break;
        }
        if (!next.entry) break;

        char* path = opendal_entry_path(next.entry);
        if (strcmp(path, file_path) == 0) {
            found_file = true;
        }
        opendal_string_free(path);
        opendal_entry_free(next.entry);
    }

    OPENDAL_ASSERT(found_file, "Should find the file with null opts");

    opendal_lister_free(list_result.lister);
    opendal_operator_delete(ctx->config->operator_instance, file_path);
    opendal_operator_delete(ctx->config->operator_instance, dir_path);
}

// Test: list_with recursive=true returns entries in nested directories
void test_list_with_recursive(opendal_test_context* ctx)
{
    const char* base_dir = "test_list_recursive/";
    const char* sub_dir = "test_list_recursive/sub/";
    const char* deep_dir = "test_list_recursive/sub/deep/";
    const char* file_top = "test_list_recursive/top.txt";
    const char* file_sub = "test_list_recursive/sub/mid.txt";
    const char* file_deep = "test_list_recursive/sub/deep/bottom.txt";

    opendal_bytes data;
    data.data = (uint8_t*)"x";
    data.len = 1;
    data.capacity = 1;

    opendal_error* error;
    error = opendal_operator_create_dir(ctx->config->operator_instance, base_dir);
    OPENDAL_ASSERT_NO_ERROR(error, "Create base dir should succeed");
    error = opendal_operator_create_dir(ctx->config->operator_instance, sub_dir);
    OPENDAL_ASSERT_NO_ERROR(error, "Create sub dir should succeed");
    error = opendal_operator_create_dir(ctx->config->operator_instance, deep_dir);
    OPENDAL_ASSERT_NO_ERROR(error, "Create deep dir should succeed");
    error = opendal_operator_write(ctx->config->operator_instance, file_top, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write top file should succeed");
    error = opendal_operator_write(ctx->config->operator_instance, file_sub, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write sub file should succeed");
    error = opendal_operator_write(ctx->config->operator_instance, file_deep, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write deep file should succeed");

    // List recursively from base_dir
    opendal_list_options* opts = opendal_list_options_new();
    OPENDAL_ASSERT_NOT_NULL(opts, "list_options_new should not return NULL");
    opendal_list_options_set_recursive(opts, true);

    opendal_result_list list_result = opendal_operator_list_with(
        ctx->config->operator_instance, base_dir, opts);
    opendal_list_options_free(opts);

    OPENDAL_ASSERT_NO_ERROR(list_result.error, "Recursive list should succeed");
    OPENDAL_ASSERT_NOT_NULL(list_result.lister, "Lister should not be null");

    std::unordered_set<std::string> found_paths;
    while (true) {
        opendal_result_lister_next next = opendal_lister_next(list_result.lister);
        if (next.error) {
            OPENDAL_ASSERT_NO_ERROR(next.error, "lister_next should not fail");
            break;
        }
        if (!next.entry) break;

        char* path = opendal_entry_path(next.entry);
        found_paths.insert(std::string(path));
        opendal_string_free(path);
        opendal_entry_free(next.entry);
    }
    opendal_lister_free(list_result.lister);

    // All three files must appear in a recursive listing
    OPENDAL_ASSERT(found_paths.count(file_top) > 0,
        "Recursive list must include top-level file");
    OPENDAL_ASSERT(found_paths.count(file_sub) > 0,
        "Recursive list must include file in sub directory");
    OPENDAL_ASSERT(found_paths.count(file_deep) > 0,
        "Recursive list must include file in deep directory");

    // Cleanup
    opendal_operator_delete(ctx->config->operator_instance, file_deep);
    opendal_operator_delete(ctx->config->operator_instance, file_sub);
    opendal_operator_delete(ctx->config->operator_instance, file_top);
    opendal_operator_delete(ctx->config->operator_instance, deep_dir);
    opendal_operator_delete(ctx->config->operator_instance, sub_dir);
    opendal_operator_delete(ctx->config->operator_instance, base_dir);
}

// Define the list test suite
opendal_test_case list_tests[] = {
    { "list_basic", test_list_basic, make_capability_write_create_dir_list() },
    { "list_empty_dir", test_list_empty_dir, make_capability_create_dir_list() },
    { "list_nested", test_list_nested, make_capability_write_create_dir_list() },
    { "entry_name_path", test_entry_name_path,
        make_capability_write_create_dir_list() },
    { "entry_metadata", test_entry_metadata,
        make_capability_write_create_dir_list() },
    { "list_with_default_options", test_list_with_default_options,
        make_capability_write_create_dir_list() },
    { "list_with_recursive", test_list_with_recursive,
        make_capability_write_create_dir_list_recursive() },
};

opendal_test_suite list_suite = {
    "List Operations", // name
    list_tests, // tests
    sizeof(list_tests) / sizeof(list_tests[0]) // test_count
};
