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

use std::fmt::Debug;

use opendal_core::*;

#[derive(Clone)]
pub struct CacacheCore {
    pub path: String,
}

impl Debug for CacacheCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacacheCore")
            .field("path", &self.path)
            .finish()
    }
}

impl CacacheCore {
    pub async fn get(&self, key: &str) -> Result<Option<bytes::Bytes>> {
        let cache_path = self.path.clone();
        let cache_key = key.to_string();

        let data = cacache::read(&cache_path, &cache_key).await;

        match data {
            Ok(bs) => Ok(Some(bytes::Bytes::from(bs))),
            Err(cacache::Error::EntryNotFound(_, _)) => Ok(None),
            Err(err) => Err(Error::new(ErrorKind::Unexpected, "cacache get failed")
                .with_operation("CacacheCore::get")
                .set_source(err)),
        }
    }

    pub async fn set(&self, key: &str, value: bytes::Bytes) -> Result<()> {
        let cache_path = self.path.clone();
        let cache_key = key.to_string();

        cacache::write(&cache_path, &cache_key, value.to_vec())
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Unexpected, "cacache set failed")
                    .with_operation("CacacheCore::set")
                    .set_source(err)
            })?;

        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let cache_path = self.path.clone();
        let cache_key = key.to_string();

        cacache::remove(&cache_path, &cache_key)
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Unexpected, "cacache delete failed")
                    .with_operation("CacacheCore::delete")
                    .set_source(err)
            })?;

        Ok(())
    }

    pub async fn metadata(&self, key: &str) -> Result<Option<cacache::Metadata>> {
        let cache_path = self.path.clone();
        let cache_key = key.to_string();

        let metadata = cacache::metadata(&cache_path, &cache_key).await;

        match metadata {
            Ok(Some(md)) => Ok(Some(md)),
            Ok(None) => Ok(None),
            Err(cacache::Error::EntryNotFound(_, _)) => Ok(None),
            Err(err) => Err(Error::new(ErrorKind::Unexpected, "cacache metadata failed")
                .with_operation("CacacheCore::metadata")
                .set_source(err)),
        }
    }
}
