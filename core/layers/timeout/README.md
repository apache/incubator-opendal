# Apache OpenDAL™ Timeout Layer

`opendal-layer-timeout` applies deadlines to control operations and stateful I/O bodies.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-timeout` feature:

```shell
cargo add opendal --features layers-timeout
```

The layer is available as `opendal::layers::TimeoutLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-timeout
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_timeout::TimeoutLayer;

fn add_layer(operator: Operator, layer: TimeoutLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

Layer order matters when timeout is combined with retry. Review the API documentation
before composing both layers.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-timeout)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
