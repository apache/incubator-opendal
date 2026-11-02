# Apache OpenDAL™ Logging Layer

`opendal-layer-logging` records OpenDAL operation lifecycles with the `log` facade.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-logging` feature:

```shell
cargo add opendal --features layers-logging
```

The layer is available as `opendal::layers::LoggingLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-logging
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_logging::LoggingLayer;

fn add_layer(operator: Operator, layer: LoggingLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-logging)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
