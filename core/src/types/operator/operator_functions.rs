// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Functions provides the functions generated by [`BlockingOperator`]
//!
//! By using functions, users can add more options for operation.

use bytes::Bytes;

use crate::ops::*;
use crate::raw::*;
use crate::*;

/// OperatorFunction is the function generated by [`BlockingOperator`].
///
/// The function will consume all the input to generate a result.
pub(crate) struct OperatorFunction<T, R> {
    inner: FusedAccessor,
    path: String,
    args: T,
    f: fn(FusedAccessor, String, T) -> Result<R>,
}

impl<T, R> OperatorFunction<T, R> {
    pub fn new(
        inner: FusedAccessor,
        path: String,
        args: T,
        f: fn(FusedAccessor, String, T) -> Result<R>,
    ) -> Self {
        Self {
            inner,
            path,
            args,
            f,
        }
    }

    fn map_args(self, f: impl FnOnce(T) -> T) -> Self {
        Self {
            inner: self.inner,
            path: self.path,
            args: f(self.args),
            f: self.f,
        }
    }

    fn call(self) -> Result<R> {
        (self.f)(self.inner, self.path, self.args)
    }
}

/// Function that generated by [`BlockingOperator::write_with`].
///
/// Users can add more options by public functions provided by this struct.
pub struct FunctionWrite(
    /// The args for FunctionWrite is a bit special because we also
    /// need to move the bytes input this function.
    pub(crate) OperatorFunction<(OpWrite, Bytes), ()>,
);

impl FunctionWrite {
    /// Set the content length for this operation.
    pub fn content_length(mut self, v: u64) -> Self {
        self.0 = self
            .0
            .map_args(|(args, bs)| (args.with_content_length(v), bs));
        self
    }

    /// Set the content type for this operation.
    pub fn content_type(mut self, v: &str) -> Self {
        self.0 = self
            .0
            .map_args(|(args, bs)| (args.with_content_type(v), bs));
        self
    }

    /// Call the function to comsume all the input and generate a
    /// result.
    pub fn call(self) -> Result<()> {
        self.0.call()
    }
}
