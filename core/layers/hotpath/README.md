# Apache OpenDAL™ Hotpath Layer

`opendal-layer-hotpath` profiles OpenDAL operations and HTTP work with `hotpath`.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-hotpath` feature:

```shell
cargo add opendal --features layers-hotpath
```

The layer is available as `opendal::layers::HotpathLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-hotpath
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_hotpath::HotpathLayer;

fn add_layer(operator: Operator, layer: HotpathLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-hotpath)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
