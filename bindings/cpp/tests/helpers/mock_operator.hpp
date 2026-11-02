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

#include <memory>
#include <string>
#include <string_view>
#include <vector>

#include "layer.hpp"
#include "lib.rs.h"
#include "utils/rust_converter.hpp"

namespace opendal::test {

enum class MockBackendKind {
  Hanging,
  Retryable,
};

class MockOperator {
 public:
  MockOperator(std::vector<std::unique_ptr<OperatorOption>> options,
                MockBackendKind kind)
      : layers_(ffi::layer_builder_new()) {
    FfiLayerBuilderMutator mutator(*layers_);
    for (const auto& option : options) {
      if (option != nullptr) {
        option->ApplyTo(mutator);
      }
    }

    if (kind == MockBackendKind::Hanging) {
      operator_ = ffi::new_test_hanging_operator(*layers_);
    } else {
      operator_ = ffi::new_test_retryable_operator(*layers_);
    }
  }

  MockOperator(const MockOperator&) = delete;
  MockOperator& operator=(const MockOperator&) = delete;

  ~MockOperator() {
    if (operator_ != nullptr) {
      ffi::delete_operator(operator_);
    }
  }

  std::string Read(std::string_view path) {
    const auto data = operator_->read(utils::rust_str(path));
    return {data.begin(), data.end()};
  }

  static uint64_t RetryAttemptCount() {
    return ffi::test_retryable_attempt_count();
  }

 private:
  rust::Box<ffi::LayerBuilder> layers_;
  ffi::Operator* operator_{nullptr};
};

}  // namespace opendal::test
