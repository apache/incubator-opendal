# Apache OpenDAL™ HDFS Service

`opendal-service-hdfs` provides access to Hadoop Distributed File System through
`libhdfs` for applications built with Apache OpenDAL™.

## Use through `opendal`

Applications should normally enable this service through the `opendal` facade with the
`services-hdfs` feature:

```shell
cargo add opendal --features services-hdfs
```

The service is available as `opendal::services::Hdfs`. Configure the
service builder, then pass it to `opendal::Operator::new`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-hdfs
```

Pass a configured service builder to `Operator::new`:

```rust
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_hdfs::{register_hdfs_service, Hdfs};

fn build_operator(builder: Hdfs) -> Result<Operator> {
    Operator::new(builder)
}

fn register_for_uri() {
    register_hdfs_service(OperatorRegistry::get());
}
```

`register_for_uri` is only needed for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`.

Services that send HTTP requests also require an HTTP transport in
`OperationContext`. See the
[`opendal-core` composition example](https://crates.io/crates/opendal-core).

For an HDFS client that does not depend on `libhdfs`, use the `hdfs-native` service.

## Documentation

- [Service configuration and examples](https://opendal.apache.org/services/hdfs)
- [Rust API documentation](https://docs.rs/opendal-service-hdfs)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
