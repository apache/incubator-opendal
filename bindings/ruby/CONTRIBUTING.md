# Contributing to Apache OpenDAL™ Ruby Binding

## Development

Install gem and its dependencies:

```shell
bundle
```

Build bindings:

```shell
bundle exec rake compile
```

Run tests:

```shell
bundle exec rake test:base

# Optional
OPENDAL_TEST=fs bundle exec rake test:service
```

Run linters:

```shell
bundle exec rake standard:fix
rustfmt --config-path ../../rustfmt.toml src/*.rs # Run rustfmt for Rust files
cargo clippy --fix --all-targets # Run rust linter clippy
```
