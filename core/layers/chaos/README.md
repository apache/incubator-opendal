# Apache OpenDAL™ Chaos Layer

`opendal-layer-chaos` injects configurable read failures for robustness testing.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-chaos` feature:

```shell
cargo add opendal --features layers-chaos
```

The layer is available as `opendal::layers::ChaosLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-chaos
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_chaos::ChaosLayer;

fn add_layer(operator: Operator, layer: ChaosLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-chaos)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
