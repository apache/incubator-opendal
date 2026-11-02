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
#include <string>
#include <vector>

// External test suite declarations
extern opendal_test_suite basic_suite;
extern opendal_test_suite list_suite;
extern opendal_test_suite presign_suite;
extern opendal_test_suite reader_writer_suite;

// List of all test suites
static opendal_test_suite* all_suites[] = {
    &basic_suite,
    &list_suite,
    &presign_suite,
    &reader_writer_suite,
};

static const size_t num_suites = sizeof(all_suites) / sizeof(all_suites[0]);

void print_usage(const char* program_name)
{
    printf("Usage: %s [options]\n", program_name);
    printf("\nOptions:\n");
    printf("  -h, --help                Show this help message\n");
    printf("  --list-suites            List all available test suites\n");
    printf("  --suite <name>           Run only the specified test suite\n");
    printf("  --test <suite::test>     Run only the specified test case\n");
    printf("  --verbose                Enable verbose output\n");
    printf("\nEnvironment Variables:\n");
    printf("  OPENDAL_TEST                     Service to test (default: memory)\n");
    printf("  OPENDAL_DISABLE_RANDOM_ROOT      Set to 'true' to disable random root generation\n");
    printf("  OPENDAL_<SERVICE>_<KEY>          Service-specific configuration\n");
    printf("\nExamples:\n");
    printf("  %s                               # Run all tests with memory service\n", program_name);
    printf("  %s --suite \"Basic Operations\"    # Run only basic operations tests\n", program_name);
    printf("  %s --test \"Basic Operations::check\"  # Run only the check test\n", program_name);
    printf("  OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp %s  # Test with filesystem service\n", program_name);
}

void list_suites()
{
    printf("Available test suites:\n");
    for (size_t i = 0; i < num_suites; i++) {
        printf("  %s (%zu tests)\n", all_suites[i]->name, all_suites[i]->test_count);
        for (size_t j = 0; j < all_suites[i]->test_count; j++) {
            printf("    - %s::%s\n", all_suites[i]->name, all_suites[i]->tests[j].name);
        }
    }
}

bool run_specific_test(opendal_test_config* config, const char* suite_name, const char* test_name)
{
    for (size_t i = 0; i < num_suites; i++) {
        opendal_test_suite* suite = all_suites[i];
        if (strcmp(suite->name, suite_name) == 0) {
            for (size_t j = 0; j < suite->test_count; j++) {
                if (strcmp(suite->tests[j].name, test_name) == 0) {
                    printf("Running specific test: %s::%s\n", suite_name, test_name);
                    opendal_run_test_case(&suite->tests[j], config, suite_name);
                    return true;
                }
            }
            printf("Test '%s' not found in suite '%s'\n", test_name, suite_name);
            return false;
        }
    }
    printf("Suite '%s' not found\n", suite_name);
    return false;
}

bool run_specific_suite(opendal_test_config* config, const char* suite_name)
{
    for (size_t i = 0; i < num_suites; i++) {
        if (strcmp(all_suites[i]->name, suite_name) == 0) {
            opendal_run_test_suite(all_suites[i], config);
            return true;
        }
    }
    printf("Suite '%s' not found\n", suite_name);
    return false;
}

void run_all_suites(opendal_test_config* config)
{
    printf("Running all test suites...\n");

    for (size_t i = 0; i < num_suites; i++) {
        opendal_run_test_suite(all_suites[i], config);
    }
}

int main(int argc, char* argv[])
{
    bool verbose = false;
    const char* suite_to_run = nullptr;
    const char* test_to_run = nullptr;

    // Parse command line arguments
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            print_usage(argv[0]);
            return 0;
        } else if (strcmp(argv[i], "--list-suites") == 0) {
            list_suites();
            return 0;
        } else if (strcmp(argv[i], "--verbose") == 0) {
            verbose = true;
        } else if (strcmp(argv[i], "--suite") == 0) {
            if (i + 1 < argc) {
                suite_to_run = argv[++i];
            } else {
                printf("Error: --suite requires a suite name\n");
                print_usage(argv[0]);
                return 1;
            }
        } else if (strcmp(argv[i], "--test") == 0) {
            if (i + 1 < argc) {
                test_to_run = argv[++i];
            } else {
                printf("Error: --test requires a test specification (suite::test)\n");
                print_usage(argv[0]);
                return 1;
            }
        } else {
            printf("Error: Unknown option '%s'\n", argv[i]);
            print_usage(argv[0]);
            return 1;
        }
    }

    // Initialize test configuration
    printf("Initializing OpenDAL test framework...\n");
    opendal_test_config* config = opendal_test_config_new();
    if (!config) {
        printf("Failed to initialize test configuration\n");
        return 1;
    }

    printf("Service: %s\n", config->scheme);
    if (config->random_root) {
        printf("Random root: %s\n", config->random_root);
    }

    // Check operator capabilities first
    opendal_operator_info* info = opendal_operator_info_new(config->operator_instance);
    if (!info) {
        printf("Failed to get operator info\n");
        opendal_test_config_free(config);
        return 1;
    }

    opendal_capability cap = opendal_operator_info_get_capability(info);

    // Check operator availability - only perform list-based check if list is supported
    if (cap.list) {
        opendal_error* check_error = opendal_operator_check(config->operator_instance);
        if (check_error) {
            printf("Operator check failed: error code %d\n", check_error->code);
            if (check_error->message.data) {
                printf("Error message: %.*s\n", (int)check_error->message.len, (char*)check_error->message.data);
            }
            opendal_error_free(check_error);
            opendal_operator_info_free(info);
            opendal_test_config_free(config);
            return 1;
        }
    } else {
        // For KV adapters that don't support list, we'll do a basic capability check instead
        printf("Note: Operator doesn't support list operations (KV adapter), skipping standard check\n");
    }

    printf("Operator is ready!\n");
    printf("Capabilities: read=%s, write=%s, list=%s, stat=%s, delete=%s\n",
        cap.read ? "yes" : "no",
        cap.write ? "yes" : "no",
        cap.list ? "yes" : "no",
        cap.stat ? "yes" : "no",
        cap.delete_ ? "yes" : "no");

    opendal_operator_info_free(info);
    printf("\n");

    // Run tests based on command line arguments
    if (test_to_run) {
        // Parse suite::test format
        char* test_spec = strdup(test_to_run);
        char* delimiter = strstr(test_spec, "::");
        if (!delimiter) {
            printf("Error: Test specification must be in format 'suite::test'\n");
            free(test_spec);
            opendal_test_config_free(config);
            return 1;
        }

        *delimiter = '\0';
        const char* suite_name = test_spec;
        const char* test_name = delimiter + 2;

        bool success = run_specific_test(config, suite_name, test_name);
        free(test_spec);

        if (!success) {
            opendal_test_config_free(config);
            return 1;
        }
    } else if (suite_to_run) {
        bool success = run_specific_suite(config, suite_to_run);
        if (!success) {
            opendal_test_config_free(config);
            return 1;
        }
    } else {
        run_all_suites(config);
    }

    // Print test summary
    opendal_print_test_summary();

    // Cleanup
    opendal_test_config_free(config);

    // Return appropriate exit code
    return (failed_tests > 0) ? 1 : 0;
}
