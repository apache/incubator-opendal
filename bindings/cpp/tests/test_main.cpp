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

#include "framework/test_framework.hpp"
#include <iostream>

int main(int argc, char **argv) {
    // Initialize GoogleTest
    ::testing::InitGoogleTest(&argc, argv);
    
    // Initialize our test framework
    opendal::test::initialize_test_framework();
    
    // Print test configuration
    auto& config = opendal::test::TestConfig::instance();
    std::cout << "=== OpenDAL C++ Test Suite ===" << std::endl;
    std::cout << "Service: " << config.service_name() << std::endl;
    std::cout << "Config options: " << config.config().size() << std::endl;
    
    for (const auto& [key, value] : config.config()) {
        std::cout << "  " << key << ": " << value << std::endl;
    }
    
    std::cout << "Random root disabled: " << (config.disable_random_root() ? "yes" : "no") << std::endl;
    std::cout << "===============================" << std::endl;
    
    // Add custom test listener for better output
    ::testing::TestEventListeners& listeners = ::testing::UnitTest::GetInstance()->listeners();
    
    // Run all tests
    int result = RUN_ALL_TESTS();
    
    std::cout << "=== Test Summary ===" << std::endl;
    auto* unit_test = ::testing::UnitTest::GetInstance();
    std::cout << "Total tests: " << unit_test->total_test_count() << std::endl;
    std::cout << "Successful tests: " << unit_test->successful_test_count() << std::endl;
    std::cout << "Failed tests: " << unit_test->failed_test_count() << std::endl;
    std::cout << "Skipped tests: " << unit_test->skipped_test_count() << std::endl;
    std::cout << "===================" << std::endl;
    
    return result;
} 