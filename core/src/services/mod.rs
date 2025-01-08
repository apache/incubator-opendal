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

//! Services will provide builders to build underlying backends.
//!
//! More ongoing services support is tracked at [opendal#5](https://github.com/apache/opendal/issues/5). Please feel free to submit issues if there are services not covered.

mod aliyun_drive;
pub use aliyun_drive::*;

mod alluxio;
pub use alluxio::*;

mod atomicserver;
pub use self::atomicserver::*;

mod azblob;
pub use azblob::*;

mod azdls;
pub use azdls::*;

mod azfile;
pub use azfile::*;

mod b2;
pub use b2::*;

mod cacache;
pub use self::cacache::*;

mod chainsafe;
pub use chainsafe::*;

mod cloudflare_kv;
pub use self::cloudflare_kv::*;

mod compfs;
pub use compfs::*;

mod cos;
pub use cos::*;

mod d1;
pub use self::d1::*;

mod dashmap;
pub use self::dashmap::*;

mod dbfs;
pub use self::dbfs::*;

mod dropbox;
pub use dropbox::*;

mod etcd;
pub use self::etcd::*;

mod foundationdb;
pub use self::foundationdb::*;

mod fs;
pub use fs::*;

mod ftp;
pub use ftp::*;

mod gcs;
pub use gcs::*;

mod gdrive;
pub use gdrive::*;

mod ghac;
pub use ghac::*;

mod github;
pub use github::*;

mod gridfs;
pub use gridfs::*;

mod hdfs;
pub use self::hdfs::*;

mod hdfs_native;
pub use hdfs_native::*;

mod http;
pub use self::http::*;

mod huggingface;
pub use huggingface::*;

mod icloud;
pub use icloud::*;

mod ipfs;
pub use self::ipfs::*;

mod ipmfs;
pub use ipmfs::*;

mod koofr;
pub use koofr::*;

mod lakefs;
pub use lakefs::*;

mod libsql;
pub use libsql::*;

mod memcached;
pub use memcached::*;

mod memory;
pub use self::memory::*;

mod mini_moka;
pub use self::mini_moka::*;

mod moka;
pub use self::moka::*;

mod mongodb;
pub use self::mongodb::*;

mod monoiofs;
pub use monoiofs::*;

mod mysql;
pub use self::mysql::*;

mod nebula_graph;
pub use nebula_graph::*;

mod obs;
pub use obs::*;

mod onedrive;
pub use onedrive::*;

mod opfs;
pub use opfs::*;

mod oss;
pub use oss::*;

mod pcloud;
pub use pcloud::*;

mod persy;
pub use self::persy::*;

mod postgresql;
pub use self::postgresql::*;

mod redb;
pub use self::redb::*;

mod redis;
pub use self::redis::*;

mod rocksdb;
pub use self::rocksdb::*;

mod s3;
pub use s3::*;

mod seafile;
pub use seafile::*;

mod sftp;
pub use sftp::*;

mod sled;
pub use self::sled::*;

mod sqlite;
pub use self::sqlite::*;

mod supabase;
pub use supabase::*;

mod surrealdb;
pub use surrealdb::*;

mod swift;
pub use self::swift::*;

mod tikv;
pub use self::tikv::*;

mod upyun;
pub use upyun::*;

mod vercel_artifacts;
pub use vercel_artifacts::*;

mod vercel_blob;
pub use vercel_blob::*;

mod webdav;
pub use webdav::*;

mod webhdfs;
pub use webhdfs::*;

mod yandex_disk;
pub use yandex_disk::*;
