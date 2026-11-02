# Apache OpenDAL™ Tracing Layer

`opendal-layer-tracing` emits spans for OpenDAL operations and deferred I/O bodies with `tracing`.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-tracing` feature:

```shell
cargo add opendal --features layers-tracing
```

The layer is available as `opendal::layers::TracingLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-tracing
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_tracing::TracingLayer;

fn add_layer(operator: Operator, layer: TracingLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-tracing)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
