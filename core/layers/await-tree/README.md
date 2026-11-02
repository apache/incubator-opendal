# Apache OpenDAL™ Await Tree Layer

`opendal-layer-await-tree` instruments service operations so `await-tree` can expose
their execution trees.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-await-tree` feature:

```shell
cargo add opendal --features layers-await-tree
```

The layer is available as `opendal::layers::AwaitTreeLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-await-tree
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_await_tree::AwaitTreeLayer;

fn add_layer(operator: Operator, layer: AwaitTreeLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-await-tree)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
