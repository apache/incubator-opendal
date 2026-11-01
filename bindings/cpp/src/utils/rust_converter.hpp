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

#include "rust/cxx.h"

namespace opendal::utils {

template <typename T>
auto rust_str(T &&s) -> decltype(s.data(), s.size(), rust::Str()) {
  return rust::Str(s.data(), s.size());
}

template <typename T>
auto rust_string(T &&s) -> decltype(s.data(), s.size(), rust::String()) {
  return rust::String(s.data(), s.size());
}

template <typename T, typename Container>
auto rust_slice(Container &&s) -> decltype(s.data(), s.size(),
                                           rust::Slice<T>()) {
  using Elem = std::remove_pointer_t<decltype(s.data())>;
  static_assert(std::is_convertible_v<Elem, T>,
                "Container element type must be convertible to T");

  return rust::Slice<T>(reinterpret_cast<T *>(s.data()), s.size());
}

}  // namespace opendal::utils
