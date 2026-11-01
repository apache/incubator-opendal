# Apache OpenDAL™ HDFS Native Service

`opendal-service-hdfs-native` provides access to Hadoop Distributed File System through
the native Rust `hdfs-native` client for applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-hdfs-native` feature:

```shell
cargo add opendal --features services-hdfs-native
```

The service is available as `opendal::services::HdfsNative`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-hdfs-native
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_hdfs_native::{register_hdfs_native_service, HdfsNative};

fn build_operator(builder: HdfsNative) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_hdfs_native_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/hdfs-native)
- [Rust API documentation](https://docs.rs/opendal-service-hdfs-native)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
