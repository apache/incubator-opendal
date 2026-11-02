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

#[cfg(feature = "services-s3")]
use bytes::Bytes;
#[cfg(feature = "services-s3")]
use object_store::ObjectStoreExt;
#[cfg(feature = "services-s3")]
use object_store::aws::AmazonS3Builder;
#[cfg(feature = "services-s3")]
use object_store::path::Path as ObjectStorePath;
#[cfg(feature = "services-s3")]
use object_store_opendal::OpendalStore;

#[cfg(feature = "services-s3")]
#[tokio::main]
async fn main() {
    let builder = AmazonS3Builder::default()
        .with_bucket_name("my_bucket")
        .with_region("my_region")
        .with_endpoint("my_endpoint")
        .with_access_key_id("my_access_key")
        .with_secret_access_key("my_secret_access_key");
    let s3_store = OpendalStore::new_amazon_s3(builder).unwrap();

    let path = ObjectStorePath::from("data/nested/test.txt");
    let bytes = Bytes::from_static(b"hello, world! I am nested.");

    s3_store.put(&path, bytes.clone().into()).await.unwrap();

    let content = s3_store.get(&path).await.unwrap().bytes().await.unwrap();

    assert_eq!(content, bytes);
}

#[cfg(not(feature = "services-s3"))]
fn main() {
    println!("The 'services-s3' feature is not enabled.");
}
