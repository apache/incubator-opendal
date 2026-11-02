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

/*
 * This file demonstrates how to use the OpenDAL C binding test framework.
 * It shows a simple test case that can be run independently.
 */

#include "test_framework.h"

// Simple test function
void simple_test()
{
    printf("Running simple test example...\n");

    // Initialize test configuration
    opendal_test_config* config = opendal_test_config_new();
    if (!config) {
        printf("Failed to create test config\n");
        return;
    }

    printf("Testing with service: %s\n", config->scheme);

    // Test basic operator functionality
    opendal_error* error = opendal_operator_check(config->operator_instance);
    if (error) {
        printf("Operator check failed: %d\n", error->code);
        if (error->message.data) {
            printf("Error: %.*s\n", (int)error->message.len,
                (char*)error->message.data);
        }
        opendal_error_free(error);
        opendal_test_config_free(config);
        return;
    }

    printf("Operator check passed!\n");

    // Test basic write/read if supported
    opendal_operator_info* info = opendal_operator_info_new(config->operator_instance);
    if (info) {
        opendal_capability cap = opendal_operator_info_get_capability(info);

        if (cap.write && cap.read) {
            printf("Testing write/read operations...\n");

            const char* test_path = "simple_test.txt";
            const char* test_content = "Hello, OpenDAL!";

            // Write test data
            opendal_bytes data = { .data = (uint8_t*)test_content,
                .len = strlen(test_content),
                .capacity = strlen(test_content) };

            error = opendal_operator_write(config->operator_instance, test_path, &data);
            if (error) {
                printf("Write failed: %d\n", error->code);
                opendal_error_free(error);
            } else {
                printf("Write successful!\n");

                // Read test data back
                opendal_result_read result = opendal_operator_read(config->operator_instance, test_path);
                if (result.error) {
                    printf("Read failed: %d\n", result.error->code);
                    opendal_error_free(result.error);
                } else {
                    printf("Read successful! Content: %.*s\n", (int)result.data.len,
                        (char*)result.data.data);

                    // Verify content
                    if (result.data.len == strlen(test_content) && memcmp(result.data.data, test_content, result.data.len) == 0) {
                        printf("Content verification passed!\n");
                    } else {
                        printf("Content verification failed!\n");
                    }

                    opendal_bytes_free(&result.data);
                }

                // Cleanup
                opendal_operator_delete(config->operator_instance, test_path);
            }
        } else {
            printf("Write/read not supported by this service\n");
        }

        opendal_operator_info_free(info);
    }

    printf("Simple test completed!\n");
    opendal_test_config_free(config);
}

int main()
{
    printf("OpenDAL C Binding Test Framework Example\n");
    printf("========================================\n\n");

    simple_test();

    printf("\nTo run the full test suite, use:\n");
    printf("  make test                    # Run all tests\n");
    printf("  ./opendal_test_runner        # Run test runner directly\n");
    printf("  ./opendal_test_runner --help # See all options\n");

    return 0;
}
