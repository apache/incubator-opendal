# Apache OpenDAL™ OpenTelemetry Trace Layer

`opendal-layer-oteltrace` traces supported OpenDAL operations with
OpenTelemetry's global tracer provider.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-otel-trace` feature:

```shell
cargo add opendal --features layers-otel-trace
```

The layer is available as `opendal::layers::OtelTraceLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-oteltrace
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_oteltrace::OtelTraceLayer;

fn add_layer(operator: Operator, layer: OtelTraceLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-oteltrace)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
