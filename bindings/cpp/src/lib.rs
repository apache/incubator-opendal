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

#[cfg(feature = "async")]
mod r#async;
mod lister;
mod reader;
mod types;

use std::collections::HashMap;
use std::str::FromStr;

use anyhow::Result;
use lister::Lister;
use opendal as od;
use reader::Reader;

#[cxx::bridge(namespace = "opendal::ffi")]
mod ffi {
    struct HashMapValue {
        key: String,
        value: String,
    }

    #[cxx_name = "SeekDir"]
    enum SeekFrom {
        Start = 0,
        Current = 1,
        End = 2,
    }

    enum EntryMode {
        File = 1,
        Dir = 2,
        Unknown = 0,
    }

    struct OptionalString {
        has_value: bool,
        value: String,
    }

    struct OptionalEntry {
        has_value: bool,
        value: Entry,
    }

    struct Metadata {
        mode: EntryMode,
        content_length: u64,
        cache_control: OptionalString,
        content_disposition: OptionalString,
        content_md5: OptionalString,
        content_type: OptionalString,
        etag: OptionalString,
        last_modified: OptionalString,
    }

    struct Entry {
        path: String,
    }

    extern "Rust" {
        type Operator;
        type Reader;
        type Lister;

        fn new_operator(scheme: &str, configs: Vec<HashMapValue>) -> Result<Box<Operator>>;
        fn read(self: &Operator, path: &str) -> Result<Vec<u8>>;
        fn write(self: &Operator, path: &str, bs: &'static [u8]) -> Result<()>;
        fn exists(self: &Operator, path: &str) -> Result<bool>;
        fn create_dir(self: &Operator, path: &str) -> Result<()>;
        fn copy(self: &Operator, src: &str, dst: &str) -> Result<()>;
        fn rename(self: &Operator, src: &str, dst: &str) -> Result<()>;
        fn remove(self: &Operator, path: &str) -> Result<()>;
        fn stat(self: &Operator, path: &str) -> Result<Metadata>;
        fn list(self: &Operator, path: &str) -> Result<Vec<Entry>>;
        fn reader(self: &Operator, path: &str) -> Result<Box<Reader>>;
        fn lister(self: &Operator, path: &str) -> Result<Box<Lister>>;

        fn read(self: &mut Reader, buf: &mut [u8]) -> Result<usize>;
        fn seek(self: &mut Reader, offset: u64, dir: SeekFrom) -> Result<u64>;

        fn next(self: &mut Lister) -> Result<OptionalEntry>;
    }
}

pub struct Operator(od::BlockingOperator);

fn new_operator(scheme: &str, configs: Vec<ffi::HashMapValue>) -> Result<Box<Operator>> {
    let scheme = od::Scheme::from_str(scheme)?;

    let map: HashMap<String, String> = configs
        .into_iter()
        .map(|value| (value.key, value.value))
        .collect();

    let op = Box::new(Operator(od::Operator::via_iter(scheme, map)?.blocking()));

    Ok(op)
}

impl Operator {
    fn read(&self, path: &str) -> Result<Vec<u8>> {
        Ok(self.0.read(path)?.to_vec())
    }

    // To avoid copying the bytes, we use &'static [u8] here.
    //
    // Safety: The bytes created from bs will be dropped after the function call.
    // So it's safe to declare its lifetime as 'static.
    fn write(&self, path: &str, bs: &'static [u8]) -> Result<()> {
        Ok(self.0.write(path, bs).map(|_| ())?)
    }

    fn exists(&self, path: &str) -> Result<bool> {
        Ok(self.0.exists(path)?)
    }

    fn create_dir(&self, path: &str) -> Result<()> {
        Ok(self.0.create_dir(path)?)
    }

    fn copy(&self, src: &str, dst: &str) -> Result<()> {
        Ok(self.0.copy(src, dst)?)
    }

    fn rename(&self, src: &str, dst: &str) -> Result<()> {
        Ok(self.0.rename(src, dst)?)
    }

    // We can't name it to delete because it's a keyword in C++
    fn remove(&self, path: &str) -> Result<()> {
        Ok(self.0.delete(path)?)
    }

    fn stat(&self, path: &str) -> Result<ffi::Metadata> {
        Ok(self.0.stat(path)?.into())
    }

    fn list(&self, path: &str) -> Result<Vec<ffi::Entry>> {
        Ok(self.0.list(path)?.into_iter().map(Into::into).collect())
    }

    fn reader(&self, path: &str) -> Result<Box<Reader>> {
        let meta = self.0.stat(path)?;
        Ok(Box::new(Reader(
            self.0
                .reader(path)?
                .into_std_read(0..meta.content_length())?,
        )))
    }

    fn lister(&self, path: &str) -> Result<Box<Lister>> {
        Ok(Box::new(Lister(self.0.lister(path)?)))
    }
}
