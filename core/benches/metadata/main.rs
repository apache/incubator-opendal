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
use std::mem;

use divan::AllocProfiler;
use divan::Bencher;
use divan::black_box;
use divan::black_box_drop;
use opendal::Capability;
use opendal::EntryMode;
use opendal::Metadata;
use opendal::MetadataBuilder;
use opendal::options::ReadOptions;
use opendal::options::WriteOptions;
use opendal::raw::OpRead;
use opendal::raw::OpWrite;
use opendal::raw::Timestamp;
use opendal::raw::oio::Entry;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

const CACHE_CONTROL: &str = "public, max-age=3600";
const CONTENT_DISPOSITION: &str = "attachment; filename=benchmark-object.json";
const CONTENT_MD5: &str = "1B2M2Y8AsgTpgAmY7PhCfg==";
const CONTENT_TYPE: &str = "application/json";
const CONTENT_ENCODING: &str = "gzip";
const ETAG: &str = "\"33a64df551425fcc55e4d42a148795d9f25f89d4\"";
const UPDATED_ETAG: &str = "\"b1946ac92492d2347c6235b4d2611184\"";
const VERSION: &str = "3/L4kqtJlcpXroDTDmJ+rmSpXd3dIbrHYiHf91CDq0";
const PATH: &str = "benchmarks/metadata/representative-object.json";

fn main() {
    println!(
        "layout: metadata legacy={} compact={} entry legacy={} compact={}",
        mem::size_of::<LegacyMetadata>(),
        mem::size_of::<Metadata>(),
        mem::size_of::<LegacyEntry>(),
        mem::size_of::<Entry>(),
    );
    println!(
        "raw layout: read legacy={} compact={} write legacy={} compact={}",
        mem::size_of::<LegacyOpRead>(),
        mem::size_of::<OpRead>(),
        mem::size_of::<LegacyOpWrite>(),
        mem::size_of::<OpWrite>(),
    );
    divan::main();
}

#[derive(Clone, Copy, Debug)]
enum Profile {
    List,
    Stat,
    UserMetadata,
}

const PROFILES: &[Profile] = &[Profile::List, Profile::Stat, Profile::UserMetadata];

#[allow(dead_code)]
#[derive(Clone, Default)]
struct LegacyMetadata {
    mode: EntryMode,
    is_current: Option<bool>,
    is_deleted: bool,
    cache_control: Option<String>,
    content_disposition: Option<String>,
    content_length: Option<u64>,
    content_md5: Option<String>,
    content_type: Option<String>,
    content_encoding: Option<String>,
    etag: Option<String>,
    last_modified: Option<Timestamp>,
    version: Option<String>,
    user_metadata: Option<HashMap<String, String>>,
}

#[allow(dead_code)]
struct LegacyEntry {
    path: String,
    metadata: LegacyMetadata,
}

#[allow(dead_code)]
#[derive(Clone, Default)]
struct LegacyOpRead {
    if_match: Option<String>,
    if_none_match: Option<String>,
    if_version_match: Option<String>,
    if_version_not_match: Option<String>,
    if_modified_since: Option<Timestamp>,
    if_unmodified_since: Option<Timestamp>,
    override_content_type: Option<String>,
    override_cache_control: Option<String>,
    override_content_disposition: Option<String>,
    version: Option<String>,
    content_length_hint: Option<u64>,
}

impl From<ReadOptions> for LegacyOpRead {
    fn from(value: ReadOptions) -> Self {
        Self {
            if_match: value.if_match,
            if_none_match: value.if_none_match,
            if_version_match: value.if_version_match,
            if_version_not_match: value.if_version_not_match,
            if_modified_since: value.if_modified_since,
            if_unmodified_since: value.if_unmodified_since,
            override_content_type: value.override_content_type,
            override_cache_control: value.override_cache_control,
            override_content_disposition: value.override_content_disposition,
            version: value.version,
            content_length_hint: value.content_length_hint,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Default)]
struct LegacyOpWrite {
    append: bool,
    concurrent: usize,
    content_type: Option<String>,
    content_disposition: Option<String>,
    content_encoding: Option<String>,
    cache_control: Option<String>,
    if_match: Option<String>,
    if_none_match: Option<String>,
    if_version_match: Option<String>,
    if_version_not_match: Option<String>,
    if_not_exists: bool,
    user_metadata: Option<HashMap<String, String>>,
}

impl From<WriteOptions> for LegacyOpWrite {
    fn from(value: WriteOptions) -> Self {
        Self {
            append: value.append,
            concurrent: value.concurrent,
            content_type: value.content_type,
            content_disposition: value.content_disposition,
            content_encoding: value.content_encoding,
            cache_control: value.cache_control,
            if_match: value.if_match,
            if_none_match: value.if_none_match,
            if_version_match: value.if_version_match,
            if_version_not_match: value.if_version_not_match,
            if_not_exists: value.if_not_exists,
            user_metadata: value.user_metadata,
        }
    }
}

fn timestamp() -> Timestamp {
    Timestamp::new(1_700_000_000, 123_456_789).expect("timestamp is valid")
}

fn make_user_metadata() -> HashMap<String, String> {
    HashMap::from([
        (
            "request-id".to_string(),
            "01HZX3K8R4N8YV6YQ5B6F3S2T1".to_string(),
        ),
        ("tenant".to_string(), "benchmark-tenant".to_string()),
        (
            "trace-id".to_string(),
            "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
        ),
        ("region".to_string(), "us-west-2".to_string()),
        ("pipeline".to_string(), "metadata-retention".to_string()),
        ("source".to_string(), "benchmark".to_string()),
        ("schema".to_string(), "v3".to_string()),
        ("owner".to_string(), "opendal".to_string()),
    ])
}

fn make_legacy(profile: Profile) -> LegacyMetadata {
    let mut metadata = LegacyMetadata {
        mode: EntryMode::FILE,
        content_length: Some(4 * 1024 * 1024),
        etag: Some(ETAG.to_string()),
        last_modified: Some(timestamp()),
        ..Default::default()
    };

    if matches!(profile, Profile::Stat | Profile::UserMetadata) {
        metadata.cache_control = Some(CACHE_CONTROL.to_string());
        metadata.content_disposition = Some(CONTENT_DISPOSITION.to_string());
        metadata.content_md5 = Some(CONTENT_MD5.to_string());
        metadata.content_type = Some(CONTENT_TYPE.to_string());
        metadata.content_encoding = Some(CONTENT_ENCODING.to_string());
        metadata.version = Some(VERSION.to_string());
    }
    if matches!(profile, Profile::UserMetadata) {
        metadata.user_metadata = Some(make_user_metadata());
    }
    metadata
}

fn make_compact(profile: Profile) -> Metadata {
    let mut metadata = MetadataBuilder::file(4 * 1024 * 1024);
    metadata.etag(ETAG).last_modified(timestamp());

    if matches!(profile, Profile::Stat | Profile::UserMetadata) {
        metadata
            .cache_control(CACHE_CONTROL)
            .content_disposition(CONTENT_DISPOSITION)
            .content_md5(CONTENT_MD5)
            .content_type(CONTENT_TYPE)
            .content_encoding(CONTENT_ENCODING)
            .version(VERSION);
    }
    if matches!(profile, Profile::UserMetadata) {
        metadata.user_metadata(make_user_metadata());
    }
    metadata.build()
}

fn make_read_options() -> ReadOptions {
    ReadOptions {
        version: Some(VERSION.to_string()),
        if_match: Some(ETAG.to_string()),
        if_none_match: Some(UPDATED_ETAG.to_string()),
        if_version_match: Some("generation-42".to_string()),
        if_version_not_match: Some("generation-41".to_string()),
        if_modified_since: Some(timestamp()),
        if_unmodified_since: Some(timestamp()),
        content_length_hint: Some(4 * 1024 * 1024),
        override_content_type: Some(CONTENT_TYPE.to_string()),
        override_cache_control: Some(CACHE_CONTROL.to_string()),
        override_content_disposition: Some(CONTENT_DISPOSITION.to_string()),
        ..Default::default()
    }
}

fn make_write_options() -> WriteOptions {
    WriteOptions {
        append: true,
        concurrent: 8,
        content_type: Some(CONTENT_TYPE.to_string()),
        content_disposition: Some(CONTENT_DISPOSITION.to_string()),
        content_encoding: Some(CONTENT_ENCODING.to_string()),
        cache_control: Some(CACHE_CONTROL.to_string()),
        if_match: Some(ETAG.to_string()),
        if_none_match: Some(UPDATED_ETAG.to_string()),
        if_version_match: Some("generation-42".to_string()),
        if_version_not_match: Some("generation-41".to_string()),
        if_not_exists: true,
        user_metadata: Some(make_user_metadata()),
        ..Default::default()
    }
}

fn make_op_read() -> OpRead {
    let (_, args, _) = make_read_options().into();
    args
}

fn make_op_write() -> OpWrite {
    OpWrite::from_options(&Capability::default(), make_write_options())
        .expect("options do not contain a logical condition")
        .0
}

mod construction {
    use super::*;

    #[divan::bench(args = PROFILES)]
    fn legacy(bencher: Bencher, profile: Profile) {
        bencher.bench(|| black_box(make_legacy(profile)));
    }

    #[divan::bench(args = PROFILES)]
    fn compact(bencher: Bencher, profile: Profile) {
        bencher.bench(|| black_box(make_compact(profile)));
    }
}

mod retain_10k {
    use super::*;

    const ENTRIES: usize = 10_000;

    #[divan::bench(args = PROFILES)]
    fn legacy(bencher: Bencher, profile: Profile) {
        bencher.bench(|| {
            black_box(
                (0..ENTRIES)
                    .map(|_| make_legacy(profile))
                    .collect::<Vec<_>>(),
            )
        });
    }

    #[divan::bench(args = PROFILES)]
    fn compact(bencher: Bencher, profile: Profile) {
        bencher.bench(|| {
            black_box(
                (0..ENTRIES)
                    .map(|_| make_compact(profile))
                    .collect::<Vec<_>>(),
            )
        });
    }
}

mod retain_10k_entries {
    use super::*;

    const ENTRIES: usize = 10_000;

    #[divan::bench]
    fn legacy(bencher: Bencher) {
        bencher.bench(|| {
            black_box(
                (0..ENTRIES)
                    .map(|_| LegacyEntry {
                        path: PATH.to_string(),
                        metadata: make_legacy(Profile::List),
                    })
                    .collect::<Vec<_>>(),
            )
        });
    }

    #[divan::bench]
    fn compact(bencher: Bencher) {
        bencher.bench(|| {
            black_box(
                (0..ENTRIES)
                    .map(|_| Entry::new(PATH, make_compact(Profile::List)))
                    .collect::<Vec<_>>(),
            )
        });
    }
}

mod lookup {
    use super::*;

    #[divan::bench]
    fn legacy_string(bencher: Bencher) {
        bencher
            .with_inputs(|| make_legacy(Profile::Stat))
            .bench_refs(|metadata| black_box(metadata.etag.as_deref().map(str::len)));
    }

    #[divan::bench]
    fn compact_string(bencher: Bencher) {
        bencher
            .with_inputs(|| make_compact(Profile::Stat))
            .bench_refs(|metadata| black_box(metadata.etag().map(str::len)));
    }

    #[divan::bench]
    fn legacy_user_metadata(bencher: Bencher) {
        bencher
            .with_inputs(|| make_legacy(Profile::UserMetadata))
            .bench_refs(|metadata| {
                black_box(
                    metadata
                        .user_metadata
                        .as_ref()
                        .and_then(|values| values.get("request-id"))
                        .map(String::len),
                )
            });
    }

    #[divan::bench]
    fn compact_user_metadata(bencher: Bencher) {
        bencher
            .with_inputs(|| make_compact(Profile::UserMetadata))
            .bench_refs(|metadata| {
                black_box(
                    metadata
                        .user_metadata()
                        .and_then(|values| values.get("request-id"))
                        .map(str::len),
                )
            });
    }
}

mod clone {
    use super::*;

    #[divan::bench(args = PROFILES)]
    fn legacy(bencher: Bencher, profile: Profile) {
        bencher
            .with_inputs(|| make_legacy(profile))
            .bench_refs(|metadata| black_box_drop(metadata.clone()));
    }

    #[divan::bench(args = PROFILES)]
    fn compact(bencher: Bencher, profile: Profile) {
        bencher
            .with_inputs(|| make_compact(profile))
            .bench_refs(|metadata| black_box_drop(metadata.clone()));
    }
}

mod clone_then_modify {
    use super::*;

    #[divan::bench(args = PROFILES)]
    fn legacy(bencher: Bencher, profile: Profile) {
        bencher
            .with_inputs(|| make_legacy(profile))
            .bench_refs(|metadata| {
                let mut metadata = metadata.clone();
                metadata.etag = Some(UPDATED_ETAG.to_string());
                black_box_drop(metadata);
            });
    }

    #[divan::bench(args = PROFILES)]
    fn compact(bencher: Bencher, profile: Profile) {
        bencher
            .with_inputs(|| make_compact(profile))
            .bench_refs(|metadata| {
                let mut builder = metadata.clone().into_builder();
                builder.etag(UPDATED_ETAG);
                black_box_drop(builder.build());
            });
    }
}

mod raw_freeze {
    use super::*;

    #[divan::bench]
    fn legacy_read(bencher: Bencher) {
        bencher
            .with_inputs(make_read_options)
            .bench_values(|options| black_box(LegacyOpRead::from(options)));
    }

    #[divan::bench]
    fn compact_read(bencher: Bencher) {
        bencher
            .with_inputs(make_read_options)
            .bench_values(|options| {
                let (_, args, _): (_, OpRead, _) = options.into();
                black_box(args)
            });
    }

    #[divan::bench]
    fn legacy_write(bencher: Bencher) {
        bencher
            .with_inputs(make_write_options)
            .bench_values(|options| black_box(LegacyOpWrite::from(options)));
    }

    #[divan::bench]
    fn compact_write(bencher: Bencher) {
        bencher
            .with_inputs(make_write_options)
            .bench_values(|options| {
                black_box(
                    OpWrite::from_options(&Capability::default(), options)
                        .expect("options do not contain a logical condition")
                        .0,
                )
            });
    }
}

mod raw_clone {
    use super::*;

    #[divan::bench]
    fn legacy_read(bencher: Bencher) {
        bencher
            .with_inputs(|| LegacyOpRead::from(make_read_options()))
            .bench_refs(|args| black_box_drop(args.clone()));
    }

    #[divan::bench]
    fn compact_read(bencher: Bencher) {
        bencher
            .with_inputs(make_op_read)
            .bench_refs(|args| black_box_drop(args.clone()));
    }

    #[divan::bench]
    fn legacy_write(bencher: Bencher) {
        bencher
            .with_inputs(|| LegacyOpWrite::from(make_write_options()))
            .bench_refs(|args| black_box_drop(args.clone()));
    }

    #[divan::bench]
    fn compact_write(bencher: Bencher) {
        bencher
            .with_inputs(make_op_write)
            .bench_refs(|args| black_box_drop(args.clone()));
    }
}

mod scalar_placement {
    use super::*;

    #[divan::bench]
    fn legacy(bencher: Bencher) {
        bencher.bench(|| {
            black_box(LegacyMetadata {
                mode: EntryMode::FILE,
                content_length: Some(4 * 1024 * 1024),
                last_modified: Some(timestamp()),
                ..Default::default()
            })
        });
    }

    #[divan::bench]
    fn compact(bencher: Bencher) {
        bencher.bench(|| {
            let mut metadata = MetadataBuilder::file(4 * 1024 * 1024);
            metadata.last_modified(timestamp());
            black_box(metadata.build())
        });
    }
}
