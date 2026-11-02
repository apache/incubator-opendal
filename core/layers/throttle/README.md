# Apache OpenDAL™ Throttle Layer

`opendal-layer-throttle` limits storage bandwidth with configurable rate and burst bounds.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-throttle` feature:

```shell
cargo add opendal --features layers-throttle
```

The layer is available as `opendal::layers::ThrottleLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-throttle
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_throttle::ThrottleLayer;

fn add_layer(operator: Operator, layer: ThrottleLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-throttle)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
