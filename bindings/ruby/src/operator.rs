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

use std::collections::HashMap;
use std::str::FromStr;

use magnus::class;
use magnus::method;
use magnus::prelude::*;
use magnus::Error;
use magnus::RModule;
use magnus::RString;
use magnus::Ruby;

use crate::capability::Capability;
use crate::io::Io;
use crate::metadata::Metadata;
use crate::*;

#[magnus::wrap(class = "OpenDAL::Operator", free_immediately, size)]
#[derive(Clone, Debug)]
struct Operator(ocore::BlockingOperator);

impl Operator {
    fn new(
        ruby: &Ruby,
        scheme: String,
        options: Option<HashMap<String, String>>,
    ) -> Result<Self, Error> {
        let scheme = ocore::Scheme::from_str(&scheme)
            .map_err(|err| {
                ocore::Error::new(ocore::ErrorKind::Unexpected, "unsupported scheme")
                    .set_source(err)
            })
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))?;
        let options = options.unwrap_or_default();

        let op = ocore::Operator::via_iter(scheme, options)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))?
            .blocking();
        Ok(Operator(op))
    }

    /// Reads the whole path into string.
    fn read(ruby: &Ruby, rb_self: &Self, path: String) -> Result<bytes::Bytes, Error> {
        let buffer = rb_self
            .0
            .read(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))?;
        Ok(buffer.to_bytes())
    }

    /// Writes string into given path.
    fn write(ruby: &Ruby, rb_self: &Self, path: String, bs: RString) -> Result<(), Error> {
        rb_self
            .0
            .write(&path, bs.to_bytes())
            .map(|_| ())
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// Gets current path's metadata **without cache** directly.
    fn stat(ruby: &Ruby, rb_self: &Self, path: String) -> Result<Metadata, Error> {
        rb_self
            .0
            .stat(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
            .map(Metadata::new)
    }

    /// Gets capabilities of the current operator
    fn capability(&self) -> Result<Capability, Error> {
        let capability = self.0.info().full_capability();
        Ok(Capability::new(capability))
    }

    /// Creates directory recursively similar as `mkdir -p`
    /// The ending path must be `/`. Otherwise, OpenDAL throws `NotADirectory` error.
    fn create_dir(ruby: &Ruby, rb_self: &Self, path: String) -> Result<(), Error> {
        rb_self
            .0
            .create_dir(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// Deletes given path
    fn delete(ruby: &Ruby, rb_self: &Self, path: String) -> Result<(), Error> {
        rb_self
            .0
            .delete(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// Returns if this path exists
    fn exists(ruby: &Ruby, rb_self: &Self, path: String) -> Result<bool, Error> {
        rb_self
            .0
            .exists(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// Renames a file from `from` to `to`
    fn rename(ruby: &Ruby, rb_self: &Self, from: String, to: String) -> Result<(), Error> {
        rb_self
            .0
            .rename(&from, &to)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// Removes the path and all nested directories and files recursively
    fn remove_all(ruby: &Ruby, rb_self: &Self, path: String) -> Result<(), Error> {
        rb_self
            .0
            .remove_all(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// Copies a file from `from` to `to`.
    fn copy(ruby: &Ruby, rb_self: &Self, from: String, to: String) -> Result<(), Error> {
        rb_self
            .0
            .copy(&from, &to)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// Opens a IO-like reader for the given path.
    fn open(ruby: &Ruby, rb_self: &Self, path: String, mode: String) -> Result<Io, Error> {
        let operator = rb_self.0.clone();
        Ok(Io::new(&ruby, operator, path, mode)?)
    }
}

pub fn include(gem_module: &RModule) -> Result<(), Error> {
    let class = gem_module.define_class("Operator", class::object())?;
    class.define_singleton_method("new", function!(Operator::new, 2))?;
    class.define_method("read", method!(Operator::read, 1))?;
    class.define_method("write", method!(Operator::write, 2))?;
    class.define_method("stat", method!(Operator::stat, 1))?;
    class.define_method("capability", method!(Operator::capability, 0))?;
    class.define_method("create_dir", method!(Operator::create_dir, 1))?;
    class.define_method("delete", method!(Operator::delete, 1))?;
    class.define_method("exist?", method!(Operator::exists, 1))?;
    class.define_method("rename", method!(Operator::rename, 2))?;
    class.define_method("remove_all", method!(Operator::remove_all, 1))?;
    class.define_method("copy", method!(Operator::copy, 2))?;
    class.define_method("open", method!(Operator::open, 2))?;

    Ok(())
}
