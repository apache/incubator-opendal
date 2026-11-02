- Proposal Name: `http_transporter`
- Start Date: 2026-06-16
- RFC PR: [apache/opendal#7749](https://github.com/apache/opendal/pull/7749)
- Tracking Issue: [apache/opendal#0000](https://github.com/apache/opendal/issues/0000)

# Summary

Move OpenDAL's HTTP transport abstraction out of `raw` and rename the current
`HttpClient` / `HttpFetch` / `HttpFetcher` set to `HttpTransport` /
`HttpTransportDyn` / `HttpTransporter`.

`opendal-core` defines only the HTTP transport contract and the erased transport
handle. The default reqwest implementation moves into a new
`opendal-http-transport-reqwest` crate. The `opendal` facade installs its default
components through a single `install_default()` entrypoint: enabled services are
registered into `OperatorRegistry`, and the default reqwest transport is
installed when the `http-transport-reqwest` feature is enabled.

# Motivation

`Operator::http_client(self, client: HttpClient)` has made `HttpClient` part of
OpenDAL's public API, but the type is still owned by `raw/http_util` and exported
through `opendal::raw`. This contradicts the raw API boundary: raw APIs are for
service and layer implementors and are less stable than the public operator API.

The current names also expose implementation details as user concepts. Users
want to replace the HTTP transport, but the API asks them to understand
`HttpClient` as a handle, `HttpFetch` as an implementation trait, and
`HttpFetcher` as an erased alias. The HTTP plane in operator composition is a
transport slot plus layer wrappers; the public vocabulary should say transport.

`opendal-core` also depends on reqwest today. The global reqwest client,
`impl HttpFetch for reqwest::Client`, and reqwest TLS features all live inside
core. Direct `opendal-core` users therefore inherit reqwest, hyper, and rustls
even when they provide their own HTTP stack. Reqwest version upgrades and TLS
feature changes should not be part of the core API maintenance surface.

# Guide-level explanation

Users that configure a custom HTTP implementation use one transport concept:

```rust
let transport = HttpTransporter::new(my_transport);
let op = Operator::new(builder)?.http_transport(transport);
```

A custom implementation implements `HttpTransport`:

```rust
pub trait HttpTransport: Send + Sync + Unpin + 'static {
    #[cfg(not(target_arch = "wasm32"))]
    fn fetch(
        &self,
        req: Request<Buffer>,
    ) -> impl Future<Output = Result<Response<HttpBody>>> + Send;

    #[cfg(target_arch = "wasm32")]
    fn fetch(
        &self,
        req: Request<Buffer>,
    ) -> impl Future<Output = Result<Response<HttpBody>>>;
}
```

`HttpTransporter` is the clone-cheap erased handle used by operators, layers,
and operation contexts:

```rust
#[derive(Clone)]
pub struct HttpTransporter {
    inner: Arc<dyn HttpTransportDyn>,
}

impl HttpTransporter {
    pub fn new(transport: impl HttpTransport) -> Self;
    pub fn install_default(transport: impl HttpTransport);
    pub async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>>;
    pub async fn send(&self, req: Request<Buffer>) -> Result<Response<Buffer>>;
}

impl Default for HttpTransporter {
    fn default() -> Self;
}
```

`HttpTransporter::default()` returns a lazy default handle. It does not require a
default transport to be installed during operator construction. If an HTTP
request is made without any installed default transport, the request returns
`ConfigInvalid`. Non-HTTP services can still be constructed and used without a
default HTTP transport.

The facade installs default global components through:

```rust
pub fn install_default() {
    install_default_operator_registry();

    #[cfg(feature = "http-transport-reqwest")]
    opendal_core::HttpTransporter::install_default(
        opendal_http_transport_reqwest::ReqwestTransport::default(),
    );
}
```

The existing public `init_default_registry()` API remains as a compatibility
entry. It only registers services and does not install the default HTTP
transport. New code that wants the facade default behavior should call
`opendal::install_default()`.

# Reference-level explanation

`core/core/src/types/mod.rs` gets a private source module and explicit public
exports:

```rust
mod http_transport;
pub use http_transport::HttpBody;
pub use http_transport::HttpTransport;
pub use http_transport::HttpTransporter;
```

The stable public paths are `opendal::HttpTransport`,
`opendal::HttpTransporter`, and `opendal::HttpBody`. There is no public
`opendal::http_transport` module.

`HttpTransportDyn` is the private object-safe adapter. It boxes the future
returned by `HttpTransport::fetch`, but users implement `HttpTransport` directly
and never name `HttpTransportDyn`.

The reqwest implementation lives in `opendal-http-transport-reqwest`:

```rust
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl HttpTransport for ReqwestTransport { ... }
impl Default for ReqwestTransport { ... }
impl From<reqwest::Client> for ReqwestTransport { ... }
```

`opendal-core` removes `reqwest`, `GLOBAL_REQWEST_CLIENT`,
`impl HttpFetch for reqwest::Client`, and the reqwest TLS features. The facade
keeps reqwest-related feature names if needed, but they forward to
`opendal-http-transport-reqwest`.

Operator composition keeps the same invariant: an operator is rebuilt from
`base service + base HTTP transport + base executor + layers`. Replacing the base
transport with `.http_transport(...)` must preserve already configured HTTP
wrappers because layers are replayed over the new base.

The naming changes are mechanical:

- `HttpClient` becomes `HttpTransporter`
- `HttpFetch` becomes `HttpTransport`
- `HttpFetchDyn` becomes private `HttpTransportDyn`
- `HttpFetcher` is removed as a public concept
- `Operator::http_client` becomes `Operator::http_transport`
- `OperationContext::http_client` becomes `OperationContext::http_transport`
- `Layer::apply_http_fetch` becomes `Layer::apply_http_transport`
- `HttpClientHttpSend` becomes internal `HttpTransporterHttpSend`

# Compatibility and migration

This is a breaking raw API cleanup. `HttpClient`, `HttpFetch`, `HttpFetchDyn`,
and `HttpFetcher` disappear from the public design. Existing code migrates from:

```rust
op.http_client(HttpClient::with(client))
```

to:

```rust
op.http_transport(HttpTransporter::new(transport))
```

Default `opendal` facade users keep the same out-of-box behavior under default
features because the facade installs `ReqwestTransport` through
`install_default()`. Direct `opendal-core` users must either install a default
transport with `HttpTransporter::install_default(...)` or configure an operator
with `.http_transport(...)`.

`HttpTransporter::install_default` is first-wins, returns no error, and does not
overwrite an already installed default. The facade installs its default before
`main` when `auto-register-services` is enabled. Users that need process-wide
replacement must disable `auto-register-services`, install their transport
themselves, and call `init_default_registry()` explicitly when URI-based
construction is needed. Otherwise, they should use operator-level
`.http_transport(...)`.

The operator-registry-only public entry `init_default_registry()` remains for
compatibility. It does not install HTTP transport. The facade
`auto-register-services` ctor calls `install_default()` so the default facade
behavior remains complete.

`reqwest-rustls-tls` and `reqwest-rustls-no-provider-tls` are removed from
`opendal-core` and forwarded from the facade to `opendal-http-transport-reqwest`.
A CI dependency check should assert that `opendal-core` does not depend on
reqwest under any feature combination.

# Drawbacks

This is a raw API breaking change for third-party service and layer authors.
The migration is mostly mechanical, but every HTTP-aware layer and every service
that calls `ctx.http_client()` must be touched.

Splitting reqwest into its own crate adds another workspace package and another
published artifact. Release tooling must publish the transport crate before the
facade package that depends on it.

The process-wide default transport remains global state. The design keeps it
minimal and first-wins, but users that need strict control must disable facade
auto-installation and own default initialization or configure transports per
operator.

# Rationale and alternatives

Keeping the name `HttpClient` would minimize the rename but preserve the wrong
concept. The value is not a concrete HTTP client like `reqwest::Client`; it is
the transport plane used by OpenDAL services and layers.

Keeping `reqwest` as an optional dependency of `opendal-core` would reduce the
number of crates, but it would still keep reqwest feature choices, TLS policy,
and security updates inside the core maintenance surface. A separate transport
crate makes the core contract independent from the default implementation.

Making `HttpTransporter` a type alias for `Arc<dyn HttpTransportDyn>` would be
smaller, but it would expose the erased representation and leave no place for
`Default`, `install_default`, `fetch`, `send`, opaque `Debug`, or future
transport-handle behavior. A newtype keeps the public contract narrow.

Adding a public `opendal::http_transport` module would expose the source-module
layout as a stable path. Existing OpenDAL public types are re-exported from the
crate root, so this RFC follows the same pattern.

# Prior art

OpenDAL already uses facade auto-registration for service construction:
`opendal` registers enabled services into `OperatorRegistry`, while
`opendal-core` provides the registry and construction contract. This RFC applies
the same split to the default HTTP transport: core owns the contract, and the
facade installs the default implementation.

The existing `Executor` type remains a newtype around its runtime implementation.
This RFC does not change executor behavior, but `HttpTransporter` follows the
same handle pattern instead of exposing an `Arc<dyn Trait>` alias directly.

# Future possibilities

Additional official HTTP transport crates can be added without changing
`opendal-core`, for example transports backed by another HTTP stack or by a
platform-native client.

The executor default installation model may later be aligned with
`HttpTransporter::install_default`, but that is outside the scope of this RFC.
