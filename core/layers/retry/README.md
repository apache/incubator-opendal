# Apache OpenDAL™ Retry Layer

`opendal-layer-retry` retries temporarily failed operations and stateful I/O bodies.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-retry` feature:

```shell
cargo add opendal --features layers-retry
```

The layer is available as `opendal::layers::RetryLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-retry
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_retry::RetryLayer;

fn add_layer(operator: Operator, layer: RetryLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

Layer order matters when retry is combined with timeout. Review the API documentation
before composing both layers.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-retry)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
