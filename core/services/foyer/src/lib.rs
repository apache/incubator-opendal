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

#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]

mod backend;
mod config;
mod core;
mod deleter;
mod reader;
mod writer;

use std::ops::Deref;

use foyer::Code;
use foyer::Result as FoyerResult;

use opendal_core::Buffer;

pub use backend::FoyerBuilder as Foyer;
pub use config::FoyerConfig;

/// URI scheme used for service registration and scheme-driven construction.
pub const FOYER_SCHEME: &str = "foyer";

/// [`FoyerKey`] is a key for the foyer cache.
///
/// It implements foyer's [`Code`] trait directly, so the service does not depend on
/// foyer's `serde` feature (and its bincode dependency).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FoyerKey {
    /// The path of the key.
    pub path: String,
}

impl Code for FoyerKey {
    fn encode(&self, writer: &mut impl std::io::Write) -> FoyerResult<()> {
        let path = self.path.as_bytes();
        writer.write_all(&(path.len() as u64).to_le_bytes())?;
        writer.write_all(path)?;
        Ok(())
    }

    fn decode(reader: &mut impl std::io::Read) -> FoyerResult<Self>
    where
        Self: Sized,
    {
        let mut len_bytes = [0u8; 8];
        reader.read_exact(&mut len_bytes)?;
        let len = u64::from_le_bytes(len_bytes) as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        let path = String::from_utf8(buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(FoyerKey { path })
    }

    fn estimated_size(&self) -> usize {
        8 + self.path.len()
    }
}

/// [`FoyerValue`] is a wrapper around `Buffer` that implements the `Code` trait.
#[derive(Debug)]
pub struct FoyerValue(pub Buffer);

impl Deref for FoyerValue {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Code for FoyerValue {
    fn encode(&self, writer: &mut impl std::io::Write) -> FoyerResult<()> {
        let len = self.0.len() as u64;
        writer.write_all(&len.to_le_bytes())?;
        std::io::copy(&mut self.0.clone(), writer)?;
        Ok(())
    }

    fn decode(reader: &mut impl std::io::Read) -> FoyerResult<Self>
    where
        Self: Sized,
    {
        let mut len_bytes = [0u8; 8];
        reader.read_exact(&mut len_bytes)?;
        let len = u64::from_le_bytes(len_bytes) as usize;
        let mut buffer = vec![0u8; len];
        reader.read_exact(&mut buffer[..len])?;
        Ok(FoyerValue(buffer.into()))
    }

    fn estimated_size(&self) -> usize {
        8 + self.0.len()
    }
}

/// Register this service's URI scheme or schemes with an operator registry.
///
/// Registration enables scheme-driven construction through
/// [`opendal_core::Operator::from_uri`] and
/// [`opendal_core::Operator::via_iter`]. Direct construction through
/// [`opendal_core::Operator::new`] does not require registration.
pub fn register_foyer_service(registry: &opendal_core::OperatorRegistry) {
    registry.register::<Foyer>(FOYER_SCHEME);
}

#[cfg(test)]
mod tests {
    use foyer::{
        BlockEngineConfig, DeviceBuilder, FsDeviceBuilder, HybridCacheBuilder, RecoverMode,
    };
    use opendal_core::ErrorKind;
    use opendal_core::Operator;
    use size::consts::MiB;

    use super::*;

    fn key(i: u8) -> String {
        format!("obj-{i}")
    }

    fn value(i: u8) -> Vec<u8> {
        vec![i; 1024]
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let dir = tempfile::tempdir().unwrap();

        let cache = HybridCacheBuilder::new()
            .memory(10)
            .with_shards(1)
            .storage()
            .with_engine_config(
                BlockEngineConfig::new(
                    FsDeviceBuilder::new(dir.path())
                        .with_capacity(16 * MiB as usize)
                        .build()
                        .unwrap(),
                )
                .with_block_size(MiB as usize),
            )
            .with_recover_mode(RecoverMode::None)
            .build()
            .await
            .unwrap();

        let op = Operator::new(Foyer::new().cache(cache)).unwrap();

        // Write some data
        for i in 0..10 {
            op.write(&key(i), value(i)).await.unwrap();
        }

        // Read back
        for i in 0..10 {
            let buf = op.read(&key(i)).await.unwrap();
            assert_eq!(buf.to_vec(), value(i));
        }

        // Stat
        for i in 0..10 {
            let meta = op.stat(&key(i)).await.unwrap();
            assert_eq!(meta.content_length(), 1024);
        }

        // Delete
        for i in 0..10 {
            op.delete(&key(i)).await.unwrap();
        }

        // Verify deleted
        for i in 0..10 {
            let res = op.read(&key(i)).await;
            assert!(res.is_err(), "should fail to read deleted file");
        }
    }

    #[tokio::test]
    async fn test_range_read() {
        let dir = tempfile::tempdir().unwrap();

        let cache = HybridCacheBuilder::new()
            .memory(1024 * 1024)
            .with_shards(1)
            .storage()
            .with_engine_config(
                BlockEngineConfig::new(
                    FsDeviceBuilder::new(dir.path())
                        .with_capacity(16 * MiB as usize)
                        .build()
                        .unwrap(),
                )
                .with_block_size(MiB as usize),
            )
            .with_recover_mode(RecoverMode::None)
            .build()
            .await
            .unwrap();

        let op = Operator::new(Foyer::new().cache(cache)).unwrap();

        let data: Vec<u8> = (0..100).collect();
        op.write("test", data.clone()).await.unwrap();

        // Range read
        let buf = op.read_with("test").range(10..20).await.unwrap();
        assert_eq!(buf.to_vec(), data[10..20]);

        let err = op.read_with("test").range(95..105).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::RangeNotSatisfied);

        let buf = op.read_with("test").range(100..).await.unwrap();
        assert!(buf.is_empty());

        let buf = op.read_with("test").range(200..).await.unwrap();
        assert!(buf.is_empty());

        let buf = op.read_with("test").range(200..200).await.unwrap();
        assert!(buf.is_empty());
    }

    #[tokio::test]
    async fn test_hybrid_cache_via_config() {
        let dir = tempfile::tempdir().unwrap();

        // Test using the builder API with disk configuration
        let op = Operator::new(
            Foyer::new()
                .memory(1024 * 1024) // 1MB memory
                .disk_path(dir.path().to_str().unwrap())
                .disk_capacity(16 * 1024 * 1024) // 16MB disk
                .disk_file_size(1024 * 1024) // 1MB per file
                .recover_mode("none")
                .shards(1),
        )
        .unwrap();

        // Write some data
        for i in 0..5 {
            op.write(&key(i), value(i)).await.unwrap();
        }

        // Read back
        for i in 0..5 {
            let buf = op.read(&key(i)).await.unwrap();
            assert_eq!(buf.to_vec(), value(i));
        }

        // Delete
        for i in 0..5 {
            op.delete(&key(i)).await.unwrap();
        }
    }
}
