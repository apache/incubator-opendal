# Apache OpenDAL Google Cloud Storage gRPC Service

This crate provides an OpenDAL service implementation for the Google Cloud
Storage gRPC API.

The implementation uses a generated `google.storage.v2.Storage` client and does
not depend on the Google Cloud Storage Rust client library.

```no_run
use opendal_core::Operator;
use opendal_service_gcs_grpc::GcsGrpc;

let builder = GcsGrpc::default()
    .bucket("my-bucket")
    .root("my-prefix");
let operator = Operator::new(builder)?;

# Ok::<(), opendal_core::Error>(())
```
