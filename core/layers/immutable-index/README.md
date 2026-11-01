# Apache OpenDAL™ Immutable Index Layer

`opendal-layer-immutable-index` adds an immutable in-memory listing index to services
that lack native list support.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-immutable-index` feature:

```shell
cargo add opendal --features layers-immutable-index
```

The layer is available as `opendal::layers::ImmutableIndexLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-immutable-index
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_immutable_index::ImmutableIndexLayer;

fn add_layer(operator: Operator, layer: ImmutableIndexLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-immutable-index)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
