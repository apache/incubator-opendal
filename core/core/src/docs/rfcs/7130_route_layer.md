- Proposal Name: `route_layer`
- Start Date: 2026-01-04
- RFC PR: [apache/opendal#7130](https://github.com/apache/opendal/pull/7130)
- Tracking Issue: [apache/opendal#7131](https://github.com/apache/opendal/issues/7131)

# Summary

Introduce `RouteLayer`, a layer that dispatches each OpenDAL operation to one of multiple pre-built `Operator` stacks by matching the operation path against user-provided glob patterns.

# Motivation

In practice, OpenDAL users often want different policies for different parts of a namespace:

- Apply caching only for frequently-read objects (e.g., `**/*.parquet`).
- Use a tighter timeout for latency-sensitive paths (e.g., `hot/**`).
- Attach different observability or throttling configurations per dataset.

Today, users can achieve this by building multiple `Operator`s and routing requests in application code. However, this approach duplicates routing logic across projects and makes it harder to share a consistent, well-tested routing behavior across bindings and integrations.

`RouteLayer` centralizes this routing as a reusable primitive while preserving OpenDAL's existing composition model: users still build independent `Operator` stacks for each policy, and `RouteLayer` only decides which stack should handle a request.

# Guide-level explanation

## Enable feature

```toml
opendal = { version = "*", features = ["layers-route"] }
```

## Build per-route operators

Build each routed `Operator` as usual, including its own service configuration and layer stack.

```rust
use std::time::Duration;

use opendal::layers::RouteLayer;
use opendal::services;
use opendal::Operator;
use opendal::Result;
use opendal_layer_timeout::TimeoutLayer;

fn build_default() -> Result<Operator> {
    Operator::new(services::Memory::default())?.finish()
}

fn build_parquet_fast_path() -> Result<Operator> {
    Operator::new(services::Memory::default())?
        .layer(TimeoutLayer::default().with_timeout(Duration::from_secs(3)))
        .finish()
}
```

## Combine them with `RouteLayer`

`RouteLayer` is applied to the default operator. When none of the patterns match, the request is handled by the default operator.

Patterns are evaluated in insertion order. The first matching pattern wins.

```rust
use opendal::Operator;
use opendal::Result;

fn build_routed_operator() -> Result<Operator> {
    let default_op = build_default()?;
    let parquet_op = build_parquet_fast_path()?;

    let routed = default_op.layer(
        RouteLayer::builder()
            .route("**/*.parquet", parquet_op)
            .build()?,
    );

    Ok(routed)
}
```

## Pattern rules

`RouteLayer` uses glob patterns (via `globset`) and does not perform any implicit expansion.

- `*.parquet` matches `file.parquet` in the root, but does not match `dir/file.parquet`.
- `**/*.parquet` matches `dir/file.parquet` and any other depth.

Paths are matched against OpenDAL normalized paths as seen by the accessor:

- The root is represented as `/`.
- Non-root paths do not start with `/` (e.g., `dir/file`).
- Directory paths end with `/` (e.g., `dir/`).

## Routing for `copy` and `rename`

For `copy(from, to, ..)` and `rename(from, to, ..)`, routing is decided only by `from`. The `to` path is forwarded unchanged to the selected operator.

# Reference-level explanation

## Public API

`RouteLayer` is constructed via a fallible builder, because glob compilation can fail and `Layer::layer()` cannot return `Result`.

```rust
pub struct RouteLayer { /* compiled router */ }

impl RouteLayer {
    pub fn builder() -> RouteLayerBuilder;
}

pub struct RouteLayerBuilder { /* patterns + operators */ }

impl RouteLayerBuilder {
    pub fn route(self, pattern: impl AsRef<str>, op: Operator) -> Self;
    pub fn build(self) -> Result<RouteLayer>;
}
```

`RouteLayer` is intended to be applied to a fully-built `Operator` (dynamic dispatch). Routed operators are also fully-built and independent.

## Internal structure

`RouteLayer` holds:

- `glob: globset::GlobSet`, compiled from patterns in insertion order.
- `targets: Vec<Accessor>`, where `targets[i]` is the accessor corresponding to the `i`-th inserted pattern.

At `build()` time:

1. Compile each pattern into a `globset::Glob`.
2. Insert each glob into a `globset::GlobSetBuilder` in insertion order.
3. Convert each routed `Operator` into its `Accessor` and store it in `targets` in the same order.

Any glob compilation error results in `build()` returning an `ErrorKind::InvalidInput` error with the pattern and the underlying source error attached.

## Dispatch algorithm

For an operation that includes a path `p`:

1. Compute `matches = glob.matches(p)`, yielding a set of matching pattern indices.
2. Select `idx = min(matches)` as the chosen route (first match in insertion order).
3. If no match exists, dispatch to the inner accessor (the default operator that `RouteLayer` is applied to).

This approach does not rely on any ordering guarantees from `globset` for returned matches.

### Operations with paths

The following operations are dispatched based on their `path` argument:

- `create_dir(path, ..)`
- `read(path, ..)`
- `write(path, ..)`
- `stat(path, ..)`
- `list(path, ..)`
- `presign(path, ..)`

### Operations with two paths

The following operations are dispatched based on `from` only:

- `copy(from, to, ..)`
- `rename(from, to, ..)`

### Delete semantics

`Access::delete()` returns a deleter that can queue multiple deletions and execute them in `close()`. To preserve this batching behavior while supporting per-path routing, `RouteLayer` returns a `RouteDeleter`:

- `RouteDeleter::delete(path, ..)` routes by `path` and lazily creates (and caches) the underlying deleter for the selected target accessor.
- `RouteDeleter::close()` closes all created deleters and returns an error if any close fails.

This design batches deletions per routed operator and avoids creating deleters for unused routes.

## `info()` behavior

`RouteAccessor::info()` forwards to the default (inner) accessor's `AccessorInfo`. This keeps `Operator::info()` stable and avoids synthesizing a potentially misleading merged capability across routed operators.

Users that need per-route `AccessorInfo` can query the routed `Operator`s directly before passing them into the builder.

# Drawbacks

- Order-dependent routing can be error-prone when multiple patterns overlap; users must manage ordering carefully.
- `Operator::info()` reflects the default operator, not a merged view across routes.
- `copy`/`rename` routing based only on `from` can lead to surprising behavior if the destination logically belongs to a different route; this is accepted for simplicity.
- Adds a new dependency (`globset`) and introduces a small per-operation routing cost.

# Rationale and alternatives

## Why glob patterns via `globset`?

- Glob patterns are a well-known way to express path-based rules and cover common needs like `**/*.parquet`.
- `globset` compiles patterns into an efficient matcher and provides a `matches` API suitable for first-match selection without scanning all patterns with separate matchers.

## Why first-match-wins without priority?

First-match-wins keeps the model explicit and deterministic without additional concepts. Rule ordering is visible in code review and can be reasoned about locally.

## Alternatives considered

- **User-side routing**: build multiple operators and dispatch in application code. Works today but duplicates logic and increases maintenance burden across projects.
- **Prefix-only routing**: efficient but does not satisfy suffix-based rules like `**/*.parquet`.
- **Implicit expansion (e.g., treating `*.parquet` as `**/*.parquet`)**: simplifies some patterns but introduces surprising behavior; this proposal requires explicit `**`.
- **Routing on operation type or options**: valuable, but out of scope for this RFC and reserved for future work.

# Prior art

- `gitignore`-style globs for hierarchical path matching.
- HTTP routers and proxy configuration models (e.g., ordered rules with first-match semantics).
- Data lake engines commonly route by file suffix (e.g., `.parquet`) to specialized code paths.

# Unresolved questions

None

# Future possibilities

- Extend routing conditions beyond path, such as operation type (read/write/list) or per-operation options.
- Support loading rules from configuration files or environment variables for CLI tools and integrations.
