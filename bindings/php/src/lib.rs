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
use std::sync::LazyLock;

use ::opendal as od;
use ext_php_rs::binary::Binary;
use ext_php_rs::convert::FromZval;
use ext_php_rs::exception::PhpException;
use ext_php_rs::flags::DataType;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

#[php_class]
#[php(name = "OpenDAL\\Operator")]
pub struct Operator(od::blocking::Operator);

#[php_impl]
#[php(change_method_case = "none")]
impl Operator {
    pub fn __construct(scheme: String, config: HashMap<String, String>) -> PhpResult<Self> {
        let op = od::Operator::via_iter(scheme, config).map_err(format_php_err)?;

        let handle = RUNTIME.handle();
        let _enter = handle.enter();
        let op = od::blocking::Operator::new(op).map_err(format_php_err)?;

        Ok(Operator(op))
    }

    /// Write string into given path.
    pub fn write(&self, path: &str, content: String) -> PhpResult<()> {
        self.0
            .write(path, content)
            .map(|_| ())
            .map_err(format_php_err)
    }

    /// Write bytes into given path, binary safe.
    pub fn write_binary(&self, path: &str, content: Vec<u8>) -> PhpResult<()> {
        self.0
            .write(path, content)
            .map(|_| ())
            .map_err(format_php_err)
    }

    /// Read the whole path into bytes, binary safe.
    pub fn read(&self, path: &str) -> PhpResult<Binary<u8>> {
        self.0
            .read(path)
            .map_err(format_php_err)
            .map(|buf| Binary::from(buf.to_vec()))
    }

    /// Check if this path exists or not, return 1 if exists, 0 otherwise.
    pub fn is_exist(&self, path: &str) -> PhpResult<u8> {
        self.0
            .exists(path)
            .map_err(format_php_err)
            .map(|b| if b { 1 } else { 0 })
    }

    /// Get current path's metadata **without cache** directly.
    ///
    /// # Notes
    ///
    /// Use `stat` if you:
    ///
    /// - Want detect the outside changes of path.
    /// - Don't want to read from cached metadata.
    pub fn stat(&self, path: &str) -> PhpResult<Metadata> {
        self.0.stat(path).map_err(format_php_err).map(Metadata)
    }

    /// Delete given path.
    ///
    /// # Notes
    ///
    /// - Delete not existing error won't return errors.
    pub fn delete(&self, path: &str) -> PhpResult<()> {
        self.0.delete(path).map_err(format_php_err)
    }

    /// Create a dir at given path.
    ///
    /// # Notes
    ///
    /// To indicate that a path is a directory, it is compulsory to include
    /// a trailing / in the path. Failure to do so may result in
    /// `NotADirectory` error being returned by OpenDAL.
    ///
    /// # Behavior
    ///
    /// - Create on existing dir will succeed.
    /// - Create dir is always recursive, works like `mkdir -p`
    pub fn create_dir(&self, path: &str) -> PhpResult<()> {
        self.0.create_dir(path).map_err(format_php_err)
    }
}

#[php_class]
#[php(name = "OpenDAL\\Metadata")]
pub struct Metadata(od::Metadata);

#[php_impl]
#[php(change_method_case = "none")]
impl Metadata {
    #[php(getter)]
    pub fn content_disposition(&self) -> Option<String> {
        self.0.content_disposition().map(|s| s.to_string())
    }

    /// Content length of this entry.
    #[php(getter)]
    pub fn content_length(&self) -> u64 {
        self.0.content_length()
    }

    /// Content MD5 of this entry.
    #[php(getter)]
    pub fn content_md5(&self) -> Option<String> {
        self.0.content_md5().map(|s| s.to_string())
    }

    /// Content Type of this entry.
    #[php(getter)]
    pub fn content_type(&self) -> Option<String> {
        self.0.content_type().map(|s| s.to_string())
    }

    /// ETag of this entry.
    #[php(getter)]
    pub fn etag(&self) -> Option<String> {
        self.0.etag().map(|s| s.to_string())
    }

    /// mode represent this entry's mode.
    #[php(getter)]
    pub fn mode(&self) -> EntryMode {
        EntryMode(self.0.mode())
    }
}

#[php_class]
#[php(name = "OpenDAL\\EntryMode")]
pub struct EntryMode(od::EntryMode);

impl<'b> FromZval<'b> for EntryMode {
    const TYPE: DataType = DataType::Object(Some("OpenDAL\\EntryMode"));

    fn from_zval(zval: &'b Zval) -> Option<Self> {
        zval.object().and_then(|obj| obj.get_property("mode").ok())
    }
}

#[php_impl]
#[php(change_method_case = "none")]
impl EntryMode {
    #[php(getter)]
    pub fn is_dir(&self) -> u8 {
        match self.0.is_dir() {
            true => 1,
            false => 0,
        }
    }

    #[php(getter)]
    pub fn is_file(&self) -> u8 {
        match self.0.is_file() {
            true => 1,
            false => 0,
        }
    }
}

fn format_php_err(e: od::Error) -> PhpException {
    // @todo use custom exception, we cannot use custom exception now,
    // see https://github.com/davidcole1340/ext-php-rs/issues/262
    PhpException::default(e.to_string())
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .class::<Operator>()
        .class::<Metadata>()
        .class::<EntryMode>()
}
