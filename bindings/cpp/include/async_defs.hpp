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
#include "rust/cxx_async.h"

CXXASYNC_DEFINE_FUTURE(rust::Vec<uint8_t>, opendal, ffi, async_op,
                       RustFutureRead);
CXXASYNC_DEFINE_FUTURE(void, opendal, ffi, async_op, RustFutureWrite);
CXXASYNC_DEFINE_FUTURE(rust::Vec<rust::String>, opendal, ffi, async_op,
                       RustFutureList);
CXXASYNC_DEFINE_FUTURE(bool, opendal, ffi, async_op, RustFutureBool);
CXXASYNC_DEFINE_FUTURE(size_t, opendal, ffi, async_op, RustFutureReaderId);
CXXASYNC_DEFINE_FUTURE(size_t, opendal, ffi, async_op, RustFutureListerId);
CXXASYNC_DEFINE_FUTURE(rust::String, opendal, ffi, async_op,
                       RustFutureEntryOption);
