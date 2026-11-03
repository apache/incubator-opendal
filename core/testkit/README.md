# Apache OpenDAL™ Testkit

`opendal-testkit` provides reusable test actions, data checkers, runtime setup,
and service initialization helpers for Apache OpenDAL™ storage implementations.

This crate is intended for service and integration tests. It is not required by
applications using an OpenDAL operator.

Add it as a development dependency:

```shell
cargo add --dev opendal-testkit
```

The `opendal` facade also re-exports the testkit behind its `tests` feature.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-testkit)
- [Behavior test guide](https://github.com/apache/opendal/tree/main/core/tests/behavior)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## Test operator configuration

`OPENDAL_TEST_IO_TIMEOUT` sets the behavior-test I/O timeout in positive whole seconds.
When unset, the test operator uses the default from `TimeoutLayer`. Service
fixtures can set this variable when their operations need a larger I/O budget.

## License

Licensed under the Apache License, Version 2.0.
