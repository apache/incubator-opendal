# Apache OpenDAL™ Capability Check Layer

`opendal-layer-capability-check` validates optional operation arguments against the
capabilities reported by a storage service.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-capability-check` feature:

```shell
cargo add opendal --features layers-capability-check
```

The layer is available as `opendal::layers::CapabilityCheckLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-capability-check
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_capability_check::CapabilityCheckLayer;

fn add_layer(operator: Operator, layer: CapabilityCheckLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-capability-check)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
