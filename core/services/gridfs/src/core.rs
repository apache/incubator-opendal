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

use asyncband::once::OnceCell;
use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use mongodb::bson::doc;
use mongodb::gridfs::{FilesCollectionDocument, GridFsBucket};
use mongodb::options::ClientOptions;
use mongodb::options::GridFsBucketOptions;
use opendal_core::raw::*;
use opendal_core::*;

#[derive(Clone)]
pub struct GridfsCore {
    pub connection_string: String,
    pub database: String,
    pub bucket: String,
    pub chunk_size: u32,
    pub bucket_instance: OnceCell<GridFsBucket>,
}

impl Debug for GridfsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridfsCore")
            .field("database", &self.database)
            .field("bucket", &self.bucket)
            .field("chunk_size", &self.chunk_size)
            .finish_non_exhaustive()
    }
}

impl GridfsCore {
    async fn get_bucket(&self) -> Result<&GridFsBucket> {
        self.bucket_instance
            .get_or_try_init(|| async {
                let client_options = ClientOptions::parse(&self.connection_string)
                    .await
                    .map_err(parse_mongodb_error)?;
                let client =
                    mongodb::Client::with_options(client_options).map_err(parse_mongodb_error)?;
                let bucket_options = GridFsBucketOptions::builder()
                    .bucket_name(Some(self.bucket.clone()))
                    .chunk_size_bytes(Some(self.chunk_size))
                    .build();
                let bucket = client
                    .database(&self.database)
                    .gridfs_bucket(bucket_options);
                Ok(bucket)
            })
            .await
    }

    async fn get_one_doc(
        bucket: &GridFsBucket,
        path: &str,
    ) -> Result<Option<FilesCollectionDocument>> {
        let filter = doc! { "filename": path };
        let doc = bucket.find_one(filter).await.map_err(parse_mongodb_error)?;
        Ok(doc)
    }

    pub async fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let bucket = self.get_bucket().await?;
        let Some(doc) = Self::get_one_doc(bucket, path).await? else {
            return Ok(None);
        };

        let mut destination = Vec::new();
        let file_id = doc.id;
        let mut stream = bucket
            .open_download_stream(file_id)
            .await
            .map_err(parse_mongodb_error)?;
        stream
            .read_to_end(&mut destination)
            .await
            .map_err(new_std_io_error)?;
        Ok(Some(Buffer::from(destination)))
    }

    /// Get the byte length of the file.
    pub async fn get_length(&self, path: &str) -> Result<Option<usize>> {
        let bucket = self.get_bucket().await?;
        let Some(doc) = Self::get_one_doc(bucket, path).await? else {
            return Ok(None);
        };

        Ok(Some(doc.length as usize))
    }

    pub async fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let bucket = self.get_bucket().await?;

        // delete old file if exists
        if let Some(doc) = Self::get_one_doc(bucket, path).await? {
            let file_id = doc.id;
            bucket.delete(file_id).await.map_err(parse_mongodb_error)?;
        };

        // set new file
        let mut upload_stream = bucket
            .open_upload_stream(path)
            .await
            .map_err(parse_mongodb_error)?;
        upload_stream
            .write_all(&value.to_vec())
            .await
            .map_err(new_std_io_error)?;
        upload_stream.close().await.map_err(new_std_io_error)?;

        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let bucket = self.get_bucket().await?;
        let Some(doc) = Self::get_one_doc(bucket, path).await? else {
            return Ok(());
        };

        let file_id = doc.id;
        bucket.delete(file_id).await.map_err(parse_mongodb_error)?;
        Ok(())
    }
}

fn parse_mongodb_error(err: mongodb::error::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "mongodb error").set_source(err)
}
