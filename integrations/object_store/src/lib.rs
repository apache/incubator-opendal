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
//! use object_store::ObjectStoreExt;
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
//!     let operator = Operator::new(builder).unwrap();
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
//!
//! Use the re-exported future helper when an integration needs the same OpenDAL
//! future wrapper used by this crate:
//!
//! ```no_run
//! use object_store_opendal::IntoSendFuture;
//! use opendal::Operator;
//!
//! async fn rename(op: Operator) -> opendal::Result<()> {
//!     op.rename("from", "to").into_send().await
//! }
//! ```

mod store;
pub use store::OpendalStore;

mod utils;
pub use utils::IntoSendFuture;

#[cfg(feature = "services-s3")]
mod amazon_s3;

mod service;

pub use service::{ObjectStoreBuilder, ObjectStoreService};

// Make sure `send_wrapper` works as expected
#[cfg(all(feature = "send_wrapper", test))]
mod assert_send {
    use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
    use opendal::Operator;

    #[allow(dead_code)]
    fn assert_send<T: Send>(_: T) {}

    #[allow(dead_code)]
    fn assertion() {
        let op = Operator::new(opendal::services::Memory::default()).unwrap();
        let store = super::OpendalStore::new(op);
        assert_send(store.put(&"test".into(), PutPayload::new()));
        assert_send(store.get(&"test".into()));
        assert_send(store.get_range(&"test".into(), 0..1));
        assert_send(store.head(&"test".into()));
        assert_send(store.delete(&"test".into()));
        assert_send(store.list(None));
        assert_send(store.list_with_offset(None, &"test".into()));
        assert_send(store.list_with_delimiter(None));
    }
}

fn timestamp_to_datetime(ts: opendal::raw::Timestamp) -> Option<chrono::DateTime<chrono::Utc>> {
    let jiff_ts = ts.into_inner();
    chrono::DateTime::<chrono::Utc>::from_timestamp(
        jiff_ts.as_second(),
        jiff_ts.subsec_nanosecond() as u32,
    )
}

fn datetime_to_timestamp(dt: chrono::DateTime<chrono::Utc>) -> Option<opendal::raw::Timestamp> {
    opendal::raw::Timestamp::new(dt.timestamp(), dt.timestamp_subsec_nanos() as i32).ok()
}
