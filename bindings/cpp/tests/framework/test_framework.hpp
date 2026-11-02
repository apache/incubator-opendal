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

#pragma once

#include <chrono>
#include <cstdlib>
#include <ctime>
#include <random>
#include <string>
#include <unordered_map>
#include <vector>
#include <algorithm>
#include <memory>
#include <functional>
#include <numeric>

#include "gtest/gtest.h"
#include "layer.hpp"
#include "opendal.hpp"

#ifdef OPENDAL_ENABLE_ASYNC
#include "opendal_async.hpp"
#include "cppcoro/sync_wait.hpp"
#include "cppcoro/task.hpp"
#endif

extern char** environ;

namespace opendal::test {

/**
 * @brief Test configuration and environment setup
 */
class TestConfig {
public:
    static TestConfig& instance() {
        static TestConfig instance;
        return instance;
    }
    
    const std::string& service_name() const { return service_name_; }
    const std::unordered_map<std::string, std::string>& config() const { return config_; }
    bool disable_random_root() const { return disable_random_root_; }
    
    void initialize() {
        // Read service name from environment
        const char* env_service = std::getenv("OPENDAL_TEST");
        if (env_service) {
            service_name_ = env_service;
        } else {
            service_name_ = "memory"; // Default for tests
        }
        
        // Read configuration from environment variables
        std::string prefix = "opendal_" + service_name_ + "_";
        std::transform(prefix.begin(), prefix.end(), prefix.begin(), ::tolower);
        
        for (char** env = environ; *env != nullptr; ++env) {
            std::string env_var(*env);
            auto eq_pos = env_var.find('=');
            if (eq_pos != std::string::npos) {
                std::string key = env_var.substr(0, eq_pos);
                std::string value = env_var.substr(eq_pos + 1);
                
                std::transform(key.begin(), key.end(), key.begin(), ::tolower);
                
                if (key.find(prefix) == 0) {
                    std::string config_key = key.substr(prefix.length());
                    config_[config_key] = value;
                }
            }
        }
        
        // Check if random root is disabled
        const char* disable_random = std::getenv("OPENDAL_DISABLE_RANDOM_ROOT");
        disable_random_root_ = (disable_random && std::string(disable_random) == "true");
        
        if (!disable_random_root_) {
            // Add random root path to avoid conflicts
            std::string root = config_.count("root") ? config_["root"] : "/";
            if (root.back() != '/') root += "/";
            root += generate_uuid() + "/";
            config_["root"] = root;
        }
    }
    
private:
    std::string service_name_;
    std::unordered_map<std::string, std::string> config_;
    bool disable_random_root_ = false;
    
    std::string generate_uuid() {
        std::random_device rd;
        std::mt19937 gen(rd());
        std::uniform_int_distribution<> dis(0, 15);
        
        const char* hex = "0123456789abcdef";
        std::string uuid(36, '-');
        
        for (int i = 0; i < 36; ++i) {
            if (i == 8 || i == 13 || i == 18 || i == 23) continue;
            uuid[i] = hex[dis(gen)];
        }
        
        return uuid;
    }
};

/**
 * @brief Test data generator utilities
 */
class TestData {
public:
    static std::string random_string(size_t length = 100) {
        static std::random_device rd;
        static std::mt19937 gen(rd());
        // Use only alphanumeric characters and safe symbols for maximum compatibility
        static std::vector<char> safe_chars;
        if (safe_chars.empty()) {
            // Include only alphanumeric characters and a few safe symbols
            for (char c = '0'; c <= '9'; ++c) safe_chars.push_back(c);
            for (char c = 'A'; c <= 'Z'; ++c) safe_chars.push_back(c); 
            for (char c = 'a'; c <= 'z'; ++c) safe_chars.push_back(c);
            // Add only very safe special characters
            safe_chars.push_back('_');
            safe_chars.push_back('-');
        }
        static std::uniform_int_distribution<> dis(0, safe_chars.size() - 1);
        
        std::string result;
        result.reserve(length);
        for (size_t i = 0; i < length; ++i) {
            result += safe_chars[dis(gen)];
        }
        return result;
    }
    
    static std::vector<uint8_t> random_bytes(size_t length = 100) {
        static std::random_device rd;
        static std::mt19937 gen(rd());
        static std::uniform_int_distribution<uint8_t> dis(0, 255);
        
        std::vector<uint8_t> result;
        result.reserve(length);
        for (size_t i = 0; i < length; ++i) {
            result.push_back(dis(gen));
        }
        return result;
    }
    
    static std::string random_path(const std::string& prefix = "test_") {
        return prefix + random_string(10);
    }
    
    static std::string random_dir_path(const std::string& prefix = "test_dir_") {
        auto path = prefix + random_string(10);
        if (path.back() != '/') path += "/";
        return path;
    }
};

/**
 * @brief Performance measurement utilities
 */
class PerformanceTimer {
public:
    void start() {
        start_time_ = std::chrono::high_resolution_clock::now();
    }
    
    void stop() {
        end_time_ = std::chrono::high_resolution_clock::now();
    }
    
    std::chrono::duration<double> elapsed() const {
        return end_time_ - start_time_;
    }
    
    double elapsed_ms() const {
        return std::chrono::duration_cast<std::chrono::duration<double, std::milli>>(elapsed()).count();
    }
    
    double elapsed_us() const {
        return std::chrono::duration_cast<std::chrono::duration<double, std::micro>>(elapsed()).count();
    }
    
private:
    std::chrono::high_resolution_clock::time_point start_time_;
    std::chrono::high_resolution_clock::time_point end_time_;
};

/**
 * @brief Base test fixture for OpenDAL tests
 */
class OpenDALTest : public ::testing::Test {
protected:
    opendal::Operator op_;
    opendal::Capability capability_;
    std::mt19937 rng_;
    
    void SetUp() override {
        auto& config = TestConfig::instance();
        rng_.seed(std::time(nullptr));
        
        try {
            op_ = opendal::Operator(config.service_name(), config.config(),
                                    opendal::DefaultBehaviorLayerOptions());
            ASSERT_TRUE(op_.Available()) << "Operator not available for service: " << config.service_name();
            capability_ = op_.Info();
        } catch (const std::exception& e) {
            GTEST_SKIP() << "Failed to create operator: " << e.what();
        }
    }
    
    void TearDown() override {
        // Cleanup if needed
    }
    
    // Helper methods for capability checking based on service name
    bool supports_write() const {
        return capability_.write;
    }
    
    bool supports_delete() const {
        return capability_.delete_feature;
    }
    
    bool supports_create_dir() const {
        return capability_.create_dir;
    }
    
    bool supports_list() const {
        return capability_.list;
    }
    
    bool supports_copy() const {
        return capability_.copy;
    }
    
    bool supports_rename() const {
        return capability_.rename;
    }
    
    // Helper methods
    std::string random_path() { return TestData::random_path(); }
    std::string random_dir_path() { return TestData::random_dir_path(); }
    std::string random_string(size_t len = 100) { return TestData::random_string(len); }
    std::vector<uint8_t> random_bytes(size_t len = 100) { return TestData::random_bytes(len); }
};

#ifdef OPENDAL_ENABLE_ASYNC
/**
 * @brief Base test fixture for async OpenDAL tests
 */
class AsyncOpenDALTest : public ::testing::Test {
protected:
    std::optional<opendal::async::Operator> op_;
    std::mt19937 rng_;
    
    void SetUp() override {
        auto& config = TestConfig::instance();
        rng_.seed(std::time(nullptr));
        
        try {
            op_ = opendal::async::Operator(config.service_name(), config.config());
        } catch (const std::exception& e) {
            GTEST_SKIP() << "Failed to create async operator: " << e.what();
        }
    }
    
    void TearDown() override {
        // Cleanup if needed
    }
    
    // Helper methods
    std::string random_path() { return TestData::random_path(); }
    std::string random_dir_path() { return TestData::random_dir_path(); }
    std::string random_string(size_t len = 100) { return TestData::random_string(len); }
    std::vector<uint8_t> random_bytes(size_t len = 100) { return TestData::random_bytes(len); }
};
#endif

/**
 * @brief Benchmark test fixture
 */
class BenchmarkTest : public OpenDALTest {
protected:
    PerformanceTimer timer_;
    
    void benchmark_operation(const std::string& operation_name, std::function<void()> operation, int iterations = 1000) {
        std::vector<double> times;
        times.reserve(iterations);
        
        for (int i = 0; i < iterations; ++i) {
            timer_.start();
            operation();
            timer_.stop();
            times.push_back(timer_.elapsed_ms());
        }
        
        // Calculate statistics
        std::sort(times.begin(), times.end());
        double min_time = times.front();
        double max_time = times.back();
        double median_time = times[times.size() / 2];
        double avg_time = std::accumulate(times.begin(), times.end(), 0.0) / times.size();
        
        std::cout << "Benchmark: " << operation_name << std::endl;
        std::cout << "  Iterations: " << iterations << std::endl;
        std::cout << "  Min: " << min_time << " ms" << std::endl;
        std::cout << "  Max: " << max_time << " ms" << std::endl;
        std::cout << "  Avg: " << avg_time << " ms" << std::endl;
        std::cout << "  Median: " << median_time << " ms" << std::endl;
    }
};

// Initialization function to be called in main()
inline void initialize_test_framework() {
    TestConfig::instance().initialize();
}

} // namespace opendal::test

// Convenience macros
#define OPENDAL_TEST_F(test_fixture, test_name) \
    TEST_F(test_fixture, test_name)

#define OPENDAL_SKIP_IF_NO_SERVICE() \
    do { \
        if (opendal::test::TestConfig::instance().service_name().empty()) { \
            GTEST_SKIP() << "No service configured for testing"; \
        } \
    } while(0) 

#define OPENDAL_SKIP_IF_UNSUPPORTED_WRITE() \
    do { \
        if (!this->supports_write()) { \
            GTEST_SKIP() << "Write operations not supported by service: " << opendal::test::TestConfig::instance().service_name(); \
        } \
    } while(0)

#define OPENDAL_SKIP_IF_UNSUPPORTED_DELETE() \
    do { \
        if (!this->supports_delete()) { \
            GTEST_SKIP() << "Delete operations not supported by service: " << opendal::test::TestConfig::instance().service_name(); \
        } \
    } while(0)

#define OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR() \
    do { \
        if (!this->supports_create_dir()) { \
            GTEST_SKIP() << "Create directory operations not supported by service: " << opendal::test::TestConfig::instance().service_name(); \
        } \
    } while(0)

#define OPENDAL_SKIP_IF_UNSUPPORTED_LIST() \
    do { \
        if (!this->supports_list()) { \
            GTEST_SKIP() << "List operations not supported by service: " << opendal::test::TestConfig::instance().service_name(); \
        } \
    } while(0)

#define OPENDAL_SKIP_IF_UNSUPPORTED_COPY() \
    do { \
        if (!this->supports_copy()) { \
            GTEST_SKIP() << "Copy operations not supported by service: " << opendal::test::TestConfig::instance().service_name(); \
        } \
    } while(0)

#define OPENDAL_SKIP_IF_UNSUPPORTED_RENAME() \
    do { \
        if (!this->supports_rename()) { \
            GTEST_SKIP() << "Rename operations not supported by service: " << opendal::test::TestConfig::instance().service_name(); \
        } \
    } while(0)
