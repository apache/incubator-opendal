# Apache OpenDAL™ Observability Metrics Common

`opendal-layer-observe-metrics-common` contains the shared metric definitions,
labels, histogram boundaries, and instrumentation adapters used by Apache
OpenDAL™ metrics layers.

This crate is an implementation building block. Applications should normally
select a public metrics layer through the `opendal` facade, such as:

- `layers-metrics`;
- `layers-otel-metrics`;
- `layers-prometheus`;
- `layers-prometheus-client`;
- `layers-fastmetrics`.

Metrics-layer implementers can depend on this crate directly to preserve the
same operation and HTTP metric semantics.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-observe-metrics-common)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
