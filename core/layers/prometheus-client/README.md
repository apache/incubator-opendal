# Apache OpenDAL™ Prometheus Client Layer

`opendal-layer-prometheus-client` registers OpenDAL operation and HTTP metrics with
`prometheus-client`.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-prometheus-client` feature:

```shell
cargo add opendal --features layers-prometheus-client
```

The layer is available as `opendal::layers::PrometheusClientLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-prometheus-client
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_prometheus_client::PrometheusClientLayer;

fn add_layer(operator: Operator, layer: PrometheusClientLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-prometheus-client)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
