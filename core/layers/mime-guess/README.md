# Apache OpenDAL™ MIME Guess Layer

`opendal-layer-mime-guess` infers missing content types from object path extensions.

## Use through `opendal`

Applications should normally enable this layer through the `opendal` facade with the
`layers-mime-guess` feature:

```shell
cargo add opendal --features layers-mime-guess
```

The layer is available as `opendal::layers::MimeGuessLayer` and can be
composed with an operator through `opendal::Operator::layer`.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-layer-mime-guess
```

Pass a configured layer to `Operator::layer`:

```rust
use opendal_core::Operator;
use opendal_layer_mime_guess::MimeGuessLayer;

fn add_layer(operator: Operator, layer: MimeGuessLayer) -> Operator {
    operator.layer(layer)
}
```

Construct and configure `layer` as described in the API documentation before
composing it with an operator.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-layer-mime-guess)
- [Apache OpenDAL production guide](https://opendal.apache.org/docs/core/production)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
