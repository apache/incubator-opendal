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

use std::collections::HashSet;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

use crate::Error;

/// Services that OpenDAL supports
///
/// # Notes
///
/// - Scheme is `non_exhaustive`, new variant COULD be added at any time.
/// - New variant SHOULD be added in alphabet orders,
/// - Users MUST NOT relay on its order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Scheme {
    /// [aliyun_drive][crate::services::AliyunDrive]: Aliyun Drive services.
    AliyunDrive,
    /// [atomicserver][crate::services::Atomicserver]: Atomicserver services.
    Atomicserver,
    /// [azblob][crate::services::Azblob]: Azure Storage Blob services.
    Azblob,
    /// [Azdls][crate::services::Azdls]: Azure Data Lake Storage Gen2.
    Azdls,
    /// [B2][crate::services::B2]: Backblaze B2 Services.
    B2,
    /// [Compfs][crate::services::Compfs]: Compio fs Services.
    Compfs,
    /// [Seafile][crate::services::Seafile]: Seafile Services.
    Seafile,
    /// [Upyun][crate::services::Upyun]: Upyun Services.
    Upyun,
    /// [VercelBlob][crate::services::VercelBlob]: VercelBlob Services.
    VercelBlob,
    /// [YandexDisk][crate::services::YandexDisk]: YandexDisk Services.
    YandexDisk,
    /// [Pcloud][crate::services::Pcloud]: Pcloud Services.
    Pcloud,
    /// [Koofr][crate::services::Koofr]: Koofr Services.
    Koofr,
    /// [Chainsafe][crate::services::Chainsafe]: Chainsafe Services.
    Chainsafe,
    /// [cacache][crate::services::Cacache]: cacache backend support.
    Cacache,
    /// [cloudflare-kv][crate::services::CloudflareKv]: Cloudflare KV services.
    CloudflareKv,
    /// [cos][crate::services::Cos]: Tencent Cloud Object Storage services.
    Cos,
    /// [d1][crate::services::D1]: D1 services
    D1,
    /// [dashmap][crate::services::Dashmap]: dashmap backend support.
    Dashmap,
    /// [etcd][crate::services::Etcd]: Etcd Services
    Etcd,
    /// [foundationdb][crate::services::Foundationdb]: Foundationdb services.
    Foundationdb,
    /// [dbfs][crate::services::Dbfs]: DBFS backend support.
    Dbfs,
    /// [fs][crate::services::Fs]: POSIX-like file system.
    Fs,
    /// [ftp][crate::services::Ftp]: FTP backend.
    Ftp,
    /// [gcs][crate::services::Gcs]: Google Cloud Storage backend.
    Gcs,
    /// [ghac][crate::services::Ghac]: GitHub Action Cache services.
    Ghac,
    /// [hdfs][crate::services::Hdfs]: Hadoop Distributed File System.
    Hdfs,
    /// [http][crate::services::Http]: HTTP backend.
    Http,
    /// [huggingface][crate::services::Huggingface]: Huggingface services.
    Huggingface,
    /// [alluxio][crate::services::Alluxio]: Alluxio services.
    Alluxio,

    /// [ipmfs][crate::services::Ipfs]: IPFS HTTP Gateway
    Ipfs,
    /// [ipmfs][crate::services::Ipmfs]: IPFS mutable file system
    Ipmfs,
    /// [icloud][crate::services::Icloud]: APPLE icloud services.
    Icloud,
    /// [memcached][crate::services::Memcached]: Memcached service support.
    Memcached,
    /// [memory][crate::services::Memory]: In memory backend support.
    Memory,
    /// [mini-moka][crate::services::MiniMoka]: Mini Moka backend support.
    MiniMoka,
    /// [moka][crate::services::Moka]: moka backend support.
    Moka,
    /// [monoiofs][crate::services::Monoiofs]: monoio fs services.
    Monoiofs,
    /// [obs][crate::services::Obs]: Huawei Cloud OBS services.
    Obs,
    /// [onedrive][crate::services::Onedrive]: Microsoft OneDrive services.
    Onedrive,
    /// [opfs][crate::services::opfs]: Original private file system.
    Opfs,
    /// [gdrive][crate::services::Gdrive]: GoogleDrive services.
    Gdrive,
    /// [dropbox][crate::services::Dropbox]: Dropbox services.
    Dropbox,
    /// [oss][crate::services::Oss]: Aliyun Object Storage Services
    Oss,
    /// [persy][crate::services::Persy]: persy backend support.
    Persy,
    /// [redis][crate::services::Redis]: Redis services
    Redis,
    /// [postgresql][crate::services::Postgresql]: Postgresql services
    Postgresql,
    /// [libsql][crate::services::Libsql]: Libsql services
    Libsql,
    /// [mysql][crate::services::Mysql]: Mysql services
    Mysql,
    /// [sqlite][crate::services::Sqlite]: Sqlite services
    Sqlite,
    /// [rocksdb][crate::services::Rocksdb]: RocksDB services
    Rocksdb,
    /// [s3][crate::services::S3]: AWS S3 alike services.
    S3,
    /// [sftp][crate::services::Sftp]: SFTP services
    Sftp,
    /// [sled][crate::services::Sled]: Sled services
    Sled,
    /// [Supabase][crate::services::Supabase]: Supabase storage service
    Supabase,
    /// [swift][crate::services::Swift]: Swift backend support.
    Swift,
    /// [Vercel Artifacts][crate::services::VercelArtifacts]: Vercel Artifacts service, as known as Vercel Remote Caching.
    VercelArtifacts,
    /// [webdav][crate::services::Webdav]: WebDAV support.
    Webdav,
    /// [webhdfs][crate::services::Webhdfs]: WebHDFS RESTful API Services
    Webhdfs,
    /// [redb][crate::services::Redb]: Redb Services
    Redb,
    /// [tikv][crate::services::Tikv]: Tikv Services
    Tikv,
    /// [azfile][crate::services::Azfile]: Azfile Services
    Azfile,
    /// [mongodb](crate::services::Mongodb): MongoDB Services
    Mongodb,
    /// [gridfs](crate::services::Gridfs): MongoDB Gridfs Services
    Gridfs,
    /// [Github Contents][crate::services::Github]: Github contents support.
    Github,
    /// [Native HDFS](crate::services::HdfsNative): Hdfs Native service, using rust hdfs-native client for hdfs
    HdfsNative,
    /// [surrealdb](crate::services::Surrealdb): Surrealdb Services
    Surrealdb,
    /// [lakefs](crate::services::Lakefs): LakeFS Services
    Lakefs,
    /// [NebulaGraph](crate::services::NebulaGraph): NebulaGraph Services
    NebulaGraph,
    /// Custom that allow users to implement services outside of OpenDAL.
    ///
    /// # NOTE
    ///
    /// - Custom must not overwrite any existing services name.
    /// - Custom must be in lower case.
    Custom(&'static str),
}

impl Scheme {
    /// Convert self into static str.
    pub fn into_static(self) -> &'static str {
        self.into()
    }

    /// Get all enabled schemes.
    ///
    /// OpenDAL could be compiled with different features, which will enable different schemes.
    /// This function returns all enabled schemes so users can make decisions based on it.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use opendal::Scheme;
    ///
    /// let enabled_schemes = Scheme::enabled();
    /// if !enabled_schemes.contains(&Scheme::Memory) {
    ///     panic!("s3 support is not enabled")
    /// }
    /// ```
    pub fn enabled() -> HashSet<Scheme> {
        HashSet::from([
            #[cfg(feature = "services-aliyun-drive")]
            Scheme::AliyunDrive,
            #[cfg(feature = "services-atomicserver")]
            Scheme::Atomicserver,
            #[cfg(feature = "services-alluxio")]
            Scheme::Alluxio,
            #[cfg(feature = "services-azblob")]
            Scheme::Azblob,
            #[cfg(feature = "services-azdls")]
            Scheme::Azdls,
            #[cfg(feature = "services-azfile")]
            Scheme::Azfile,
            #[cfg(feature = "services-b2")]
            Scheme::B2,
            #[cfg(feature = "services-cacache")]
            Scheme::Cacache,
            #[cfg(feature = "services-cos")]
            Scheme::Cos,
            #[cfg(feature = "services-compfs")]
            Scheme::Compfs,
            #[cfg(feature = "services-dashmap")]
            Scheme::Dashmap,
            #[cfg(feature = "services-dropbox")]
            Scheme::Dropbox,
            #[cfg(feature = "services-etcd")]
            Scheme::Etcd,
            #[cfg(feature = "services-foundationdb")]
            Scheme::Foundationdb,
            #[cfg(feature = "services-fs")]
            Scheme::Fs,
            #[cfg(feature = "services-ftp")]
            Scheme::Ftp,
            #[cfg(feature = "services-gcs")]
            Scheme::Gcs,
            #[cfg(feature = "services-ghac")]
            Scheme::Ghac,
            #[cfg(feature = "services-hdfs")]
            Scheme::Hdfs,
            #[cfg(feature = "services-http")]
            Scheme::Http,
            #[cfg(feature = "services-huggingface")]
            Scheme::Huggingface,
            #[cfg(feature = "services-ipfs")]
            Scheme::Ipfs,
            #[cfg(feature = "services-ipmfs")]
            Scheme::Ipmfs,
            #[cfg(feature = "services-icloud")]
            Scheme::Icloud,
            #[cfg(feature = "services-libsql")]
            Scheme::Libsql,
            #[cfg(feature = "services-memcached")]
            Scheme::Memcached,
            #[cfg(feature = "services-memory")]
            Scheme::Memory,
            #[cfg(feature = "services-mini-moka")]
            Scheme::MiniMoka,
            #[cfg(feature = "services-moka")]
            Scheme::Moka,
            #[cfg(feature = "services-monoiofs")]
            Scheme::Monoiofs,
            #[cfg(feature = "services-mysql")]
            Scheme::Mysql,
            #[cfg(feature = "services-obs")]
            Scheme::Obs,
            #[cfg(feature = "services-onedrive")]
            Scheme::Onedrive,
            #[cfg(feature = "services-postgresql")]
            Scheme::Postgresql,
            #[cfg(feature = "services-gdrive")]
            Scheme::Gdrive,
            #[cfg(feature = "services-oss")]
            Scheme::Oss,
            #[cfg(feature = "services-persy")]
            Scheme::Persy,
            #[cfg(feature = "services-redis")]
            Scheme::Redis,
            #[cfg(feature = "services-rocksdb")]
            Scheme::Rocksdb,
            #[cfg(feature = "services-s3")]
            Scheme::S3,
            #[cfg(feature = "services-seafile")]
            Scheme::Seafile,
            #[cfg(feature = "services-upyun")]
            Scheme::Upyun,
            #[cfg(feature = "services-yandex-disk")]
            Scheme::YandexDisk,
            #[cfg(feature = "services-pcloud")]
            Scheme::Pcloud,
            #[cfg(feature = "services-sftp")]
            Scheme::Sftp,
            #[cfg(feature = "services-sled")]
            Scheme::Sled,
            #[cfg(feature = "services-sqlite")]
            Scheme::Sqlite,
            #[cfg(feature = "services-supabase")]
            Scheme::Supabase,
            #[cfg(feature = "services-swift")]
            Scheme::Swift,
            #[cfg(feature = "services-tikv")]
            Scheme::Tikv,
            #[cfg(feature = "services-vercel-artifacts")]
            Scheme::VercelArtifacts,
            #[cfg(feature = "services-vercel-blob")]
            Scheme::VercelBlob,
            #[cfg(feature = "services-webdav")]
            Scheme::Webdav,
            #[cfg(feature = "services-webhdfs")]
            Scheme::Webhdfs,
            #[cfg(feature = "services-redb")]
            Scheme::Redb,
            #[cfg(feature = "services-mongodb")]
            Scheme::Mongodb,
            #[cfg(feature = "services-hdfs-native")]
            Scheme::HdfsNative,
            #[cfg(feature = "services-surrealdb")]
            Scheme::Surrealdb,
            #[cfg(feature = "services-lakefs")]
            Scheme::Lakefs,
            #[cfg(feature = "services-nebula-graph")]
            Scheme::NebulaGraph,
        ])
    }
}

impl Default for Scheme {
    fn default() -> Self {
        Self::Memory
    }
}

impl Display for Scheme {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.into_static())
    }
}

impl FromStr for Scheme {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_lowercase();
        match s.as_str() {
            "aliyun_drive" => Ok(Scheme::AliyunDrive),
            "atomicserver" => Ok(Scheme::Atomicserver),
            "azblob" => Ok(Scheme::Azblob),
            "alluxio" => Ok(Scheme::Alluxio),
            // Notes:
            //
            // OpenDAL used to call `azdls` as `azdfs`, we keep it for backward compatibility.
            // And abfs is widely used in hadoop ecosystem, keep it for easy to use.
            "azdls" | "azdfs" | "abfs" => Ok(Scheme::Azdls),
            "b2" => Ok(Scheme::B2),
            "chainsafe" => Ok(Scheme::Chainsafe),
            "cacache" => Ok(Scheme::Cacache),
            "compfs" => Ok(Scheme::Compfs),
            "cloudflare_kv" => Ok(Scheme::CloudflareKv),
            "cos" => Ok(Scheme::Cos),
            "d1" => Ok(Scheme::D1),
            "dashmap" => Ok(Scheme::Dashmap),
            "dropbox" => Ok(Scheme::Dropbox),
            "etcd" => Ok(Scheme::Etcd),
            "dbfs" => Ok(Scheme::Dbfs),
            "fs" => Ok(Scheme::Fs),
            "gcs" => Ok(Scheme::Gcs),
            "gdrive" => Ok(Scheme::Gdrive),
            "ghac" => Ok(Scheme::Ghac),
            "gridfs" => Ok(Scheme::Gridfs),
            "github" => Ok(Scheme::Github),
            "hdfs" => Ok(Scheme::Hdfs),
            "http" | "https" => Ok(Scheme::Http),
            "huggingface" | "hf" => Ok(Scheme::Huggingface),
            "ftp" | "ftps" => Ok(Scheme::Ftp),
            "ipfs" | "ipns" => Ok(Scheme::Ipfs),
            "ipmfs" => Ok(Scheme::Ipmfs),
            "icloud" => Ok(Scheme::Icloud),
            "koofr" => Ok(Scheme::Koofr),
            "libsql" => Ok(Scheme::Libsql),
            "memcached" => Ok(Scheme::Memcached),
            "memory" => Ok(Scheme::Memory),
            "mysql" => Ok(Scheme::Mysql),
            "sqlite" => Ok(Scheme::Sqlite),
            "mini_moka" => Ok(Scheme::MiniMoka),
            "moka" => Ok(Scheme::Moka),
            "monoiofs" => Ok(Scheme::Monoiofs),
            "obs" => Ok(Scheme::Obs),
            "onedrive" => Ok(Scheme::Onedrive),
            "opfs" => Ok(Scheme::Opfs),
            "persy" => Ok(Scheme::Persy),
            "postgresql" => Ok(Scheme::Postgresql),
            "redb" => Ok(Scheme::Redb),
            "redis" => Ok(Scheme::Redis),
            "rocksdb" => Ok(Scheme::Rocksdb),
            "s3" => Ok(Scheme::S3),
            "seafile" => Ok(Scheme::Seafile),
            "upyun" => Ok(Scheme::Upyun),
            "yandex_disk" => Ok(Scheme::YandexDisk),
            "pcloud" => Ok(Scheme::Pcloud),
            "sftp" => Ok(Scheme::Sftp),
            "sled" => Ok(Scheme::Sled),
            "supabase" => Ok(Scheme::Supabase),
            "swift" => Ok(Scheme::Swift),
            "oss" => Ok(Scheme::Oss),
            "vercel_artifacts" => Ok(Scheme::VercelArtifacts),
            "vercel_blob" => Ok(Scheme::VercelBlob),
            "webdav" => Ok(Scheme::Webdav),
            "webhdfs" => Ok(Scheme::Webhdfs),
            "tikv" => Ok(Scheme::Tikv),
            "azfile" => Ok(Scheme::Azfile),
            "mongodb" => Ok(Scheme::Mongodb),
            "hdfs_native" => Ok(Scheme::HdfsNative),
            "surrealdb" => Ok(Scheme::Surrealdb),
            "lakefs" => Ok(Scheme::Lakefs),
            "nebula_graph" => Ok(Scheme::NebulaGraph),
            _ => Ok(Scheme::Custom(Box::leak(s.into_boxed_str()))),
        }
    }
}

impl From<Scheme> for &'static str {
    fn from(v: Scheme) -> Self {
        match v {
            Scheme::AliyunDrive => "aliyun_drive",
            Scheme::Atomicserver => "atomicserver",
            Scheme::Azblob => "azblob",
            Scheme::Azdls => "azdls",
            Scheme::B2 => "b2",
            Scheme::Chainsafe => "chainsafe",
            Scheme::Cacache => "cacache",
            Scheme::CloudflareKv => "cloudflare_kv",
            Scheme::Cos => "cos",
            Scheme::Compfs => "compfs",
            Scheme::D1 => "d1",
            Scheme::Dashmap => "dashmap",
            Scheme::Etcd => "etcd",
            Scheme::Dbfs => "dbfs",
            Scheme::Fs => "fs",
            Scheme::Gcs => "gcs",
            Scheme::Ghac => "ghac",
            Scheme::Gridfs => "gridfs",
            Scheme::Hdfs => "hdfs",
            Scheme::Http => "http",
            Scheme::Huggingface => "huggingface",
            Scheme::Foundationdb => "foundationdb",
            Scheme::Ftp => "ftp",
            Scheme::Ipfs => "ipfs",
            Scheme::Ipmfs => "ipmfs",
            Scheme::Icloud => "icloud",
            Scheme::Koofr => "koofr",
            Scheme::Libsql => "libsql",
            Scheme::Memcached => "memcached",
            Scheme::Memory => "memory",
            Scheme::MiniMoka => "mini_moka",
            Scheme::Moka => "moka",
            Scheme::Monoiofs => "monoiofs",
            Scheme::Obs => "obs",
            Scheme::Onedrive => "onedrive",
            Scheme::Opfs => "opfs",
            Scheme::Persy => "persy",
            Scheme::Postgresql => "postgresql",
            Scheme::Mysql => "mysql",
            Scheme::Gdrive => "gdrive",
            Scheme::Github => "github",
            Scheme::Dropbox => "dropbox",
            Scheme::Redis => "redis",
            Scheme::Rocksdb => "rocksdb",
            Scheme::S3 => "s3",
            Scheme::Seafile => "seafile",
            Scheme::Sftp => "sftp",
            Scheme::Sled => "sled",
            Scheme::Supabase => "supabase",
            Scheme::Swift => "swift",
            Scheme::VercelArtifacts => "vercel_artifacts",
            Scheme::VercelBlob => "vercel_blob",
            Scheme::Oss => "oss",
            Scheme::Webdav => "webdav",
            Scheme::Webhdfs => "webhdfs",
            Scheme::Redb => "redb",
            Scheme::Tikv => "tikv",
            Scheme::Azfile => "azfile",
            Scheme::Sqlite => "sqlite",
            Scheme::Mongodb => "mongodb",
            Scheme::Alluxio => "alluxio",
            Scheme::Upyun => "upyun",
            Scheme::YandexDisk => "yandex_disk",
            Scheme::Pcloud => "pcloud",
            Scheme::HdfsNative => "hdfs_native",
            Scheme::Surrealdb => "surrealdb",
            Scheme::Lakefs => "lakefs",
            Scheme::NebulaGraph => "nebula_graph",
            Scheme::Custom(v) => v,
        }
    }
}

impl From<Scheme> for String {
    fn from(v: Scheme) -> Self {
        v.into_static().to_string()
    }
}
