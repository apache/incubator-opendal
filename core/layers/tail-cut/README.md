# Apache OpenDAL™ Tail Cut Layer

`opendal-layer-tail-cut` uses observed latency to cancel long-tail requests with adaptive deadlines.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-tail-cut` feature:

```shell
cargo add opendal --features layers-tail-cut
```

The layer is available as `opendal::layers::TailCutLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-tail-cut
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_tail_cut::TailCutLayer;

fn add_layer(operator: Operator, layer: TailCutLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-tail-cut)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
