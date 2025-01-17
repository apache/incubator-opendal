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
use std::sync::Arc;

use crate::layers::*;
use crate::raw::*;
use crate::*;
// TODO: is this import path idiomatic to the project?
use super::registry::GLOBAL_OPERATOR_REGISTRY;

/// # Operator build API
///
/// Operator should be built via [`OperatorBuilder`]. We recommend to use [`Operator::new`] to get started:
///
/// ```
/// # use anyhow::Result;
/// use opendal::services::Fs;
/// use opendal::Operator;
/// async fn test() -> Result<()> {
///     // Create fs backend builder.
///     let builder = Fs::default().root("/tmp");
///
///     // Build an `Operator` to start operating the storage.
///     let op: Operator = Operator::new(builder)?.finish();
///
///     Ok(())
/// }
/// ```
impl Operator {
    /// Create a new operator with input builder.
    ///
    /// OpenDAL will call `builder.build()` internally, so we don't need
    /// to import `opendal::Builder` trait.
    ///
    /// # Examples
    ///
    /// Read more backend init examples in [examples](https://github.com/apache/opendal/tree/main/examples).
    ///
    /// ```
    /// # use anyhow::Result;
    /// use opendal::services::Fs;
    /// use opendal::Operator;
    /// async fn test() -> Result<()> {
    ///     // Create fs backend builder.
    ///     let builder = Fs::default().root("/tmp");
    ///
    ///     // Build an `Operator` to start operating the storage.
    ///     let op: Operator = Operator::new(builder)?.finish();
    ///
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::new_ret_no_self)]
    pub fn new<B: Builder>(ab: B) -> Result<OperatorBuilder<impl Access>> {
        let acc = ab.build()?;
        Ok(OperatorBuilder::new(acc))
    }

    /// Create a new operator from given config.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// use std::collections::HashMap;
    ///
    /// use opendal::services::MemoryConfig;
    /// use opendal::Operator;
    /// async fn test() -> Result<()> {
    ///     let cfg = MemoryConfig::default();
    ///
    ///     // Build an `Operator` to start operating the storage.
    ///     let op: Operator = Operator::from_config(cfg)?.finish();
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn from_config<C: Configurator>(cfg: C) -> Result<OperatorBuilder<impl Access>> {
        let builder = cfg.into_builder();
        let acc = builder.build()?;
        Ok(OperatorBuilder::new(acc))
    }

    /// TODO: document this.
    ///
    /// TODO: improve those examples
    /// TODO: this test does not work. It always output Ok
    /// # Examples
    /// ```
    /// # use anyhow::Result;
    /// use opendal::Operator;
    ///
    /// fn test() -> Result<()> {
    ///     Operator::from_uri("fs://?root=/tmp/test", vec![])?;
    ///      Ok(())
    /// }
    /// ```
    pub fn from_uri(
        uri: &str,
        options: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self> {
        GLOBAL_OPERATOR_REGISTRY.with(|registry| registry.parse(uri, options))
    }

    /// Create a new operator from given iterator in static dispatch.
    ///
    /// # Notes
    ///
    /// `from_iter` generates a `OperatorBuilder` which allows adding layer in zero-cost way.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// use std::collections::HashMap;
    ///
    /// use opendal::services::Fs;
    /// use opendal::Operator;
    /// async fn test() -> Result<()> {
    ///     let map = HashMap::from([
    ///         // Set the root for fs, all operations will happen under this root.
    ///         //
    ///         // NOTE: the root must be absolute path.
    ///         ("root".to_string(), "/tmp".to_string()),
    ///     ]);
    ///
    ///     // Build an `Operator` to start operating the storage.
    ///     let op: Operator = Operator::from_iter::<Fs>(map)?.finish();
    ///
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_iter<B: Builder>(
        iter: impl IntoIterator<Item = (String, String)>,
    ) -> Result<OperatorBuilder<impl Access>> {
        let builder = B::Config::from_iter(iter)?.into_builder();
        let acc = builder.build()?;
        Ok(OperatorBuilder::new(acc))
    }

    /// Create a new operator via given scheme and iterator of config value in dynamic dispatch.
    ///
    /// # Notes
    ///
    /// `via_iter` generates a `Operator` which allows building operator without generic type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// use std::collections::HashMap;
    ///
    /// use opendal::Operator;
    /// use opendal::Scheme;
    /// async fn test() -> Result<()> {
    ///     let map = [
    ///         // Set the root for fs, all operations will happen under this root.
    ///         //
    ///         // NOTE: the root must be absolute path.
    ///         ("root".to_string(), "/tmp".to_string()),
    ///     ];
    ///
    ///     // Build an `Operator` to start operating the storage.
    ///     let op: Operator = Operator::via_iter(Scheme::Fs, map)?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[allow(unused_variables, unreachable_code)]
    pub fn via_iter(
        scheme: Scheme,
        iter: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Operator> {
        let op = match scheme {
            #[cfg(feature = "services-aliyun-drive")]
            Scheme::AliyunDrive => Self::from_iter::<services::AliyunDrive>(iter)?.finish(),
            #[cfg(feature = "services-atomicserver")]
            Scheme::Atomicserver => Self::from_iter::<services::Atomicserver>(iter)?.finish(),
            #[cfg(feature = "services-alluxio")]
            Scheme::Alluxio => Self::from_iter::<services::Alluxio>(iter)?.finish(),
            #[cfg(feature = "services-compfs")]
            Scheme::Compfs => Self::from_iter::<services::Compfs>(iter)?.finish(),
            #[cfg(feature = "services-upyun")]
            Scheme::Upyun => Self::from_iter::<services::Upyun>(iter)?.finish(),
            #[cfg(feature = "services-koofr")]
            Scheme::Koofr => Self::from_iter::<services::Koofr>(iter)?.finish(),
            #[cfg(feature = "services-yandex-disk")]
            Scheme::YandexDisk => Self::from_iter::<services::YandexDisk>(iter)?.finish(),
            #[cfg(feature = "services-pcloud")]
            Scheme::Pcloud => Self::from_iter::<services::Pcloud>(iter)?.finish(),
            #[cfg(feature = "services-chainsafe")]
            Scheme::Chainsafe => Self::from_iter::<services::Chainsafe>(iter)?.finish(),
            #[cfg(feature = "services-azblob")]
            Scheme::Azblob => Self::from_iter::<services::Azblob>(iter)?.finish(),
            #[cfg(feature = "services-azdls")]
            Scheme::Azdls => Self::from_iter::<services::Azdls>(iter)?.finish(),
            #[cfg(feature = "services-azfile")]
            Scheme::Azfile => Self::from_iter::<services::Azfile>(iter)?.finish(),
            #[cfg(feature = "services-b2")]
            Scheme::B2 => Self::from_iter::<services::B2>(iter)?.finish(),
            #[cfg(feature = "services-cacache")]
            Scheme::Cacache => Self::from_iter::<services::Cacache>(iter)?.finish(),
            #[cfg(feature = "services-cos")]
            Scheme::Cos => Self::from_iter::<services::Cos>(iter)?.finish(),
            #[cfg(feature = "services-d1")]
            Scheme::D1 => Self::from_iter::<services::D1>(iter)?.finish(),
            #[cfg(feature = "services-dashmap")]
            Scheme::Dashmap => Self::from_iter::<services::Dashmap>(iter)?.finish(),
            #[cfg(feature = "services-dropbox")]
            Scheme::Dropbox => Self::from_iter::<services::Dropbox>(iter)?.finish(),
            #[cfg(feature = "services-etcd")]
            Scheme::Etcd => Self::from_iter::<services::Etcd>(iter)?.finish(),
            #[cfg(feature = "services-foundationdb")]
            Scheme::Foundationdb => Self::from_iter::<services::Foundationdb>(iter)?.finish(),
            #[cfg(feature = "services-fs")]
            Scheme::Fs => Self::from_iter::<services::Fs>(iter)?.finish(),
            #[cfg(feature = "services-ftp")]
            Scheme::Ftp => Self::from_iter::<services::Ftp>(iter)?.finish(),
            #[cfg(feature = "services-gcs")]
            Scheme::Gcs => Self::from_iter::<services::Gcs>(iter)?.finish(),
            #[cfg(feature = "services-ghac")]
            Scheme::Ghac => Self::from_iter::<services::Ghac>(iter)?.finish(),
            #[cfg(feature = "services-gridfs")]
            Scheme::Gridfs => Self::from_iter::<services::Gridfs>(iter)?.finish(),
            #[cfg(feature = "services-github")]
            Scheme::Github => Self::from_iter::<services::Github>(iter)?.finish(),
            #[cfg(feature = "services-hdfs")]
            Scheme::Hdfs => Self::from_iter::<services::Hdfs>(iter)?.finish(),
            #[cfg(feature = "services-http")]
            Scheme::Http => Self::from_iter::<services::Http>(iter)?.finish(),
            #[cfg(feature = "services-huggingface")]
            Scheme::Huggingface => Self::from_iter::<services::Huggingface>(iter)?.finish(),
            #[cfg(feature = "services-ipfs")]
            Scheme::Ipfs => Self::from_iter::<services::Ipfs>(iter)?.finish(),
            #[cfg(feature = "services-ipmfs")]
            Scheme::Ipmfs => Self::from_iter::<services::Ipmfs>(iter)?.finish(),
            #[cfg(feature = "services-icloud")]
            Scheme::Icloud => Self::from_iter::<services::Icloud>(iter)?.finish(),
            #[cfg(feature = "services-libsql")]
            Scheme::Libsql => Self::from_iter::<services::Libsql>(iter)?.finish(),
            #[cfg(feature = "services-memcached")]
            Scheme::Memcached => Self::from_iter::<services::Memcached>(iter)?.finish(),
            #[cfg(feature = "services-memory")]
            Scheme::Memory => Self::from_iter::<services::Memory>(iter)?.finish(),
            #[cfg(feature = "services-mini-moka")]
            Scheme::MiniMoka => Self::from_iter::<services::MiniMoka>(iter)?.finish(),
            #[cfg(feature = "services-moka")]
            Scheme::Moka => Self::from_iter::<services::Moka>(iter)?.finish(),
            #[cfg(feature = "services-monoiofs")]
            Scheme::Monoiofs => Self::from_iter::<services::Monoiofs>(iter)?.finish(),
            #[cfg(feature = "services-mysql")]
            Scheme::Mysql => Self::from_iter::<services::Mysql>(iter)?.finish(),
            #[cfg(feature = "services-obs")]
            Scheme::Obs => Self::from_iter::<services::Obs>(iter)?.finish(),
            #[cfg(feature = "services-onedrive")]
            Scheme::Onedrive => Self::from_iter::<services::Onedrive>(iter)?.finish(),
            #[cfg(feature = "services-postgresql")]
            Scheme::Postgresql => Self::from_iter::<services::Postgresql>(iter)?.finish(),
            #[cfg(feature = "services-gdrive")]
            Scheme::Gdrive => Self::from_iter::<services::Gdrive>(iter)?.finish(),
            #[cfg(feature = "services-oss")]
            Scheme::Oss => Self::from_iter::<services::Oss>(iter)?.finish(),
            #[cfg(feature = "services-persy")]
            Scheme::Persy => Self::from_iter::<services::Persy>(iter)?.finish(),
            #[cfg(feature = "services-redis")]
            Scheme::Redis => Self::from_iter::<services::Redis>(iter)?.finish(),
            #[cfg(feature = "services-rocksdb")]
            Scheme::Rocksdb => Self::from_iter::<services::Rocksdb>(iter)?.finish(),
            #[cfg(feature = "services-s3")]
            Scheme::S3 => Self::from_iter::<services::S3>(iter)?.finish(),
            #[cfg(feature = "services-seafile")]
            Scheme::Seafile => Self::from_iter::<services::Seafile>(iter)?.finish(),
            #[cfg(feature = "services-sftp")]
            Scheme::Sftp => Self::from_iter::<services::Sftp>(iter)?.finish(),
            #[cfg(feature = "services-sled")]
            Scheme::Sled => Self::from_iter::<services::Sled>(iter)?.finish(),
            #[cfg(feature = "services-sqlite")]
            Scheme::Sqlite => Self::from_iter::<services::Sqlite>(iter)?.finish(),
            #[cfg(feature = "services-supabase")]
            Scheme::Supabase => Self::from_iter::<services::Supabase>(iter)?.finish(),
            #[cfg(feature = "services-swift")]
            Scheme::Swift => Self::from_iter::<services::Swift>(iter)?.finish(),
            #[cfg(feature = "services-tikv")]
            Scheme::Tikv => Self::from_iter::<services::Tikv>(iter)?.finish(),
            #[cfg(feature = "services-vercel-artifacts")]
            Scheme::VercelArtifacts => Self::from_iter::<services::VercelArtifacts>(iter)?.finish(),
            #[cfg(feature = "services-vercel-blob")]
            Scheme::VercelBlob => Self::from_iter::<services::VercelBlob>(iter)?.finish(),
            #[cfg(feature = "services-webdav")]
            Scheme::Webdav => Self::from_iter::<services::Webdav>(iter)?.finish(),
            #[cfg(feature = "services-webhdfs")]
            Scheme::Webhdfs => Self::from_iter::<services::Webhdfs>(iter)?.finish(),
            #[cfg(feature = "services-redb")]
            Scheme::Redb => Self::from_iter::<services::Redb>(iter)?.finish(),
            #[cfg(feature = "services-mongodb")]
            Scheme::Mongodb => Self::from_iter::<services::Mongodb>(iter)?.finish(),
            #[cfg(feature = "services-hdfs-native")]
            Scheme::HdfsNative => Self::from_iter::<services::HdfsNative>(iter)?.finish(),
            #[cfg(feature = "services-lakefs")]
            Scheme::Lakefs => Self::from_iter::<services::Lakefs>(iter)?.finish(),
            #[cfg(feature = "services-nebula-graph")]
            Scheme::NebulaGraph => Self::from_iter::<services::NebulaGraph>(iter)?.finish(),
            v => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "scheme is not enabled or supported",
                )
                .with_context("scheme", v))
            }
        };

        Ok(op)
    }

    /// Create a new operator from given map.
    ///
    /// # Notes
    ///
    /// from_map is using static dispatch layers which is zero cost. via_map is
    /// using dynamic dispatch layers which has a bit runtime overhead with an
    /// extra vtable lookup and unable to inline. But from_map requires generic
    /// type parameter which is not always easy to be used.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// use std::collections::HashMap;
    ///
    /// use opendal::services::Fs;
    /// use opendal::Operator;
    /// async fn test() -> Result<()> {
    ///     let map = HashMap::from([
    ///         // Set the root for fs, all operations will happen under this root.
    ///         //
    ///         // NOTE: the root must be absolute path.
    ///         ("root".to_string(), "/tmp".to_string()),
    ///     ]);
    ///
    ///     // Build an `Operator` to start operating the storage.
    ///     let op: Operator = Operator::from_map::<Fs>(map)?.finish();
    ///
    ///     Ok(())
    /// }
    /// ```
    #[deprecated = "use from_iter instead"]
    pub fn from_map<B: Builder>(
        map: HashMap<String, String>,
    ) -> Result<OperatorBuilder<impl Access>> {
        Self::from_iter::<B>(map)
    }

    /// Create a new operator from given scheme and map.
    ///
    /// # Notes
    ///
    /// from_map is using static dispatch layers which is zero cost. via_map is
    /// using dynamic dispatch layers which has a bit runtime overhead with an
    /// extra vtable lookup and unable to inline. But from_map requires generic
    /// type parameter which is not always easy to be used.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// use std::collections::HashMap;
    ///
    /// use opendal::Operator;
    /// use opendal::Scheme;
    /// async fn test() -> Result<()> {
    ///     let map = HashMap::from([
    ///         // Set the root for fs, all operations will happen under this root.
    ///         //
    ///         // NOTE: the root must be absolute path.
    ///         ("root".to_string(), "/tmp".to_string()),
    ///     ]);
    ///
    ///     // Build an `Operator` to start operating the storage.
    ///     let op: Operator = Operator::via_map(Scheme::Fs, map)?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[deprecated = "use via_iter instead"]
    pub fn via_map(scheme: Scheme, map: HashMap<String, String>) -> Result<Operator> {
        Self::via_iter(scheme, map)
    }

    /// Create a new layer with dynamic dispatch.
    ///
    /// # Notes
    ///
    /// `OperatorBuilder::layer()` is using static dispatch which is zero
    /// cost. `Operator::layer()` is using dynamic dispatch which has a
    /// bit runtime overhead with an extra vtable lookup and unable to
    /// inline.
    ///
    /// It's always recommended to use `OperatorBuilder::layer()` instead.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// use opendal::layers::LoggingLayer;
    /// use opendal::services::Fs;
    /// use opendal::Operator;
    ///
    /// # async fn test() -> Result<()> {
    /// let op = Operator::new(Fs::default())?.finish();
    /// let op = op.layer(LoggingLayer::default());
    /// // All operations will go through the new_layer
    /// let _ = op.read("test_file").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn layer<L: Layer<Accessor>>(self, layer: L) -> Self {
        Self::from_inner(Arc::new(
            TypeEraseLayer.layer(layer.layer(self.into_inner())),
        ))
    }
}

/// OperatorBuilder is a typed builder to build an Operator.
///
/// # Notes
///
/// OpenDAL uses static dispatch internally and only performs dynamic
/// dispatch at the outmost type erase layer. OperatorBuilder is the only
/// public API provided by OpenDAL come with generic parameters.
///
/// It's required to call `finish` after the operator built.
///
/// # Examples
///
/// For users who want to support many services, we can build a helper function like the following:
///
/// ```
/// use std::collections::HashMap;
///
/// use opendal::layers::LoggingLayer;
/// use opendal::layers::RetryLayer;
/// use opendal::services;
/// use opendal::Builder;
/// use opendal::Operator;
/// use opendal::Result;
/// use opendal::Scheme;
///
/// fn init_service<B: Builder>(cfg: HashMap<String, String>) -> Result<Operator> {
///     let op = Operator::from_map::<B>(cfg)?
///         .layer(LoggingLayer::default())
///         .layer(RetryLayer::new())
///         .finish();
///
///     Ok(op)
/// }
///
/// async fn init(scheme: Scheme, cfg: HashMap<String, String>) -> Result<()> {
///     let _ = match scheme {
///         Scheme::S3 => init_service::<services::S3>(cfg)?,
///         Scheme::Fs => init_service::<services::Fs>(cfg)?,
///         _ => todo!(),
///     };
///
///     Ok(())
/// }
/// ```
pub struct OperatorBuilder<A: Access> {
    accessor: A,
}

impl<A: Access> OperatorBuilder<A> {
    /// Create a new operator builder.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(accessor: A) -> OperatorBuilder<impl Access> {
        // Make sure error context layer has been attached.
        OperatorBuilder { accessor }
            .layer(ErrorContextLayer)
            .layer(CompleteLayer)
            .layer(CorrectnessCheckLayer)
    }

    /// Create a new layer with static dispatch.
    ///
    /// # Notes
    ///
    /// `OperatorBuilder::layer()` is using static dispatch which is zero
    /// cost. `Operator::layer()` is using dynamic dispatch which has a
    /// bit runtime overhead with an extra vtable lookup and unable to
    /// inline.
    ///
    /// It's always recommended to use `OperatorBuilder::layer()` instead.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// use opendal::layers::LoggingLayer;
    /// use opendal::services::Fs;
    /// use opendal::Operator;
    ///
    /// # async fn test() -> Result<()> {
    /// let op = Operator::new(Fs::default())?
    ///     .layer(LoggingLayer::default())
    ///     .finish();
    /// // All operations will go through the new_layer
    /// let _ = op.read("test_file").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn layer<L: Layer<A>>(self, layer: L) -> OperatorBuilder<L::LayeredAccess> {
        OperatorBuilder {
            accessor: layer.layer(self.accessor),
        }
    }

    /// Finish the building to construct an Operator.
    pub fn finish(self) -> Operator {
        let ob = self.layer(TypeEraseLayer);
        Operator::from_inner(Arc::new(ob.accessor) as Accessor)
    }
}
