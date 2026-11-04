- Proposal Name: `ffi_plugin_abi`
- Start Date: 2026-08-25
- RFC PR: [apache/opendal#8207](https://github.com/apache/opendal/pull/8207)
- Tracking Issue: #0000

# Summary

Taking hints from DuckDB's core extension loading system, this RFC proposes a stable ABI that lets a service or a layer (together, a `plugin`) load at run time from a separate native library. All language bindings use this mechanism without a change to how they build an operator. The highlights are:

1. A host crate loads each plugin library, checks its ABI version, and registers it into the `OperatorRegistry`.
2. A plugin-backed operator is an ordinary `Operator`: it composes with other operators and with layers, and it moves payload bytes without a copy.

# Motivation

RFC 6828 split the Rust core into `opendal-core`, one `opendal-service-*` crate per backend, and one `opendal-layer-*` crate per layer. This split solved the Rust compile-time footprint. It did not solve the binding footprint.

Currently, each binding selects services and layers with `services-*` and `layers-*` features. Cargo links every selected service into one static artifact, resulting in one large shared library for each binding, often over 20 MB. A user who needs only one service or layer still gets all of them.

Rust has no stable ABI. A service built on its own therefore cannot load into a separately built core, because the trait object layout, the tokio types, and the shared core types do not match across separate builds. The pyo3 capsule technique hides this problem inside one Python process only, but this RFC attempts to propose a solution beyond just one binding.

OpenDAL ships on two release trains. `opendal-core`, `opendal-service-*`, and `opendal-layer-*` share one fast train and release together. The bindings run a slower train, so a binding may run an older core than the newest service. The two trains meet at the plugin boundary, which must stay stable across versions (see [Versioning](#versioning)).

This RFC has these goals:

- Let a user install one service as one package for one language binding.
- Let a user add one or more layers to that service at run time.
- Let services and layers from separate plugins compose in one process, exactly as compiled-in ones do. A program can, for example, copy an object from one plugin-backed service to another, or stack a plugin layer on a plugin service.
- Keep the data path free of extra copies and the hot path free of extra allocations, so a plugin-backed operator performs like a compiled-in one.
- Let a third party publish a service or a layer without a change to this repository.
- Do the work one time in the core host, not one time per binding.

# Guide-level explanation

## For a user of a binding

You install a thin binding package. The package contains the host, not the services. You then install each service that you need as a separate native package.

Example for Python:

```shell
pip install opendal
pip install opendal-plugin-s3
```

Your code does not change. You still build an operator from a scheme and a configuration map:

```python
op = opendal.Operator("s3", root="/data", bucket="example")
```

The host loads the `s3` plugin, registers it under the scheme `s3`, and builds the operator. Layers work in the same way.

## For a service or layer author

You write the service against the same `Service` trait as today. You then add a small C entry point from the new `opendal-abi` crate. You build the crate as a `cdylib`. You publish one native library for each target platform.

## For a binding maintainer

You add one function to the binding, for example `load_plugin(path)`. You do not change how the binding builds an operator. The registry does the rest.

# Reference-level explanation

## Design principle

Every binding builds an operator through one point: the `OperatorRegistry` and `Operator::via_iter` / `Operator::from_uri`. The input is a scheme string and a key-value map. The output is an `Operator` that holds a `Servicer` (`Arc<dyn ServiceDyn>`).

The plugin system connects to this one point. A plugin gives the registry a factory for its scheme. Every binding then reaches the plugin with no further per-binding code. This single seam is the reason the mechanism lives once in the core host instead of once per binding.

```text
binding                host                 plugin.so
  |  load_plugin(path)   |                       |
  |--------------------->| abi_version() ------->|
  |                      |<-- AbiVersion --------|   admit/refuse: Versioning
  |                      | register(registrar) ->|
  |                      |<-- AbiServiceDesc ----|   -> OperatorRegistry[scheme]
  |  Operator("s3", ...) |                       |
  |--------------------->| build via registry    |
```

## New crate: `opendal-abi`

`opendal-abi` holds the frozen contract. It contains only `#[repr(C)]` types and `extern "C"` function signatures. It does not depend on tokio or on service crates.

The crate defines these items at a high level:

- `AbiVersion`, a structure with a major number and a minor number.
- `AbiStr`, a pointer and a length for a UTF-8 string.
- `AbiBuf`, a pointer, a length, a capacity, and a free function for a byte buffer.
- `AbiError`, an error code and a message.
- `AbiConfigEntry`, a key and a value. An array of these entries carries the configuration map.
- `AbiCapability`, a flat mirror of `Capability`.
- `AbiServiceDesc`, a scheme, a capability, and an operation table.
- `AbiRegistrar`, an opaque host handle the plugin adds services to

Each plugin library exports two functions:

- `opendal_plugin_abi_version() -> AbiVersion`.
- `opendal_plugin_register(registrar: *mut AbiRegistrar)`.

The host calls the version function first and admits or refuses the plugin by the rules in [Versioning](#versioning). On admission it calls the register function, and the plugin adds one or more `AbiServiceDesc` values to the registrar.

The host cannot know a plugin's config schema, so it passes the configuration only as raw `AbiConfigEntry` string pairs. The plugin deserializes those pairs into its own `Configurator` and returns an `AbiError` for a missing or malformed key. This keeps `opendal-abi` free of per-service config types.

## Operation table

The operation table is a structure of `extern "C"` function pointers. It mirrors `ServiceDyn` exhaustively, one entry for every trait method: `info`, `capability`, `stat`, `read`, `write`, `list`, `delete`, `create_dir`, `copy`, `rename`, and `presign`. A compile-time check in `opendal-abi` fails the build when a `Service` method has no table entry, so the table cannot drift from the trait. Adding a method is an additive change that raises the minor version (see [Versioning](#versioning)).

Functions like `read`, `write`, and `list` return an opaque handle. Each handle has its own function table plus a `drop` function. The host wraps each handle in a Rust type that implements the matching `oio` trait.

Each operation receives its arguments as a `#[repr(C)]` structure. These structures mirror the `Op*` types. The ABI does not use the Rust `Op*` types, because those types are not stable across builds. The argument structures carry the full option surface, and that surface grows over time, so the versioning rules govern how the ABI adds new fields.

## Asynchronous streaming model

The `oio::Read`, `oio::Write`, and `oio::List` traits are asynchronous. A C ABI has no futures. The ABI bridges this gap with a poll model.

- Each asynchronous operation returns a future handle. The handle has a `poll` function and a `drop` function.
- The `poll` function has the shape `poll(handle, waker, out) -> {Ready, Pending}`. On each poll the host passes a waker that borrows the current task waker for the duration of the call and writes the result into the caller-owned `out` on `Ready`.
- A `Ready` poll clones and allocates nothing: the plugin fills `out` and returns. The plugin clones the waker into an owned form only when it returns `Pending`, so it can wake the task once the parked work makes progress. The host then polls again.
- The host wraps the future handle in a Rust `Future`, so the plugin drives both the asynchronous operator and the blocking operator through the same handle.

```text
host (Rust Future)                         plugin.so (future handle)
  |  poll(handle, &waker, out) ----------->|  work not done yet
  |                                         |  clone waker -> owned
  |<-- Pending -----------------------------|  park, return
  |  (task suspends)                        |
  |                                    ...  |  parked work makes progress
  |<-- wake() ------------------------------|  owned waker fires
  |  poll(handle, &waker, out) ----------->|  work done
  |                                         |  fill out, no clone, no alloc
  |<-- Ready -------------------------------|  return
  |  drop(handle) ------------------------->|  release future resources
```

The RFC selects the poll model over a blocking-thread model. In the blocking-thread model each plugin operation blocks and the host runs it with `spawn_blocking`, which has a smaller ABI surface but blocks one runtime worker for each operation in flight. It does not scale for a service with high concurrency, such as S3. The poll model keeps true asynchronous behavior and uses no extra threads, at the cost of more ABI surface.

The host owns the one tokio runtime. A plugin never starts its own runtime and never blocks inside a `poll` call, so the whole process shares that single runtime. A plugin that wraps a native asynchronous dependency bridges it to the poll model behind its own handle rather than start a second runtime.

## Data plane: no copy on the payload

The payload never crosses the boundary as a copy. A plugin produces its bytes in an allocation it owns and transfers ownership of that allocation to the host through `AbiBuf`; the host wraps the pointer as a `Buffer` and takes on the responsibility to free it later through the plugin's own free function. A write travels the same way in reverse: the host hands the plugin a borrowed slice for the duration of the call, and the plugin either consumes it in place or, for a buffered backend, appends it to its own allocation. No step memcpies the payload to move it across the boundary.

This matches how `opendal-core` already handles bytes internally. `Buffer` is a reference-counted, sliceable view over one or more `Bytes`, so a compiled-in read returns a slice of the backend's buffer without a copy. The ABI preserves that property across the boundary: a plugin-backed read is also a single owned allocation surfaced as a `Buffer`, so the data path of a plugin service and a compiled-in service are the same shape.

## Control plane: no allocation on the steady-state hot path

The poll contract (see [Asynchronous streaming model](#asynchronous-streaming-model)) sets the cost. A `Ready` poll allocates nothing: the plugin fills the caller-owned outcome struct and the borrowed waker builds no owned state. A `Pending` poll clones the waker once, so the plugin can wake the task later; this is the same cost profile as any async leaf in `opendal-core`, which also clones a waker only when a future actually parks. A backend that answers from a warm cache or a local file, and therefore rarely parks, pays no per-operation allocation on the boundary at all.

The fixed cost that remains is the indirect call through the operation table, which is one predictable, non-allocating branch per operation, on the order of nanoseconds. Against a backend where an operation costs microseconds for local disk or milliseconds for the network, this call is not measurable. A plugin-backed operator therefore performs like a compiled-in one on both the data path and the control path.

## Composition

A plugin-backed operator is an ordinary `Operator`. It holds a `Servicer`, the same erased service handle a compiled-in service produces, so everything that composes over `Servicer` composes over a plugin.

- Layers stack on a plugin service. A compiled-in layer wraps the plugin's `Servicer` through the existing `Layer::apply` path and does not know or care that the inner service came from a plugin. A layer that is itself a plugin stacks the same way through the layer ABI (see [Layers](#layers)); the composition seam is identical, only the wrapped service is erased on both sides.
- Services from separate plugins run in one process. Each loaded plugin registers its scheme into the shared `OperatorRegistry`, and each built operator owns its service handle and a reference to its originating library, so two operators from two different plugins are independent and coexist.
- Cross-service data movement inherits the data-plane rule (see [Data plane](#data-plane-no-copy-on-the-payload)). A read surfaces one owned `Buffer`, the destination write borrows it as a slice, so bytes move from a source plugin to a destination plugin with no boundary copy.

## Host crate: `opendal-plugin-host`

The host crate connects the ABI to the core. It contains these parts:

- `FfiService`, a type that implements `Service`. It calls the operation table. It wraps each handle in an `oio` type that drives the poll functions.
- `PluginLoader`, a type that loads a plugin library. It uses the `libloading` crate. It checks the ABI version. It calls `opendal_plugin_register`. It then registers each service into the `OperatorRegistry`.

## Changes to `opendal-core`

The mechanism builds an operator through the public construction path. It needs a small, contained set of additions to `opendal-core`.

- A constructor that builds an operator from an erased service. `Operator::new` takes a `Builder`, and a `Builder` requires a `Configurator` config type with serde bounds. A plugin-backed service has no such config. `opendal-core` already has `Operator::from_parts(OperationContext, Servicer)`, which takes an erased `Servicer` directly. The change is to confirm `from_parts` applies the same critical layers as `Operator::new`, or to add a thin wrapper that does, rather than a new constructor.
- A runtime scheme in `ServiceInfo`. `ServiceInfo::new` takes a `&'static str` scheme. A plugin scheme is known only at run time. `opendal-core` holds the scheme as an owned reference-counted string, `Arc<str>`, so a host does not leak a scheme string per operator.
- A registry factory that captures state. Today the registry stores a bare `fn(&OperatorUri) -> Result<Operator>` for each scheme. A plugin factory must capture the loaded library and the operation table, so the registry also accepts a boxed closure, `Box<dyn Fn(&OperatorUri) -> Result<Operator>>`. The implementation confirms that no code relies on the bare function pointer for a `const` context or for performance.

## Safety rules at the boundary

- Wrap every `extern "C"` boundary function, on both sides, in `catch_unwind`. A panic becomes an `AbiError`. A panic must not unwind across the C ABI.
- Define clear memory ownership. Each side frees the memory that it allocates. `AbiBuf` carries a free function, so the side that receives a buffer releases it through the allocator that produced it.
- Define the threading contract for each handle. The blocking operator drives futures on runtime worker threads, so a future handle moves between threads. The contract states which handles move between threads and which handles a caller may use concurrently.
- Define cancellation. The host drops a future handle when it drops the future, including before completion. A plugin releases every resource of a future on drop, whether the future completed or not.
- Validate every input from a plugin at the boundary. Treat a plugin as untrusted input.

## Versioning

The two release trains (see [Motivation](#motivation)) meet at the plugin boundary under two conditions, and each condition sets a different bar. The design serves both with two conformance modes. Only `opendal-abi` is shared at the boundary; the host needs `opendal-abi`, the loader, and the `Servicer`-to-`Operator` glue, not the full core. This scope lets the fast core train and the slow binding train move on their own cadence.

### Same-train mode: build-fingerprint match

`opendal-core` and a first-party service on the same train build together with one compiler in one run, so they match by construction, not by promise. For this common case the host uses a build fingerprint: a hash of the compiler version, the `opendal` version, and the ABI layout. The host loads a plugin whose fingerprint equals its own and refuses any other. This mode is the cheapest and the safest, and it is the default for first-party plugins, the common case.

### Cross-train mode: append-only ABI with load-time negotiation

Two cases cannot rely on a fingerprint match: a third-party plugin, and a plugin whose core version is newer than the core that the binding embeds. These cases use a stable ABI.

`AbiVersion` carries a major number and a minor number. A plugin pins the `opendal-abi` version that it builds against. An additive change, such as a new operation or a new option field on an argument structure, raises the minor number. A breaking change raises the major number.

The ABI grows by appending, never by reordering. A struct of function pointers grows only by adding fields at the end, and each field records the version that introduced it.

```text
operation table         v0 plugin reads   v1 host offers
  read      (v0)         [read]            [read]
  write     (v0)         [write]           [write]
  list      (v0)         [list]            [list]
  restore   (v1)          --               [restore]   host skips: not in v0
```

A plugin that targets an older minor version reads a prefix of the current table; the host does not call a function that the plugin does not provide. Field offsets of one major version stay stable for the whole life of that version, so a newer host loads an older plugin without a recompile.

The host and the plugin negotiate the surface at load time rather than trust a match. The plugin passes the version it built against, and the host returns the table for that version or refuses the load. A refusal is a clear, logged error, not a crash.

## Layers

A pluggable layer is part of this design from the start, not a later addition: a pluggable service you cannot add a layer to at run time would defeat the composition goal. The layer ABI lives in `opendal-abi` next to the service ABI and reuses the same operation table, handles, and versioning rules.

A layer plugin adds one direction the service ABI does not need. It exports an operation table, so the host or an outer layer calls it like a service, and it also calls into the operation table of the inner service it wraps. The host hands the layer the inner table when it applies the layer; the layer delegates each operation, adds its behavior, and returns through the same copy-free data plane and poll-based control plane. The project validates a layer plugin over a plugin service early (see [Delivery phases](#delivery-phases)).

# Binding integration

A binding needs only one new function to load a plugin, because it already builds an operator through the registry (see [Design principle](#design-principle)). The project adds bindings in this order, from the least to the most effort:

1. C. The C binding already enables no services by default. It is a natural host. It tests the ABI directly.
2. Go. The Go binding already uses run-time FFI through purego. The `opendal-go-services` packages already ship one library per service.
3. Python (pyo3) and Java (JNI). Each binding adds a thin `load_plugin` function and a package for each service.

# Distribution

- The binding package ships the host only. It contains the core and the loader. It does not contain services.
- Each service ships as a prebuilt native library for each platform. The library ships inside the native package of the ecosystem, for example a Python wheel, a Maven classifier JAR, or an npm optional dependency.
- The host loads a plugin from an explicit path first. A later version can scan a configured plugin directory. The scan uses an allowlist (see Security).

# Security

- Run-time load of native code runs that code in the host process. Treat this as a risk. Fail closed on any error.
- Do not search the current directory or the system library path for a plugin. Load a plugin only from an explicit path or from a configured, fixed plugin directory. This rule prevents an uncontrolled search path (CWE-427).
- Support an optional signature check or manifest check before the host loads a library.
- Log the actor, the action, and the outcome for each load. Record the path, the scheme, the ABI version, and the result. Do not log secrets.

# Drawbacks

- The cross-train ABI is a long-term commitment. The project must not break its major version without a migration path. First-party same-train plugins avoid this surface through the build-fingerprint match (see [Versioning](#versioning)).
- The asynchronous ABI is complex. It adds risk and maintenance cost. The design keeps the data path copy-free and the steady-state control path allocation-free (see [Data plane](#data-plane-no-copy-on-the-payload) and [Control plane](#control-plane-no-allocation-on-the-steady-state-hot-path)), but achieving this depends on getting the ownership and poll contracts exactly right.
- A plugin-backed operation crosses one indirect call through the operation table that a compiled-in operation does not. This call does not allocate and is not measurable against real backend latency, but it is not literally zero.
- A plugin that wraps a native asynchronous dependency must bridge it to the host poll model (see [Asynchronous streaming model](#asynchronous-streaming-model)) instead of running its own runtime. This bridge is extra work for the plugin author.
- The build and release process must produce a native library for each service and for each platform. This grows the CI matrix.

# Rationale and alternatives

The RFC selects a stable ABI with run-time loading. This is the only option that supports the full set of services, including services with native dependencies such as RocksDB, HDFS, and FoundationDB, and also supports true run-time mixing of services in one process.

## Implementation of the ABI

Because both sides are Rust, the baseline generates the stable ABI with maintained crates rather than hand-rolling C. `stabby` and `abi_stable` (also used by Apache DataFusion) produce stable-ABI trait objects with layout checks and a generated version check. `async_ffi` provides `FfiFuture`, which polls a future across an FFI boundary and matches the poll model this RFC selects. These crates remove much hand-written `unsafe` and keep every goal. The trade-off is a dependency on the crate and its health. A hand-written C ABI in `opendal-abi` is the fallback if a generated approach cannot meet a goal, and either choice leaves the rest of this design unchanged.

## Rejected alternatives

- Alternative A: static per-service packages. The project ships one prebuilt binding package for each service. Each package links the core and one service. This option has no ABI problem, and it keeps a zero-cost path for each service. It does not support run-time mixing of two services that ship as separate packages.

- Alternative B: an out-of-process sidecar. Each service runs as a separate process, and the host talks to it over a wire protocol such as gRPC, a local socket, or shared memory. This option removes the ABI problem entirely. It also adds serialization, a context switch, and a copy for each operation, so it fails the copy-free and allocation-free goals. A cross-service copy becomes a double hop through the host.

- Alternative C: build or link on demand at install time. The user selects the services, and a tool builds or statically links a thin artifact on the user machine. The result is fully static and zero-cost. It needs a build toolchain at install time, which is a heavy requirement for a `pip` or Maven user, and it cannot load an independently shipped third-party plugin at run time. This is a richer form of Alternative A. The RFC rejects it as the primary approach.

- Alternative D: WebAssembly components with WIT. WIT gives a generated, versioned, stable ABI. It also gives a sandbox. It does not support services with native dependencies today. The network support in WASI is not mature. This option fits a subset of services, such as pure HTTP services.

# Prior art

- The `opendal-go-services` packages already ship one native library per service for the Go binding.
- The `tower` and `tokio-util` crates split utilities from a core runtime.
- The Rust plugin ecosystem uses `libloading` and hand-written C ABIs for run-time load of native code.
- DuckDB ships a similar append-only, version-negotiated `cdylib` ABI in production, with two differences. Its struct is host-to-plugin: the host exposes its API and the plugin calls back. This RFC starts plugin-to-host and adds the host-to-plugin direction only with the layer ABI. DuckDB's data plane is also mostly synchronous with caller-owned buffers. A storage provider over a stable, asynchronous ABI is the new part, so the project validates the poll model (see [Asynchronous streaming model](#asynchronous-streaming-model)) first.

# Unresolved questions

- What is the bridge pattern for a plugin that wraps a native asynchronous library with its own event loop? The host-owns-the-runtime contract (see [Asynchronous streaming model](#asynchronous-streaming-model)) rules out a second runtime, so the plugin must drive that loop through the poll model. Phase 3 validates this pattern.

# Delivery phases

The project delivers the work in phases. Each phase has an exit gate.

- Phase 0. The community accepts the RFC. The project freezes `opendal-abi` version 0, covering the service ABI and the layer ABI.
- Phase 1. The project builds `opendal-abi`, the host, and the loader. The project ships the `fs` service as a `cdylib` plugin. The exit gate is a read, a write, a list, and a delete through the C binding.
- Phase 2. The project ships a layer (for example `timeout`) as a `cdylib` plugin and stacks it on the `fs` service plugin. The exit gate is the layer plugin observably wrapping the plugin service through the C binding.
- Phase 3. The project ships the `s3` plugin, which drives an HTTP backend through the asynchronous poll model, and stacks the `retry` layer plugin on it. The project tests the plugin under concurrency. The exit gate is correct streaming reads and writes for a high-concurrency service, a layer plugin wrapping an asynchronous plugin service, and a validated bridge pattern for a native asynchronous dependency under the host-owns-the-runtime contract.
- Phase 4. The project adds `load_plugin` and packaging to each binding, in the order C, Go, Python, and Java.
- Phase 5. The project builds each plugin for each platform in CI and publishes the packages.
