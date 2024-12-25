use std::sync::Arc;

use bytes::Bytes;
use object_store::path::Path;
use object_store::ObjectStore;

#[cfg(feature = "services-s3")]
use object_store_opendal::AmazonS3Builder;

#[cfg(feature = "services-s3")]
#[tokio::main]
async fn main() {
    let s3_store = AmazonS3Builder::new()
        .with_access_key_id("my_access_key")
        .with_secret_access_key("my_secret_key")
        .with_endpoint("my_endpoint")
        .with_region("my_region")
        .with_bucket_name("my_bucket")
        .build()
        .unwrap();

    let object_store = Arc::new(s3_store);

    let path = Path::from("data/nested/test.txt");
    let bytes = Bytes::from_static(b"hello, world! I am nested.");

    object_store.put(&path, bytes.clone().into()).await.unwrap();

    let content = object_store
        .get(&path)
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();

    assert_eq!(content, bytes);
}

#[cfg(not(feature = "services-s3"))]
fn main() {
    println!("The 'services-s3' feature is not enabled.");
}
