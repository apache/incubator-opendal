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
mod layer;
mod lister;
mod reader;
mod types;
mod writer;

#[cfg(feature = "testing")]
mod test_support;

pub use layer::LayerBuilder;

use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::Result;
use lister::Lister;
use od::raw::Timestamp;
use opendal as od;
use reader::Reader;
use writer::Writer;

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

#[cxx::bridge(namespace = "opendal::ffi")]
mod ffi {
    struct HashMapValue {
        key: String,
        value: String,
    }

    struct OptionalU64 {
        has_value: bool,
        value: u64,
    }

    struct OptionalUsize {
        has_value: bool,
        value: usize,
    }

    struct OptionalTimestamp {
        has_value: bool,
        seconds: i64,
        nanoseconds: u32,
    }

    struct FfiBytesRange {
        kind: u8,
        start: u64,
        end: u64,
    }

    struct FfiStatOptions {
        version: OptionalString,
        if_match: OptionalString,
        if_none_match: OptionalString,
        if_modified_since: OptionalTimestamp,
        if_unmodified_since: OptionalTimestamp,
        override_content_type: OptionalString,
        override_cache_control: OptionalString,
        override_content_disposition: OptionalString,
    }

    struct FfiReadOptions {
        range: FfiBytesRange,
        version: OptionalString,
        if_match: OptionalString,
        if_none_match: OptionalString,
        if_modified_since: OptionalTimestamp,
        if_unmodified_since: OptionalTimestamp,
        content_length_hint: OptionalU64,
        concurrent: usize,
        chunk: OptionalUsize,
        gap: OptionalUsize,
        override_content_type: OptionalString,
        override_cache_control: OptionalString,
        override_content_disposition: OptionalString,
    }

    struct FfiReaderOptions {
        version: OptionalString,
        if_match: OptionalString,
        if_none_match: OptionalString,
        if_modified_since: OptionalTimestamp,
        if_unmodified_since: OptionalTimestamp,
        content_length_hint: OptionalU64,
        concurrent: usize,
        chunk: OptionalUsize,
        gap: OptionalUsize,
        prefetch: usize,
    }

    struct FfiWriteOptions {
        append: bool,
        cache_control: OptionalString,
        content_type: OptionalString,
        content_disposition: OptionalString,
        content_encoding: OptionalString,
        has_user_metadata: bool,
        user_metadata: Vec<HashMapValue>,
        if_match: OptionalString,
        if_none_match: OptionalString,
        if_not_exists: bool,
        concurrent: usize,
        chunk: OptionalUsize,
    }

    struct FfiCopyOptions {
        if_not_exists: bool,
        if_match: OptionalString,
        source_version: OptionalString,
        source_content_length_hint: OptionalU64,
        concurrent: usize,
        chunk: OptionalUsize,
    }

    struct FfiRenameOptions {
        if_not_exists: bool,
    }

    struct FfiDeleteOptions {
        version: OptionalString,
        recursive: bool,
    }

    struct FfiListOptions {
        limit: OptionalUsize,
        start_after: OptionalString,
        recursive: bool,
        versions: bool,
        deleted: bool,
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

    struct OptionalBool {
        has_value: bool,
        value: bool,
    }

    struct OptionalEntry {
        has_value: bool,
        value: Entry,
    }

    struct Capability {
        stat: bool,
        stat_with_if_match: bool,
        stat_with_if_none_match: bool,
        read: bool,
        read_with_if_match: bool,
        read_with_if_none_match: bool,
        read_with_override_cache_control: bool,
        read_with_override_content_disposition: bool,
        read_with_override_content_type: bool,
        write: bool,
        write_can_multi: bool,
        write_can_empty: bool,
        write_can_append: bool,
        write_with_content_type: bool,
        write_with_content_disposition: bool,
        write_with_cache_control: bool,
        write_multi_max_size: usize,
        write_multi_min_size: usize,
        write_total_max_size: usize,
        create_dir: bool,
        delete_feature: bool,
        copy: bool,
        rename: bool,
        list: bool,
        list_with_limit: bool,
        list_with_start_after: bool,
        list_with_recursive: bool,
        presign: bool,
        presign_read: bool,
        presign_stat: bool,
        presign_write: bool,
        shared: bool,
    }

    struct Metadata {
        mode: EntryMode,
        content_length: u64,
        cache_control: OptionalString,
        content_disposition: OptionalString,
        content_md5: OptionalString,
        content_type: OptionalString,
        content_encoding: OptionalString,
        etag: OptionalString,
        last_modified: OptionalString,
        version: OptionalString,
        is_current: OptionalBool,
        is_deleted: bool,
        // Note: content_range and user_metadata are complex types that need special handling
    }

    struct Entry {
        path: String,
    }

    extern "Rust" {
        type Operator;
        type Reader;
        type Lister;
        type Writer;
        type LayerBuilder;

        fn layer_builder_new() -> Box<LayerBuilder>;
        fn add_timeout(self: &mut LayerBuilder, timeout_ns: u64, io_timeout_ns: u64);
        fn add_retry(
            self: &mut LayerBuilder,
            jitter: bool,
            factor: f32,
            min_delay_ns: u64,
            max_delay_ns: u64,
            max_times: u64,
        );

        // cxx::UniquePtr isn't yet supported in stable Rust. We'll adopt it once support becomes available
        fn new_operator(
            scheme: &str,
            configs: Vec<HashMapValue>,
            layers: &LayerBuilder,
        ) -> Result<*mut Operator>;
        unsafe fn delete_operator(op: *mut Operator);

        fn read(self: &Operator, path: &str) -> Result<Vec<u8>>;
        fn read_options(self: &Operator, path: &str, opts: FfiReadOptions) -> Result<Vec<u8>>;
        fn write(self: &Operator, path: &str, bs: &[u8]) -> Result<()>;
        fn write_options(
            self: &Operator,
            path: &str,
            bs: &[u8],
            opts: FfiWriteOptions,
        ) -> Result<()>;
        fn exists(self: &Operator, path: &str) -> Result<bool>;
        fn create_dir(self: &Operator, path: &str) -> Result<()>;
        fn copy(self: &Operator, src: &str, dst: &str) -> Result<()>;
        fn copy_options(self: &Operator, src: &str, dst: &str, opts: FfiCopyOptions) -> Result<()>;
        fn rename(self: &Operator, src: &str, dst: &str) -> Result<()>;
        fn rename_options(
            self: &Operator,
            src: &str,
            dst: &str,
            opts: FfiRenameOptions,
        ) -> Result<()>;
        fn remove(self: &Operator, path: &str) -> Result<()>;
        fn remove_options(self: &Operator, path: &str, opts: FfiDeleteOptions) -> Result<()>;
        fn remove_all(self: &Operator, paths: Vec<String>) -> Result<()>;
        fn stat(self: &Operator, path: &str) -> Result<Metadata>;
        fn stat_options(self: &Operator, path: &str, opts: FfiStatOptions) -> Result<Metadata>;
        fn list(self: &Operator, path: &str) -> Result<Vec<Entry>>;
        fn list_options(self: &Operator, path: &str, opts: FfiListOptions) -> Result<Vec<Entry>>;
        fn reader(self: &Operator, path: &str) -> Result<*mut Reader>;
        fn reader_options(
            self: &Operator,
            path: &str,
            opts: FfiReaderOptions,
        ) -> Result<*mut Reader>;
        fn lister(self: &Operator, path: &str) -> Result<*mut Lister>;
        fn lister_options(self: &Operator, path: &str, opts: FfiListOptions)
        -> Result<*mut Lister>;
        fn writer(self: &Operator, path: &str) -> Result<*mut Writer>;
        fn writer_options(
            self: &Operator,
            path: &str,
            opts: FfiWriteOptions,
        ) -> Result<*mut Writer>;
        fn info(self: &Operator) -> Result<Capability>;

        unsafe fn delete_reader(reader: *mut Reader);
        fn read(self: &mut Reader, buf: &mut [u8]) -> Result<usize>;
        fn read_at(self: &Reader, buf: &mut [u8], offset: u64) -> Result<usize>;
        fn seek(self: &mut Reader, offset: i64, dir: SeekFrom) -> Result<u64>;

        unsafe fn delete_writer(writer: *mut Writer);
        fn write(self: &mut Writer, bs: &[u8]) -> Result<()>;
        fn flush(self: &mut Writer) -> Result<()>;
        fn close(self: &mut Writer) -> Result<()>;

        unsafe fn delete_lister(lister: *mut Lister);
        fn next(self: &mut Lister) -> Result<OptionalEntry>;

        #[cfg(feature = "testing")]
        fn new_test_hanging_operator(layers: &LayerBuilder) -> Result<*mut Operator>;
        #[cfg(feature = "testing")]
        fn new_test_retryable_operator(layers: &LayerBuilder) -> Result<*mut Operator>;
        #[cfg(feature = "testing")]
        fn test_retryable_attempt_count() -> u64;
    }
}

pub struct Operator(od::blocking::Operator);

fn into_blocking_operator(op: od::Operator) -> Result<Operator> {
    let runtime =
        tokio::runtime::Handle::try_current().unwrap_or_else(|_| RUNTIME.handle().clone());
    let _guard = runtime.enter();
    Ok(Operator(od::blocking::Operator::new(op)?))
}

fn layer_builder_new() -> Box<LayerBuilder> {
    Box::new(LayerBuilder::new())
}

fn new_operator(
    scheme: &str,
    configs: Vec<ffi::HashMapValue>,
    layers: &LayerBuilder,
) -> Result<*mut Operator> {
    let map: HashMap<String, String> = configs
        .into_iter()
        .map(|value| (value.key, value.value))
        .collect();

    od::init_default_registry();

    let op = od::Operator::via_iter(scheme, map)?;
    let op = layers.apply(op);
    let op = into_blocking_operator(op)?;

    let op = Box::into_raw(Box::new(op));

    Ok(op)
}

#[cfg(feature = "testing")]
fn new_test_hanging_operator(layers: &LayerBuilder) -> Result<*mut Operator> {
    test_support::new_hanging_operator(layers)
}

#[cfg(feature = "testing")]
fn new_test_retryable_operator(layers: &LayerBuilder) -> Result<*mut Operator> {
    test_support::new_retryable_operator(layers)
}

#[cfg(feature = "testing")]
fn test_retryable_attempt_count() -> u64 {
    test_support::retryable_attempt_count() as u64
}

unsafe fn delete_operator(op: *mut Operator) {
    if !op.is_null() {
        unsafe {
            drop(Box::from_raw(op));
        }
    }
}

unsafe fn delete_reader(reader: *mut Reader) {
    if !reader.is_null() {
        unsafe {
            drop(Box::from_raw(reader));
        }
    }
}

unsafe fn delete_writer(writer: *mut Writer) {
    if !writer.is_null() {
        unsafe {
            drop(Box::from_raw(writer));
        }
    }
}

unsafe fn delete_lister(lister: *mut Lister) {
    if !lister.is_null() {
        unsafe {
            drop(Box::from_raw(lister));
        }
    }
}

fn optional_string(v: ffi::OptionalString) -> Option<String> {
    v.has_value.then_some(v.value)
}

fn optional_u64(v: ffi::OptionalU64) -> Option<u64> {
    v.has_value.then_some(v.value)
}

fn optional_usize(v: ffi::OptionalUsize) -> Option<usize> {
    v.has_value.then_some(v.value)
}

fn optional_timestamp(v: ffi::OptionalTimestamp) -> Result<Option<Timestamp>> {
    if v.has_value {
        Ok(Some(Timestamp::new(v.seconds, v.nanoseconds as i32)?))
    } else {
        Ok(None)
    }
}

fn bytes_range(v: ffi::FfiBytesRange) -> od::BytesRange {
    match v.kind {
        1 => od::BytesRange::new(v.start, None),
        2 => od::BytesRange::new(v.start, Some(v.end.saturating_sub(v.start))),
        3 => od::BytesRange::suffix(v.end),
        _ => od::BytesRange::default(),
    }
}

fn user_metadata(
    has_value: bool,
    values: Vec<ffi::HashMapValue>,
) -> Option<HashMap<String, String>> {
    has_value.then(|| {
        values
            .into_iter()
            .map(|value| (value.key, value.value))
            .collect()
    })
}

fn stat_options(opts: ffi::FfiStatOptions) -> Result<od::options::StatOptions> {
    Ok(od::options::StatOptions {
        version: optional_string(opts.version),
        if_match: optional_string(opts.if_match),
        if_none_match: optional_string(opts.if_none_match),
        if_modified_since: optional_timestamp(opts.if_modified_since)?,
        if_unmodified_since: optional_timestamp(opts.if_unmodified_since)?,
        override_content_type: optional_string(opts.override_content_type),
        override_cache_control: optional_string(opts.override_cache_control),
        override_content_disposition: optional_string(opts.override_content_disposition),
        ..Default::default()
    })
}

fn read_options(opts: ffi::FfiReadOptions) -> Result<od::options::ReadOptions> {
    Ok(od::options::ReadOptions {
        range: bytes_range(opts.range),
        version: optional_string(opts.version),
        if_match: optional_string(opts.if_match),
        if_none_match: optional_string(opts.if_none_match),
        if_modified_since: optional_timestamp(opts.if_modified_since)?,
        if_unmodified_since: optional_timestamp(opts.if_unmodified_since)?,
        content_length_hint: optional_u64(opts.content_length_hint),
        concurrent: opts.concurrent,
        chunk: optional_usize(opts.chunk),
        gap: optional_usize(opts.gap),
        override_content_type: optional_string(opts.override_content_type),
        override_cache_control: optional_string(opts.override_cache_control),
        override_content_disposition: optional_string(opts.override_content_disposition),
        ..Default::default()
    })
}

fn reader_options(opts: ffi::FfiReaderOptions) -> Result<od::options::ReaderOptions> {
    Ok(od::options::ReaderOptions {
        version: optional_string(opts.version),
        if_match: optional_string(opts.if_match),
        if_none_match: optional_string(opts.if_none_match),
        if_modified_since: optional_timestamp(opts.if_modified_since)?,
        if_unmodified_since: optional_timestamp(opts.if_unmodified_since)?,
        content_length_hint: optional_u64(opts.content_length_hint),
        concurrent: opts.concurrent,
        chunk: optional_usize(opts.chunk),
        gap: optional_usize(opts.gap),
        prefetch: opts.prefetch,
        ..Default::default()
    })
}

fn write_options(opts: ffi::FfiWriteOptions) -> od::options::WriteOptions {
    od::options::WriteOptions {
        append: opts.append,
        cache_control: optional_string(opts.cache_control),
        content_type: optional_string(opts.content_type),
        content_disposition: optional_string(opts.content_disposition),
        content_encoding: optional_string(opts.content_encoding),
        user_metadata: user_metadata(opts.has_user_metadata, opts.user_metadata),
        if_match: optional_string(opts.if_match),
        if_none_match: optional_string(opts.if_none_match),
        if_not_exists: opts.if_not_exists,
        concurrent: opts.concurrent,
        chunk: optional_usize(opts.chunk),
        ..Default::default()
    }
}

fn copy_options(opts: ffi::FfiCopyOptions) -> od::options::CopyOptions {
    od::options::CopyOptions {
        if_not_exists: opts.if_not_exists,
        if_match: optional_string(opts.if_match),
        source_version: optional_string(opts.source_version),
        source_content_length_hint: optional_u64(opts.source_content_length_hint),
        concurrent: opts.concurrent,
        chunk: optional_usize(opts.chunk),
        ..Default::default()
    }
}

fn rename_options(opts: ffi::FfiRenameOptions) -> od::options::RenameOptions {
    od::options::RenameOptions {
        if_not_exists: opts.if_not_exists,
    }
}

fn delete_options(opts: ffi::FfiDeleteOptions) -> od::options::DeleteOptions {
    od::options::DeleteOptions {
        version: optional_string(opts.version),
        recursive: opts.recursive,
        if_match: None,
        ..Default::default()
    }
}

fn list_options(opts: ffi::FfiListOptions) -> od::options::ListOptions {
    od::options::ListOptions {
        limit: optional_usize(opts.limit),
        start_after: optional_string(opts.start_after),
        recursive: opts.recursive,
        versions: opts.versions,
        deleted: opts.deleted,
    }
}

impl Operator {
    fn read(&self, path: &str) -> Result<Vec<u8>> {
        Ok(self.0.read(path)?.to_vec())
    }

    fn read_options(&self, path: &str, opts: ffi::FfiReadOptions) -> Result<Vec<u8>> {
        Ok(self.0.read_options(path, read_options(opts)?)?.to_vec())
    }

    fn write(&self, path: &str, bs: &[u8]) -> Result<()> {
        Ok(self.0.write(path, bs.to_vec()).map(|_| ())?)
    }

    fn write_options(&self, path: &str, bs: &[u8], opts: ffi::FfiWriteOptions) -> Result<()> {
        Ok(self
            .0
            .write_options(path, bs.to_vec(), write_options(opts))
            .map(|_| ())?)
    }

    fn exists(&self, path: &str) -> Result<bool> {
        Ok(self.0.exists(path)?)
    }

    fn create_dir(&self, path: &str) -> Result<()> {
        Ok(self.0.create_dir(path)?)
    }

    fn copy(&self, src: &str, dst: &str) -> Result<()> {
        self.0.copy(src, dst)?;
        Ok(())
    }

    fn copy_options(&self, src: &str, dst: &str, opts: ffi::FfiCopyOptions) -> Result<()> {
        self.0.copy_options(src, dst, copy_options(opts))?;
        Ok(())
    }

    fn rename(&self, src: &str, dst: &str) -> Result<()> {
        Ok(self.0.rename(src, dst)?)
    }

    fn rename_options(&self, src: &str, dst: &str, opts: ffi::FfiRenameOptions) -> Result<()> {
        Ok(self.0.rename_options(src, dst, rename_options(opts))?)
    }

    // We can't name it to delete because it's a keyword in C++
    fn remove(&self, path: &str) -> Result<()> {
        Ok(self.0.delete(path)?)
    }

    fn remove_options(&self, path: &str, opts: ffi::FfiDeleteOptions) -> Result<()> {
        Ok(self.0.delete_options(path, delete_options(opts))?)
    }

    fn remove_all(&self, paths: Vec<String>) -> Result<()> {
        Ok(self.0.delete_iter(paths)?)
    }

    fn stat(&self, path: &str) -> Result<ffi::Metadata> {
        Ok(self.0.stat(path)?.into())
    }

    fn stat_options(&self, path: &str, opts: ffi::FfiStatOptions) -> Result<ffi::Metadata> {
        Ok(self.0.stat_options(path, stat_options(opts)?)?.into())
    }

    fn list(&self, path: &str) -> Result<Vec<ffi::Entry>> {
        Ok(self.0.list(path)?.into_iter().map(Into::into).collect())
    }

    fn list_options(&self, path: &str, opts: ffi::FfiListOptions) -> Result<Vec<ffi::Entry>> {
        Ok(self
            .0
            .list_options(path, list_options(opts))?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn reader(&self, path: &str) -> Result<*mut Reader> {
        let meta = self.0.stat(path)?;
        let reader = Box::into_raw(Box::new(Reader::new(
            self.0.reader(path)?,
            meta.content_length(),
        )));
        Ok(reader)
    }

    fn reader_options(&self, path: &str, opts: ffi::FfiReaderOptions) -> Result<*mut Reader> {
        let opts = reader_options(opts)?;
        let content_length = match opts.content_length_hint {
            Some(v) => v,
            None => {
                let stat_opts = od::options::StatOptions {
                    version: opts.version.clone(),
                    if_match: opts.if_match.clone(),
                    if_none_match: opts.if_none_match.clone(),
                    if_modified_since: opts.if_modified_since,
                    if_unmodified_since: opts.if_unmodified_since,
                    ..Default::default()
                };
                self.0.stat_options(path, stat_opts)?.content_length()
            }
        };
        let reader = Box::into_raw(Box::new(Reader::new(
            self.0.reader_options(path, opts)?,
            content_length,
        )));
        Ok(reader)
    }

    fn lister(&self, path: &str) -> Result<*mut Lister> {
        let lister = Box::into_raw(Box::new(Lister(self.0.lister(path)?)));
        Ok(lister)
    }

    fn lister_options(&self, path: &str, opts: ffi::FfiListOptions) -> Result<*mut Lister> {
        let lister = Box::into_raw(Box::new(Lister(
            self.0.lister_options(path, list_options(opts))?,
        )));
        Ok(lister)
    }

    fn writer(&self, path: &str) -> Result<*mut Writer> {
        let writer = Box::into_raw(Box::new(Writer(self.0.writer(path)?)));
        Ok(writer)
    }

    fn writer_options(&self, path: &str, opts: ffi::FfiWriteOptions) -> Result<*mut Writer> {
        let writer = Box::into_raw(Box::new(Writer(
            self.0.writer_options(path, write_options(opts))?,
        )));
        Ok(writer)
    }

    fn info(&self) -> Result<ffi::Capability> {
        let info = self.0.info();
        let cap = info.capability();

        Ok(ffi::Capability {
            stat: cap.stat,
            stat_with_if_match: cap.stat_with_if_match,
            stat_with_if_none_match: cap.stat_with_if_none_match,
            read: cap.read,
            read_with_if_match: cap.read_with_if_match,
            read_with_if_none_match: cap.read_with_if_none_match,
            read_with_override_cache_control: cap.read_with_override_cache_control,
            read_with_override_content_disposition: cap.read_with_override_content_disposition,
            read_with_override_content_type: cap.read_with_override_content_type,
            write: cap.write,
            write_can_multi: cap.write_can_multi,
            write_can_empty: cap.write_can_empty,
            write_can_append: cap.write_can_append,
            write_with_content_type: cap.write_with_content_type,
            write_with_content_disposition: cap.write_with_content_disposition,
            write_with_cache_control: cap.write_with_cache_control,
            write_multi_max_size: cap.write_multi_max_size.unwrap_or(0),
            write_multi_min_size: cap.write_multi_min_size.unwrap_or(0),
            write_total_max_size: cap.write_total_max_size.unwrap_or(0),
            create_dir: cap.create_dir,
            delete_feature: cap.delete,
            copy: cap.copy,
            rename: cap.rename,
            list: cap.list,
            list_with_limit: cap.list_with_limit,
            list_with_start_after: cap.list_with_start_after,
            list_with_recursive: cap.list_with_recursive,
            presign: cap.presign,
            presign_read: cap.presign_read,
            presign_stat: cap.presign_stat,
            presign_write: cap.presign_write,
            shared: cap.shared,
        })
    }
}
