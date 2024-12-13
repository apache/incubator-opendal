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

//! object_store_opendal is an object store implementation using opendal.
//!
//! This crate can help you to access 30 more storage services with the same object_store API.
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use bytes::Bytes;
//! use object_store::path::Path;
//! use object_store::ObjectStore;
//! use object_store_opendal::OpendalStore;
//! use opendal::services::S3;
//! use opendal::{Builder, Operator};
//!
//! #[tokio::main]
//! async fn main() {
//!    let builder = S3::default()
//!     .access_key_id("my_access_key")
//!     .secret_access_key("my_secret_key")
//!     .endpoint("my_endpoint")
//!     .region("my_region");
//!
//!     // Create a new operator
//!     let operator = Operator::new(builder).unwrap().finish();
//!
//!     // Create a new object store
//!     let object_store = Arc::new(OpendalStore::new(operator));
//!
//!     let path = Path::from("data/nested/test.txt");
//!     let bytes = Bytes::from_static(b"hello, world! I am nested.");
//!
//!     object_store.put(&path, bytes.clone().into()).await.unwrap();
//!
//!     let content = object_store
//!         .get(&path)
//!         .await
//!         .unwrap()
//!         .bytes()
//!         .await
//!         .unwrap();
//!
//!     assert_eq!(content, bytes);
//! }
//! ```

mod store;
pub use store::OpendalStore;

mod utils;

#[cfg(feature = "services-s3")]
mod s3_builder;
#[cfg(feature = "services-s3")]
pub use s3_builder::AmazonS3Builder;

// Make sure `send_wrapper` works as expected
#[cfg(all(feature = "send_wrapper", target_arch = "wasm32"))]
mod assert_send {
    use object_store::ObjectStore;

    #[allow(dead_code)]
    fn assert_send<T: Send>(_: T) {}

    #[allow(dead_code)]
    fn assertion() {
        let op = super::Operator::new(opendal::services::Memory::default())
            .unwrap()
            .finish();
        let store = super::OpendalStore::new(op);
        assert_send(store.put(&"test".into(), bytes::Bytes::new()));
        assert_send(store.get(&"test".into()));
        assert_send(store.get_range(&"test".into(), 0..1));
        assert_send(store.head(&"test".into()));
        assert_send(store.delete(&"test".into()));
        assert_send(store.list(None));
        assert_send(store.list_with_offset(None, &"test".into()));
        assert_send(store.list_with_delimiter(None));
    }
}
