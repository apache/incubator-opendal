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
#include <cstdint>
#include <memory>
#include <vector>

namespace opendal {

class Operator;

namespace ffi {
class LayerBuilder;
}  // namespace ffi

/**
 * @brief Abstraction for applying layers during operator construction.
 */
class LayerBuilderMutator {
 public:
  virtual ~LayerBuilderMutator() = default;

  virtual void AddTimeout(uint64_t timeout_ns, uint64_t io_timeout_ns) = 0;
  virtual void AddRetry(bool jitter, float factor, uint64_t min_delay_ns,
                        uint64_t max_delay_ns, uint64_t max_times) = 0;
};

/**
 * @brief Applies layers through the cxx bridge.
 */
class FfiLayerBuilderMutator final : public LayerBuilderMutator {
 public:
  explicit FfiLayerBuilderMutator(ffi::LayerBuilder &builder);

  void AddTimeout(uint64_t timeout_ns, uint64_t io_timeout_ns) override;
  void AddRetry(bool jitter, float factor, uint64_t min_delay_ns,
                uint64_t max_delay_ns, uint64_t max_times) override;

 private:
  ffi::LayerBuilder &builder_;
};

/**
 * @brief Configuration for the retry layer.
 */
struct RetryConfig {
  bool jitter = false;
  float factor = 2.0f;
  std::chrono::nanoseconds min_delay{std::chrono::seconds(1)};
  std::chrono::nanoseconds max_delay{std::chrono::minutes(1)};
  uint64_t max_times = 3;
};

/**
 * @brief Configures operator layers applied during construction.
 */
class OperatorOption {
 public:
  virtual ~OperatorOption() = default;

  void ApplyTo(LayerBuilderMutator &builder) const { Apply(builder); }

 protected:
  friend class Operator;
  virtual void Apply(LayerBuilderMutator &builder) const = 0;
};

/**
 * @brief Adds a timeout layer to the operator.
 *
 * timeout controls non-IO operations, while io_timeout controls read, write,
 * list, and streaming IO operations.
 */
std::unique_ptr<OperatorOption> WithTimeout(
    std::chrono::nanoseconds timeout, std::chrono::nanoseconds io_timeout);

/**
 * @brief Adds a retry layer to the operator.
 *
 * Layers are applied in the order passed to Operator. When combining retry and
 * timeout, pass WithTimeout before WithRetry so each retry attempt has its own
 * timeout.
 */
std::unique_ptr<OperatorOption> WithRetry(RetryConfig config = {});

/**
 * @brief Default layer options used by behavior tests.
 */
std::vector<std::unique_ptr<OperatorOption>> DefaultBehaviorLayerOptions();

}  // namespace opendal
