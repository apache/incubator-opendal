# Apache OpenDAL™ DTrace Layer

`opendal-layer-dtrace` emits User Statically-Defined Tracing probes for OpenDAL operations.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-dtrace` feature:

```shell
cargo add opendal --features layers-dtrace
```

The layer is available as `opendal::layers::DtraceLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-dtrace
```

Pass a configured layer to `Operator::layer`:

```rust
#[cfg(target_os = "linux")]
use opendal_core::Operator;
#[cfg(target_os = "linux")]
use opendal_layer_dtrace::DtraceLayer;

#[cfg(target_os = "linux")]
fn add_layer(operator: Operator, layer: DtraceLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

This experimental layer is available on Linux.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-dtrace)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
