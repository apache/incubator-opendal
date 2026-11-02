# Apache OpenDAL™ Concurrent Limit Layer

`opendal-layer-concurrent-limit` limits concurrent storage and HTTP requests and can
share one limit across operators.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-concurrent-limit` feature:

```shell
cargo add opendal --features layers-concurrent-limit
```

The layer is available as `opendal::layers::ConcurrentLimitLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-concurrent-limit
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_concurrent_limit::ConcurrentLimitLayer;

fn add_layer(operator: Operator, layer: ConcurrentLimitLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-concurrent-limit)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
