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

#include <atomic>
#include <memory>
#include <mutex>
#include <string>

#include "gtest/gtest.h"

extern "C" {
#include "opendal.h"
}

namespace {
struct Capture {
    std::mutex mutex;
    size_t events = 0;
    size_t warnings = 0;
    bool valid = true;
    std::string message;
    std::string file;
    std::string warning;
};

Capture& capture()
{
    // The callback stays valid beyond individual test bodies.
    static auto* state = new Capture;
    return *state;
}

std::atomic<size_t> rejected_events { 0 };

extern "C" void capture_log(int32_t level, const char* file, uint32_t line, const char* message)
{
    (void)line;
    auto& state = capture();
    std::lock_guard<std::mutex> lock(state.mutex);
    if (level < 0 || level > 3 || file == nullptr || message == nullptr) {
        state.valid = false;
        return;
    }
    ++state.events;
    state.message = message;
    state.file = file;
    if (level >= 2) {
        ++state.warnings;
        state.warning = message;
    }
}

extern "C" void rejected_log(int32_t level, const char* file, uint32_t line, const char* message)
{
    (void)level;
    (void)file;
    (void)line;
    (void)message;
    ++rejected_events;
}
}

TEST(OpendalLogging, NativeEventsAndRegistrationFailure)
{
    using Error = std::unique_ptr<opendal_error, decltype(&opendal_error_free)>;
    Error missing_callback(opendal_logging_init(nullptr), opendal_error_free);
    ASSERT_NE(missing_callback, nullptr);
    EXPECT_EQ(missing_callback->code, OPENDAL_CONFIG_INVALID);

    Error initialized(opendal_logging_init(capture_log), opendal_error_free);
    ASSERT_EQ(initialized, nullptr);
    auto result = opendal_operator_new("memory", nullptr);
    Error construction_error(result.error, opendal_error_free);
    ASSERT_EQ(construction_error, nullptr);
    std::unique_ptr<const opendal_operator, decltype(&opendal_operator_free)> op(result.op, opendal_operator_free);
    ASSERT_NE(op, nullptr);
    std::string content = "native logging";
    opendal_bytes data { reinterpret_cast<uint8_t*>(&content[0]), content.size() };
    auto& state = capture();
    size_t before_success;
    {
        std::lock_guard<std::mutex> lock(state.mutex);
        before_success = state.events;
    }
    Error written(opendal_operator_write(op.get(), "logging-value", &data), opendal_error_free);
    ASSERT_EQ(written, nullptr);
    size_t before_failure;
    {
        std::lock_guard<std::mutex> lock(state.mutex);
        EXPECT_TRUE(state.valid);
        EXPECT_GT(state.events, before_success);
        EXPECT_FALSE(state.message.empty());
        EXPECT_FALSE(state.file.empty());
        before_failure = state.warnings;
    }

    auto read = opendal_operator_read(op.get(), "missing-logging-value");
    Error read_error(read.error, opendal_error_free);
    opendal_bytes_free(&read.data);
    ASSERT_NE(read_error, nullptr);
    EXPECT_EQ(read_error->code, OPENDAL_NOT_FOUND);
    {
        std::lock_guard<std::mutex> lock(state.mutex);
        EXPECT_TRUE(state.valid);
        EXPECT_GT(state.warnings, before_failure);
        EXPECT_FALSE(state.warning.empty());
    }

    Error duplicate(opendal_logging_init(rejected_log), opendal_error_free);
    ASSERT_NE(duplicate, nullptr);
    EXPECT_EQ(duplicate->code, OPENDAL_CONFIG_INVALID);
    size_t before_write;
    {
        std::lock_guard<std::mutex> lock(state.mutex);
        before_write = state.events;
    }
    Error second_write(opendal_operator_write(op.get(), "second-logging-value", &data), opendal_error_free);
    ASSERT_EQ(second_write, nullptr);
    {
        std::lock_guard<std::mutex> lock(state.mutex);
        EXPECT_GT(state.events, before_write);
        EXPECT_FALSE(state.message.empty());
    }
    EXPECT_EQ(rejected_events.load(), 0u);
}
