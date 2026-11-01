# Apache OpenDAL™ Fastrace Layer

`opendal-layer-fastrace` traces OpenDAL operations and deferred I/O bodies with `fastrace`.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-fastrace` feature:

```shell
cargo add opendal --features layers-fastrace
```

The layer is available as `opendal::layers::FastraceLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-fastrace
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_fastrace::FastraceLayer;

fn add_layer(operator: Operator, layer: FastraceLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-fastrace)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
