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

#![allow(
    rustdoc::broken_intra_doc_links,
    reason = "YARD's syntax for documentation"
)]
#![allow(rustdoc::invalid_html_tags, reason = "YARD's syntax for documentation")]
#![allow(rustdoc::bare_urls, reason = "YARD's syntax for documentation")]

use std::collections::HashMap;

use magnus::Error;
use magnus::RHash;
use magnus::RModule;
use magnus::RString;
use magnus::Ruby;
use magnus::Value;
use magnus::method;
use magnus::prelude::*;
use magnus::scan_args::get_kwargs;
use magnus::scan_args::scan_args;

use crate::capability::Capability;
use crate::io::Io;
use crate::lister::Lister;
use crate::metadata::Metadata;
use crate::operator_info::OperatorInfo;
use crate::*;

/// @yard
/// The entrypoint for operating with file services and files.
#[magnus::wrap(class = "OpenDal::Operator", free_immediately, size)]
pub struct Operator {
    // We keep a reference to an `Operator` because:
    // 1. Some builder functions exist only with the `Operator` struct.
    // 2. Some builder functions don't exist in the `BlockingOperator`, e.g., `Operator::layer`, builder methods.
    //
    // We don't support async because:
    // 1. Ractor and async is not stable.
    // 2. magnus doesn't release GVL lock during operations yet.
    // 3. Majority of use cases are still blocking.
    //
    // In practice, we will not use operator directly because of the async support.
    pub async_op: ocore::Operator,
    // The cached blocking operator coming from `Operator::blocking`.
    // Important to keep in sync after making changes to the `Operator`.
    //
    // We declare this `BlockingOperator` to state the intent of assumptions instead of
    // getting a `BlockingOperator` for an operation dynamically every time.
    pub blocking_op: ocore::blocking::Operator,
}

impl Operator {
    // Convenience helper to construct operator
    #[inline]
    pub(crate) fn from_operator(operator: ocore::Operator) -> Operator {
        let handle = RUNTIME.handle();
        let _enter = handle.enter();
        let blocking_op = ocore::blocking::Operator::new(operator.clone())
            .expect("initiate blocking operator must succeed");

        Operator {
            async_op: operator,
            blocking_op,
        }
    }
}

impl Operator {
    fn new(
        ruby: &Ruby,
        scheme: String,
        options: Option<HashMap<String, String>>,
    ) -> Result<Self, Error> {
        let options = options.unwrap_or_default();

        let op = ocore::Operator::via_iter(scheme, options)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))?;

        Ok(Operator::from_operator(op))
    }

    /// @yard
    /// @def read(path)
    /// Reads the whole path into string.
    /// @param path [String]
    /// @return [String]
    fn read(ruby: &Ruby, rb_self: &Self, path: String) -> Result<bytes::Bytes, Error> {
        let buffer = rb_self
            .blocking_op
            .read(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))?;
        Ok(buffer.to_bytes())
    }

    /// @yard
    /// @def write(path, buffer)
    /// Writes string into given path.
    /// @param path [String]
    /// @param buffer [String]
    /// @return [nil]
    fn write(ruby: &Ruby, rb_self: &Self, path: String, bs: RString) -> Result<(), Error> {
        rb_self
            .blocking_op
            .write(&path, bs.to_bytes())
            .map(|_| ())
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// @yard
    /// @def read(path)
    /// Gets current path's metadata **without cache** directly.
    /// @param path
    /// @return [Metadata]
    fn stat(ruby: &Ruby, rb_self: &Self, path: String) -> Result<Metadata, Error> {
        rb_self
            .blocking_op
            .stat(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
            .map(Metadata::new)
    }

    /// @yard
    /// @def capability
    /// Gets capabilities of the current operator
    /// @return [Capability]
    fn capability(&self) -> Result<Capability, Error> {
        let capability = self.blocking_op.info().capability();
        Ok(Capability::new(capability))
    }

    /// @yard
    /// @def create_dir(path)
    /// Creates directory recursively similar as `mkdir -p`
    /// The ending path must be `/`. Otherwise, OpenDAL throws `NotADirectory` error.
    /// @param path [String]
    /// @return [nil]
    fn create_dir(ruby: &Ruby, rb_self: &Self, path: String) -> Result<(), Error> {
        rb_self
            .blocking_op
            .create_dir(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// @yard
    /// @def delete(path)
    /// Deletes given path
    /// @param path [String]
    /// @return [nil]
    fn delete(ruby: &Ruby, rb_self: &Self, path: String) -> Result<(), Error> {
        rb_self
            .blocking_op
            .delete(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// @yard
    /// @def exist?(path)
    /// Returns if this path exists
    /// @param path [String]
    /// @return [Boolean]
    fn exists(ruby: &Ruby, rb_self: &Self, path: String) -> Result<bool, Error> {
        rb_self
            .blocking_op
            .exists(&path)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// @yard
    /// @def rename(from, to)
    /// Renames a file from `from` to `to`
    /// @param from [String] a file path
    /// @param to [String] a file path
    /// @return [nil]
    fn rename(ruby: &Ruby, rb_self: &Self, from: String, to: String) -> Result<(), Error> {
        rb_self
            .blocking_op
            .rename(&from, &to)
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// @yard
    /// @def remove_all(path)
    /// Removes the path and all nested directories and files recursively
    /// @param path [String]
    /// @return [nil]
    fn remove_all(ruby: &Ruby, rb_self: &Self, path: String) -> Result<(), Error> {
        use ocore::options::DeleteOptions;
        rb_self
            .blocking_op
            .delete_options(
                &path,
                DeleteOptions {
                    recursive: true,
                    ..Default::default()
                },
            )
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// @yard
    /// @def copy(from, to)
    /// Copies a file from `from` to `to`.
    /// @param from [String] a file path
    /// @param to [String] a file path
    /// @return [nil]
    fn copy(ruby: &Ruby, rb_self: &Self, from: String, to: String) -> Result<(), Error> {
        rb_self
            .blocking_op
            .copy(&from, &to)
            .map(|_| ())
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))
    }

    /// @yard
    /// @def open(path, mode)
    /// Opens a `IO`-like reader for the given path.
    /// @param path [String] file path
    /// @param mode [String] operation mode, e.g., `r`, `w`, or `rb`.
    /// @raise [ArgumentError] invalid mode, or when the mode is not unique
    /// @return [OpenDal::IO]
    fn open(ruby: &Ruby, rb_self: &Self, args: &[Value]) -> Result<Io, Error> {
        let args = scan_args::<(String,), (Option<Value>, Option<Value>), (), (), RHash, ()>(args)?;
        let (path,) = args.required;
        let (option_mode, option_permission) = args.optional;
        let kwargs = args.keywords;

        // Ruby handles Qnil safely (will not assign to Qnil)
        let mode = option_mode.unwrap_or(ruby.str_new("r").as_value());
        let permission = option_permission.unwrap_or(ruby.qnil().as_value());

        let operator = rb_self.blocking_op.clone();
        Io::new(ruby, operator, path, mode, permission, kwargs)
    }

    /// @yard
    /// @def list(limit: nil, start_after: nil, recursive: nil)
    /// Lists the directory.
    /// @param limit [usize, nil] per-request max results
    /// @param start_after [String, nil] the specified key to start listing from.
    /// @param recursive [Boolean, nil] lists the directory recursively.
    /// @return [Lister]
    pub fn list(ruby: &Ruby, rb_self: &Self, args: &[Value]) -> Result<Lister, Error> {
        let args = scan_args::<(String,), (), (), (), _, ()>(args)?;
        let (path,) = args.required;
        let kwargs = get_kwargs::<_, (), (Option<usize>, Option<String>, Option<bool>), ()>(
            args.keywords,
            &[],
            &["limit", "start_after", "recursive"],
        )?;
        let (limit, start_after, recursive) = kwargs.optional;

        let lister = rb_self
            .blocking_op
            .lister_options(
                &path,
                ocore::options::ListOptions {
                    limit,
                    start_after,
                    recursive: recursive.unwrap_or(false),
                    ..Default::default()
                },
            )
            .map_err(|err| Error::new(ruby.exception_runtime_error(), err.to_string()))?;

        Ok(Lister::new(lister))
    }

    /// @yard
    /// @def info
    /// Gets meta information of the underlying accessor.
    /// @return [OperatorInfo]
    fn info(&self) -> Result<OperatorInfo, Error> {
        Ok(OperatorInfo(self.blocking_op.info()))
    }
}

pub fn include(ruby: &Ruby, gem_module: &RModule) -> Result<(), Error> {
    let class = gem_module.define_class("Operator", ruby.class_object())?;
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
    class.define_method("open", method!(Operator::open, -1))?;
    class.define_method("list", method!(Operator::list, -1))?;
    class.define_method("info", method!(Operator::info, 0))?;

    Ok(())
}
