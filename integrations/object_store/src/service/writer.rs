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

use std::sync::Arc;

use object_store::MultipartUpload;
use object_store::ObjectStore;
use object_store::PutPayload;
use object_store::path::Path as ObjectStorePath;
use object_store::{Attribute, AttributeValue};

use asyncband::mutex::Mutex;
use opendal::raw::oio::MultipartPart;
use opendal::raw::*;
use opendal::*;

use super::core::{format_put_multipart_options, format_put_result, parse_op_write};
use super::error::parse_error;

pub struct ObjectStoreWriter {
    store: Arc<dyn ObjectStore + 'static>,
    path: ObjectStorePath,
    args: OpWrite,
    upload: Mutex<Option<Box<dyn MultipartUpload>>>,
}

impl ObjectStoreWriter {
    pub fn new(store: Arc<dyn ObjectStore + 'static>, path: &str, args: OpWrite) -> Self {
        Self {
            store,
            path: ObjectStorePath::from(path),
            args,
            upload: Mutex::new(None),
        }
    }
}

impl oio::MultipartWrite for ObjectStoreWriter {
    /// Write the entire object in one go.
    /// Used when the object is small enough to bypass multipart upload.
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        // Validate that actual body size matches expected size
        let actual_size = body.len() as u64;
        if actual_size != size {
            return Err(Error::new(
                ErrorKind::Unexpected,
                format!("Expected size {size} but got {actual_size}"),
            ));
        }

        let bytes = body.to_bytes();
        let payload = PutPayload::from(bytes);
        let mut opts = parse_op_write(&self.args)?;

        // Add size metadata for tracking
        opts.attributes.insert(
            Attribute::Metadata("content-size".into()),
            AttributeValue::from(size.to_string()),
        );

        let result = self
            .store
            .put_opts(&self.path, payload, opts)
            .await
            .map_err(parse_error)?;

        // Build metadata from put result
        let mut metadata = MetadataBuilder::unknown();
        if let Some(etag) = &result.e_tag {
            metadata.etag(etag);
        }
        if let Some(version) = &result.version {
            metadata.version(version);
        }

        Ok(metadata.build())
    }

    // Generate a unique upload ID that we'll use to track this session
    async fn initiate_part(&self) -> Result<String> {
        // Start a new multipart upload using object_store
        let opts = parse_op_write(&self.args)?;
        let multipart_opts = format_put_multipart_options(opts);
        let upload = self
            .store
            .put_multipart_opts(&self.path, multipart_opts)
            .await
            .map_err(parse_error)?;

        // Store the multipart upload for later use
        let mut guard = self.upload.lock().await;
        if guard.is_some() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "Upload already initiated, abort the previous upload first",
            ));
        }
        *guard = Some(upload);

        // object_store does not provide a way to get the upload id, so we use a fixed string
        // as the upload id. it's ok because the upload id is already tracked inside the upload
        // object.
        Ok("".to_string())
    }

    /// Upload a single part of the multipart upload.
    /// Part numbers must be sequential starting from 1.
    /// Returns the ETag and part information for this uploaded part.
    async fn write_part(
        &self,
        _upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<MultipartPart> {
        // Validate that actual body size matches expected size
        let actual_size = body.len() as u64;
        if actual_size != size {
            return Err(Error::new(
                ErrorKind::Unexpected,
                format!("Expected size {size} but got {actual_size}"),
            ));
        }

        // Convert Buffer to PutPayload
        let bytes = body.to_bytes();

        // Return empty string as ETag since it's not used by object_store
        let etag = String::new();

        let payload = PutPayload::from(bytes);

        // Upload the part
        let mut guard = self.upload.lock().await;
        let upload = guard
            .as_mut()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "Upload not initiated"))?;
        upload.put_part(payload).await.map_err(parse_error)?;

        // Create MultipartPart with the proper ETag
        let multipart_part = MultipartPart {
            part_number,
            etag,
            checksum: None, // No checksum for now
            size: None,
        };
        Ok(multipart_part)
    }

    async fn complete_part(&self, _upload_id: &str, parts: &[MultipartPart]) -> Result<Metadata> {
        // Validate that we have parts to complete
        if parts.is_empty() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "Cannot complete multipart upload with no parts",
            ));
        }

        // Get the multipart upload for this upload_id
        let mut guard = self.upload.lock().await;
        let upload = guard
            .as_mut()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "Upload not initiated"))?;

        // Complete the multipart upload
        let result = upload.complete().await.map_err(parse_error)?;
        *guard = None;

        // Build metadata from the result
        let metadata = format_put_result(result);
        Ok(metadata)
    }

    async fn abort_part(&self, _upload_id: &str) -> Result<()> {
        // Get the multipart upload for this upload_id
        let mut guard = self.upload.lock().await;
        let upload = guard
            .as_mut()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "Upload not initiated"))?;

        // Abort the multipart upload
        upload.abort().await.map_err(parse_error)?;
        *guard = None;

        Ok(())
    }
}
