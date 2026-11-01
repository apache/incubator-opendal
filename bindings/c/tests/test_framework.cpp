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
#include <uuid/uuid.h>

// Global test counters
int total_tests = 0;
int passed_tests = 0;
int failed_tests = 0;

// Normalize scheme by replacing underscores with hyphens
static char* normalize_scheme(const char* scheme)
{
    if (!scheme)
        return NULL;

    char* normalized = strdup(scheme);
    if (!normalized)
        return NULL;

    for (char* p = normalized; *p; ++p) {
        if (*p == '_') {
            *p = '-';
        }
    }

    return normalized;
}

opendal_test_config* opendal_test_config_new()
{
    opendal_test_config* config = (opendal_test_config*)malloc(sizeof(opendal_test_config));
    if (!config)
        return NULL;

    // Read environment variables for configuration
    const char* scheme = getenv("OPENDAL_TEST");
    if (!scheme) {
        scheme = "memory"; // Default to memory for testing
    }

    // Normalize the scheme (replace underscores with hyphens)
    char* normalized_scheme = normalize_scheme(scheme);
    if (!normalized_scheme) {
        free(config);
        return NULL;
    }

    config->scheme = normalized_scheme;
    config->options = opendal_operator_options_new();

    // Read configuration from environment variables
    // Format: OPENDAL_{SCHEME}_{CONFIG_KEY}
    char env_prefix[256];
    snprintf(env_prefix, sizeof(env_prefix), "OPENDAL_%s_", scheme);

    // Convert to uppercase
    for (char* p = env_prefix; *p; ++p) {
        *p = toupper(*p);
    }

    // Look for environment variables with this prefix
    extern char** environ;
    for (char** env = environ; *env; ++env) {
        if (strncmp(*env, env_prefix, strlen(env_prefix)) == 0) {
            char* key_value = strdup(*env + strlen(env_prefix));
            char* equals = strchr(key_value, '=');
            if (equals) {
                *equals = '\0';
                char* key = key_value;
                char* value = equals + 1;

                // Convert key to lowercase
                for (char* p = key; *p; ++p) {
                    *p = tolower(*p);
                }

                opendal_operator_options_set(config->options, key, value);
            }
            free(key_value);
        }
    }

    // Generate random root if not disabled
    const char* disable_random_root = getenv("OPENDAL_DISABLE_RANDOM_ROOT");
    if (!disable_random_root || strcmp(disable_random_root, "true") != 0) {
        // Get existing root configuration if any
        const char* existing_root = NULL;

        // Check if root was already set from environment variables
        char root_env_var[256];
        snprintf(root_env_var, sizeof(root_env_var), "OPENDAL_%s_ROOT", scheme);
        // Convert to uppercase
        for (char* p = root_env_var; *p; ++p) {
            *p = toupper(*p);
        }
        existing_root = getenv(root_env_var);

        // Generate random path based on existing root
        config->random_root = opendal_generate_random_path(existing_root);
        opendal_operator_options_set(config->options, "root", config->random_root);
    } else {
        config->random_root = NULL;
    }

    // Create operator
    opendal_result_operator_new result = opendal_operator_new(config->scheme, config->options);
    if (result.error) {
        printf("Failed to create operator: error code %d\n", result.error->code);
        if (result.error->message.data) {
            printf("Error message: %.*s\n", (int)result.error->message.len,
                (char*)result.error->message.data);
        }
        opendal_error_free(result.error);
        opendal_test_config_free(config);
        return NULL;
    }

    config->operator_instance = result.op;
    return config;
}

void opendal_test_config_free(opendal_test_config* config)
{
    if (!config)
        return;

    if (config->operator_instance) {
        opendal_operator_free(config->operator_instance);
    }
    if (config->options) {
        opendal_operator_options_free(config->options);
    }
    if (config->scheme) {
        free((void*)config->scheme);
    }
    if (config->random_root) {
        free(config->random_root);
    }
    free(config);
}

bool opendal_check_capability(const opendal_operator* op,
    opendal_required_capability required)
{
    opendal_operator_info* info = opendal_operator_info_new(op);
    if (!info)
        return false;

    opendal_capability cap = opendal_operator_info_get_capability(info);

    bool result = true;
    if (required.stat && !cap.stat)
        result = false;
    if (required.read && !cap.read)
        result = false;
    if (required.write && !cap.write)
        result = false;
    if (required.delete_ && !cap.delete_)
        result = false;
    if (required.list && !cap.list)
        result = false;
    if (required.list_with_start_after && !cap.list_with_start_after)
        result = false;
    if (required.list_with_recursive && !cap.list_with_recursive)
        result = false;
    if (required.copy && !cap.copy)
        result = false;
    if (required.rename && !cap.rename)
        result = false;
    if (required.create_dir && !cap.create_dir)
        result = false;
    if (required.presign && !cap.presign)
        result = false;
    if (required.presign_read && !cap.presign_read)
        result = false;
    if (required.presign_write && !cap.presign_write)
        result = false;
    if (required.presign_stat && !cap.presign_stat)
        result = false;
    if (required.presign_delete && !cap.presign_delete)
        result = false;

    opendal_operator_info_free(info);
    return result;
}

char* opendal_generate_random_path(const char* base_root)
{
    uuid_t uuid;
    char uuid_str[37];

    uuid_generate(uuid);
    uuid_unparse(uuid, uuid_str);

    char* path = (char*)malloc(512); // Increase size to accommodate longer paths
    if (!path)
        return NULL;

    if (base_root && strlen(base_root) > 0) {
        // If base_root is provided, append the random UUID to it
        // Ensure proper path separator handling
        const char* separator = (base_root[strlen(base_root) - 1] == '/') ? "" : "/";
        snprintf(path, 512, "%s%stest_%s/", base_root, separator, uuid_str);
    } else {
        // If no base_root, use /tmp as a safe default instead of filesystem root
        snprintf(path, 512, "/tmp/test_%s/", uuid_str);
    }

    return path;
}

char* opendal_generate_random_content(size_t length)
{
    char* content = (char*)malloc(length + 1);
    if (!content)
        return NULL;

    srand(time(NULL));
    for (size_t i = 0; i < length; i++) {
        content[i] = 'a' + (rand() % 26);
    }
    content[length] = '\0';

    return content;
}

void opendal_run_test_suite(opendal_test_suite* suite,
    opendal_test_config* config)
{
    printf("\n=== Running Test Suite: %s ===\n", suite->name);

    for (size_t i = 0; i < suite->test_count; i++) {
        opendal_run_test_case(&suite->tests[i], config, suite->name);
    }

    printf("=== Test Suite %s Completed ===\n", suite->name);
}

void opendal_run_test_case(opendal_test_case* test_case,
    opendal_test_config* config,
    const char* suite_name)
{
    total_tests++;

    // Check capabilities
    if (!opendal_check_capability(config->operator_instance,
            test_case->capability)) {
        printf("SKIPPED: %s::%s (missing required capabilities)\n", suite_name,
            test_case->name);
        return;
    }

    printf("RUNNING: %s::%s ... ", suite_name, test_case->name);
    fflush(stdout);

    // Create test context
    opendal_test_context ctx = {
        .config = config, .test_name = test_case->name, .suite_name = suite_name
    };

    // Capture stdout to detect assertion failures
    int saved_failed_count = failed_tests;

    // Run the test
    test_case->func(&ctx);

    // Check if test passed or failed
    if (failed_tests > saved_failed_count) {
        printf("FAILED\n");
    } else {
        printf("PASSED\n");
        passed_tests++;
    }
}

void opendal_print_test_summary()
{
    printf("\n=== Test Summary ===\n");
    printf("Total tests: %d\n", total_tests);
    printf("Passed: %d\n", passed_tests);
    printf("Failed: %d\n", failed_tests);
    printf("Skipped: %d\n", total_tests - passed_tests - failed_tests);

    if (failed_tests > 0) {
        printf("\nTests FAILED!\n");
    } else {
        printf("\nAll tests PASSED!\n");
    }
}

opendal_test_data* opendal_test_data_new(const char* path,
    const char* content)
{
    opendal_test_data* data = (opendal_test_data*)malloc(sizeof(opendal_test_data));
    if (!data)
        return NULL;

    data->path = strdup(path);

    size_t content_len = strlen(content);
    data->content.data = (uint8_t*)malloc(content_len);
    data->content.len = content_len;
    data->content.capacity = content_len;
    memcpy(data->content.data, content, content_len);

    return data;
}

void opendal_test_data_free(opendal_test_data* data)
{
    if (!data)
        return;

    if (data->path) {
        free(data->path);
    }
    if (data->content.data) {
        free(data->content.data);
    }
    free(data);
}
