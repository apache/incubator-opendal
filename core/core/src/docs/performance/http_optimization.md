# HTTP Optimization

Most OpenDAL services use HTTP, so transport configuration can dominate
throughput and tail latency. This guide uses reqwest because the `opendal`
facade installs it as the default transport when an HTTP transport feature is
enabled.

The default `auto-register-services` feature calls `opendal::install_default()`
before `main`. Applications that need a different process-wide default must
disable `auto-register-services`, initialize the service registry when needed,
and install their transport before another default is installed.

A simpler option is to replace the transport for one operator. Add
`opendal-http-transport-reqwest` and `reqwest` as direct dependencies, build one
client, and reuse the resulting operator:

```rust,ignore
use opendal::{HttpTransporter, Operator};
use opendal_http_transport_reqwest::ReqwestTransport;

fn with_reqwest_client(op: Operator, client: reqwest::Client) -> Operator {
    let transport = HttpTransporter::new(ReqwestTransport::new(client));
    let ctx = op.base_context().with_http_transport(transport);
    op.with_context(ctx)
}
```

Creating a client or operator per request discards DNS and connection-pool
state and repeatedly pays connection setup costs.

## Prefer HTTP/1.1 for large transfers

For high-throughput uploads and downloads of large objects, benchmark HTTP/1.1
first. OpenDAL users often see higher aggregate throughput with HTTP/1.1 because
reqwest can open several pooled connections, while HTTP/2 normally multiplexes
requests over one connection.

One HTTP/2 connection can become the bottleneck when it reaches its stream
limit or the bandwidth available to one TCP connection. The hyper client does
not currently open another pooled HTTP/2 connection when the existing one
reaches its maximum concurrent streams; see [hyper issue #3623]. This matters
most for long-lived streams and highly concurrent large transfers.

Force HTTP/1.1 with `ClientBuilder::http1_only`:

```rust,ignore
let client = reqwest::Client::builder()
    .http1_only()
    .build()?;
let op = with_reqwest_client(op, client);
```

HTTP/1.1 can open many TCP connections when no idle connection is available.
Bound OpenDAL request concurrency so the client cannot exhaust host resources
or overload the service. HTTP/2 can still be better for many small requests or
when connection count matters, so measure both protocols against the real
endpoint and object-size distribution.

## Enable DNS caching

Without the optional Hickory resolver, reqwest uses a `getaddrinfo`-based
resolver and does not maintain a reqwest-level DNS cache. Repeated resolution
can become visible at high request rates and can overload a constrained local
resolver.

Enable reqwest's `hickory-dns` feature in the application and select it on the
client:

```toml
[dependencies]
reqwest = { version = "0.13", features = ["hickory-dns"] }
```

```rust,ignore
let client = reqwest::Client::builder()
    .hickory_dns(true)
    .build()?;
let op = with_reqwest_client(op, client);
```

The built-in Hickory settings are a good starting point. Use
`ClientBuilder::dns_resolver` when the application needs to share a custom
resolver or control cache size, positive and negative TTLs, IP selection, or
address shuffling. Longer TTLs reduce resolver traffic but retain stale records
longer, so test failover behavior before increasing them.

## Set timeouts deliberately

Reqwest 0.13 has no request, read, or connect timeout by default. At minimum,
set a connect timeout so an unreachable endpoint cannot leave connection setup
unbounded:

```rust,ignore
use std::time::Duration;

let client = reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(5))
    .read_timeout(Duration::from_secs(30))
    .build()?;
let op = with_reqwest_client(op, client);
```

`read_timeout` limits inactivity between successful reads. A total request
timeout created with `ClientBuilder::timeout` covers the complete transfer, so
do not set it below the expected duration of a healthy large upload or
download.

The facade's optional `layers-timeout` feature also provides `TimeoutLayer` for
OpenDAL operation and I/O timeouts:

```rust,ignore
use std::time::Duration;

use opendal::layers::TimeoutLayer;

let op = op.layer(
    TimeoutLayer::new()
        .with_timeout(Duration::from_secs(30))
        .with_io_timeout(Duration::from_secs(60)),
);
```

Transport timeouts bound HTTP phases. `TimeoutLayer` applies at OpenDAL's
operation and I/O boundaries. Configure both when the application needs bounds
at both levels.

## Size the connection pool

Reqwest reuses idle connections. In reqwest 0.13, the default idle timeout is
90 seconds and the default maximum number of idle connections per host is
unlimited. Tune both values for the traffic pattern; neither setting limits
active connections:

```rust,ignore
use std::time::Duration;

let client = reqwest::Client::builder()
    .pool_idle_timeout(Duration::from_secs(60))
    .pool_max_idle_per_host(32)
    .build()?;
let op = with_reqwest_client(op, client);
```

- Keep enough idle connections for expected HTTP/1.1 concurrency so bursts do
  not repeat DNS, TCP, and TLS setup.
- Set a finite idle per-host limit for applications that contact many
  endpoints. Bound active connections with OpenDAL request concurrency.
- Shorten the idle timeout when traffic is sparse and connection reuse is rare.

Measure connection reuse, open sockets, handshake rate, request concurrency,
throughput, and tail latency together. A pool setting that improves a steady
single-endpoint workload can waste resources in a bursty multi-endpoint one.

[hyper issue #3623]: https://github.com/hyperium/hyper/issues/3623
