# Apache OpenDAL™ Route Layer

`opendal-layer-route` routes operations to different operators by matching paths against
glob patterns.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-route` feature:

```shell
cargo add opendal --features layers-route
```

The layer is available as `opendal::layers::RouteLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-route
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_route::RouteLayer;

fn add_layer(operator: Operator, layer: RouteLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Routing semantics

- Routes are evaluated in the order they are added. The first matching glob
  wins.
- Paths that match no route continue through the operator wrapped by the
  layer.
- Copy and rename select a service from the source path. The layer does not
  transfer data between two routed services.
- Batched deletes route each path independently.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-route)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
