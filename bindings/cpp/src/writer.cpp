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

#include "lib.rs.h"
#include "opendal.hpp"
#include "utils/rust_converter.hpp"

namespace opendal {

void Writer::Destroy() noexcept {
  if (writer_) {
    ffi::delete_writer(writer_);
    writer_ = nullptr;
  }
}

Writer::Writer(ffi::Writer *writer) noexcept : writer_{writer} {}

Writer::Writer(Writer &&other) noexcept : writer_{other.writer_} {
  other.writer_ = nullptr;
}

Writer::~Writer() noexcept { Destroy(); }

void Writer::Write(std::string_view data) {
  writer_->write(utils::rust_slice<const uint8_t>(data));
}

void Writer::Flush() { writer_->flush(); }

void Writer::Close() { writer_->close(); }

}  // namespace opendal
