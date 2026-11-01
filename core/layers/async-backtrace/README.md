# Apache OpenDAL™ Async Backtrace Layer

`opendal-layer-async-backtrace` records logical stack traces for asynchronous service
operations with `async-backtrace`.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-async-backtrace` feature:

```shell
cargo add opendal --features layers-async-backtrace
```

The layer is available as `opendal::layers::AsyncBacktraceLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-async-backtrace
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_async_backtrace::AsyncBacktraceLayer;

fn add_layer(operator: Operator, layer: AsyncBacktraceLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-async-backtrace)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
