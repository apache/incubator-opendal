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

#include <chrono>
#include <memory>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "framework/test_framework.hpp"
#include "helpers/mock_operator.hpp"
#include "layer.hpp"

namespace opendal::test {

class LayerTest : public ::testing::Test {};

namespace {

std::vector<std::unique_ptr<OperatorOption>> FastRetryOptions() {
  RetryConfig config;
  config.max_times = 3;
  config.min_delay = std::chrono::milliseconds(1);
  config.max_delay = std::chrono::milliseconds(1);

  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithRetry(config));
  return options;
}

}  // namespace

OPENDAL_TEST_F(LayerTest, IoTimeoutAbortsHangingRead) {
  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithTimeout(std::chrono::minutes(1),
                                std::chrono::milliseconds(1)));

  const auto start = std::chrono::steady_clock::now();
  MockOperator op(std::move(options), MockBackendKind::Hanging);

  try {
    op.Read("test");
    FAIL() << "expected hanging read to fail with timeout";
  } catch (const std::exception& e) {
    EXPECT_NE(std::string_view{e.what()}.find("timeout"), std::string_view::npos)
        << e.what();
  }

  const auto elapsed = std::chrono::steady_clock::now() - start;
  EXPECT_LT(elapsed, std::chrono::seconds(2));
}

OPENDAL_TEST_F(LayerTest, RetryReadSucceedsAfterTransientFailures) {
  MockOperator op(FastRetryOptions(), MockBackendKind::Retryable);

  EXPECT_EQ(op.Read("retryable_error"), "Hello, World!");
  EXPECT_EQ(MockOperator::RetryAttemptCount(), 5U);
}

OPENDAL_TEST_F(LayerTest, RetryExhaustsConfiguredMaxTimes) {
  RetryConfig config;
  config.max_times = 1;
  config.min_delay = std::chrono::milliseconds(1);
  config.max_delay = std::chrono::milliseconds(1);

  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithRetry(config));

  MockOperator op(std::move(options), MockBackendKind::Retryable);

  EXPECT_THROW(op.Read("retryable_error"), std::exception);
  EXPECT_LT(MockOperator::RetryAttemptCount(), 5U);
}

OPENDAL_TEST_F(LayerTest, OperatorWithTimeoutAndRetry) {
  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithTimeout(std::chrono::minutes(1),
                                std::chrono::seconds(10)));
  options.push_back(WithRetry());

  Operator op("memory", {}, std::move(options));
  ASSERT_TRUE(op.Available());

  constexpr std::string_view data = "layer-test";
  op.Write("layer-test.txt", data);
  EXPECT_EQ(op.Read("layer-test.txt"), data);
}

OPENDAL_TEST_F(LayerTest, WithTimeoutRejectsZeroTimeout) {
  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithTimeout(std::chrono::seconds(0), std::chrono::seconds(1)));
  EXPECT_THROW(Operator("memory", {}, std::move(options)), std::invalid_argument);
}

OPENDAL_TEST_F(LayerTest, WithTimeoutRejectsZeroIoTimeout) {
  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithTimeout(std::chrono::seconds(1), std::chrono::seconds(0)));
  EXPECT_THROW(Operator("memory", {}, std::move(options)), std::invalid_argument);
}

OPENDAL_TEST_F(LayerTest, RetryMaxTimesRejectsZero) {
  RetryConfig config;
  config.max_times = 0;

  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithRetry(config));

  EXPECT_THROW(Operator("memory", {}, std::move(options)), std::invalid_argument);
}

OPENDAL_TEST_F(LayerTest, RetryMaxDelayBeforeMinDelay) {
  RetryConfig config;
  config.min_delay = std::chrono::seconds(10);
  config.max_delay = std::chrono::seconds(1);

  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithRetry(config));

  EXPECT_THROW(Operator("memory", {}, std::move(options)), std::invalid_argument);
}

OPENDAL_TEST_F(LayerTest, RetryFactorRejectsInvalidValue) {
  RetryConfig config;
  config.factor = 0.5F;

  std::vector<std::unique_ptr<OperatorOption>> options;
  options.push_back(WithRetry(config));

  EXPECT_THROW(Operator("memory", {}, std::move(options)), std::invalid_argument);
}

}  // namespace opendal::test
