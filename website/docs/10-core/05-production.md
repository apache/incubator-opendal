---
title: Going to production
sidebar_label: Going to production
description: Make OpenDAL robust in production — retries, timeouts, concurrency limits, error handling, and observability.
---

# Going to production

The basics read and write data. Production code also has to survive transient
failures, bound its resource use, observe what happens, and react to errors.
OpenDAL handles the first three with **layers** and gives you typed **errors**
and **capability checks** for the rest.

## Layers

A layer wraps an operator to add cross-cutting behavior without touching your
storage code. Apply one with `.layer()`; it returns a new operator. Layers nest
like an onion, and the **last** layer applied is the **outermost** — it sees the
request first and the response last. A robust order is logging on the outside,
retry on the inside, so each retry attempt is logged:

```rust
use std::time::Duration;
use opendal::layers::{ConcurrentLimitLayer, LoggingLayer, RetryLayer, TimeoutLayer};

let op = Operator::new(builder)?
    .layer(RetryLayer::new().with_max_times(3))      // retry transient failures
    .layer(TimeoutLayer::new().with_timeout(Duration::from_secs(10))) // bound slow calls
    .layer(ConcurrentLimitLayer::new(16))            // cap in-flight requests
    .layer(LoggingLayer::default());                 // log every attempt (outermost)
```

These four ship in the default features. See [Concepts](../03-concepts.mdx#layer)
for the model.

## Observability

`LoggingLayer` (above) emits logs through the `log` crate. For metrics and
tracing, enable the matching feature and apply the layer the same way:

| Feature | Layer | Use it for |
|---------|-------|------------|
| `layers-metrics` | `MetricsLayer` | `metrics`-crate counters and histograms |
| `layers-prometheus` | `PrometheusLayer` | Prometheus metrics |
| `layers-tracing` | `TracingLayer` | `tracing` spans |
| `layers-fastrace` | `FastraceLayer` | distributed tracing with fastrace |
| `layers-otel-trace` | `OtelTraceLayer` | OpenTelemetry traces |

See the [`layers` module][layers] for the full list and each layer's options.

## Error handling

Every fallible call returns `opendal::Result<T>`, an alias for
`Result<T, opendal::Error>`. Match on [`ErrorKind`] to branch on what went wrong
instead of parsing messages:

```rust
use opendal::ErrorKind;

match op.read("maybe-missing.txt").await {
    Ok(bytes) => { /* ... */ }
    Err(err) if err.kind() == ErrorKind::NotFound => { /* handle absence */ }
    Err(err) => return Err(err),
}
```

Common kinds include `NotFound`, `PermissionDenied`, `AlreadyExists`,
`RateLimited`, and `Unsupported`. Note that `delete` is idempotent — deleting a
missing path succeeds rather than returning `NotFound`.

## Capability checks {#capability-checks}

Not every service supports every operation. Query what a backend can do with
[`Capability`] before calling optional operations like `copy`, `rename`, or
presign:

```rust
let cap = op.info().capability();
if cap.copy {
    op.copy("a.txt", "b.txt").await?;
}
```

Calling an unsupported operation returns an error with `ErrorKind::Unsupported`,
so capability checks are an optimization, not a requirement for safety.

[layers]: https://docs.rs/opendal/latest/opendal/layers/index.html
[`ErrorKind`]: https://docs.rs/opendal/latest/opendal/enum.ErrorKind.html
[`Capability`]: https://docs.rs/opendal/latest/opendal/struct.Capability.html
