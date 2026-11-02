- Proposal Name: `capability_api`
- Start Date: 2026-06-05
- RFC PR: [apache/opendal#7700](https://github.com/apache/opendal/pull/7700)
- Tracking Issue: [apache/opendal#0000](https://github.com/apache/opendal/issues/0000)

# Summary

Rename OpenDAL's capability APIs around one user-facing concept:
`Capability` describes what the current `Operator` can do. Internally,
`AccessorInfo` will still keep both the service-declared capability and the
effective operator capability so layers can simulate, complete, or override
behavior without changing the service declaration.

# Motivation

OpenDAL currently exposes `native_capability()` and `full_capability()` on
`OperatorInfo`. The split is technically useful, but it makes the public API
harder to use correctly:

- Users usually need to know whether an operation is available on the current
  `Operator`. That is the effective capability.
- Layers such as `SimulateLayer`, `CompleteLayer`, and
  `CapabilityOverrideLayer` need to know or update different capability states.
- The names `native` and `full` describe implementation history instead of the
  contract that callers should rely on.

The result is a confusing public model. A caller can read
`native_capability().delete_with_recursive == false` and conclude that
recursive delete is unavailable, even though `full_capability()` may be `true`
after simulation. OpenDAL should make the availability check obvious and keep
the service-declared state as an internal implementation boundary.

# Guide-level explanation

Users should use `OperatorInfo::capability()` to check whether a feature is
available:

```rust
let cap = op.info().capability();

if cap.delete_with_recursive {
    op.delete_with("path/").recursive(true).await?;
}
```

`Capability` remains the only public capability type. It describes the current
operator after services, layers, simulations, and overrides have been applied.

Service authors and layer authors should use the service capability only when
they need the service-declared baseline. For example, `SimulateLayer` checks the
service capability to decide whether it should synthesize a missing operation,
then updates the effective capability to describe the new behavior.

The old Rust methods remain as deprecated compatibility aliases during the
transition:

```diff
+ pub fn capability(&self) -> Capability
+ pub(crate) or raw fn service_capability(&self) -> Capability
- pub fn full_capability(&self) -> Capability
- pub fn native_capability(&self) -> Capability
```

New user-facing examples and binding APIs should prefer `capability()`.
Bindings that already expose `capability()` keep their current behavior.
Bindings that expose `FullCapability` and `NativeCapability` should add
`Capability` as the primary property and deprecate the old names.

# Reference-level explanation

`Capability` keeps its existing fields and representation. This RFC does not
introduce a new `ServiceCapability` type. The service-declared and effective
states have the same shape today, and adding a new wrapper type would not add a
new contract.

`AccessorInfoInner` will be renamed from:

```rust
struct AccessorInfoInner {
    native_capability: Capability,
    full_capability: Capability,
}
```

to:

```rust
struct AccessorInfoInner {
    service_capability: Capability,
    capability: Capability,
}
```

The fields have these meanings:

- `service_capability`: the capability declared by the service implementation.
- `capability`: the effective capability of the accessor after layers have been
  applied.

`AccessorInfo` will provide the following methods:

```rust
impl AccessorInfo {
    pub fn capability(&self) -> Capability;
    pub fn service_capability(&self) -> Capability;

    pub fn set_service_capability(&self, capability: Capability) -> &Self;
    pub fn update_capability(&self, f: impl FnOnce(Capability) -> Capability) -> &Self;
}
```

`set_service_capability` also resets `capability` to the same value. This keeps
the current invariant of `set_native_capability`: services declare the baseline
once, and layers derive the effective capability from that baseline.

The existing methods become deprecated aliases:

```rust
impl AccessorInfo {
    #[deprecated(note = "use service_capability()")]
    pub fn native_capability(&self) -> Capability;

    #[deprecated(note = "use set_service_capability()")]
    pub fn set_native_capability(&self, capability: Capability) -> &Self;

    #[deprecated(note = "use capability()")]
    pub fn full_capability(&self) -> Capability;

    #[deprecated(note = "use update_capability()")]
    pub fn update_full_capability(&self, f: impl FnOnce(Capability) -> Capability) -> &Self;
}
```

`OperatorInfo` will expose the effective capability:

```rust
impl OperatorInfo {
    pub fn capability(&self) -> Capability;

    #[deprecated(note = "use capability()")]
    pub fn full_capability(&self) -> Capability;

    #[deprecated(note = "service capability is not intended for availability checks")]
    pub fn native_capability(&self) -> Capability;
}
```

The stable user-facing API should not add `OperatorInfo::service_capability()`
in the first implementation. The service capability is an implementation detail
for service and layer authors. If a real application use case requires exposing
it later, it can be added as a separate API with clear documentation that it is
not an availability check.

Layer behavior will be updated as follows:

- `SimulateLayer` reads `service_capability()` and updates `capability()`.
- `CapabilityOverrideLayer` updates `capability()`.
- `CorrectnessCheckLayer` and `CapabilityCheckLayer` read `capability()`.
- Helper code that chooses part sizes, batch sizes, or writer behavior reads
  `capability()`.

Service implementations will call `set_service_capability()` instead of
`set_native_capability()`.

# Compatibility and migration

This change is source-compatible for Rust users during the transition because
the old APIs remain as deprecated aliases.

Rust migration:

```diff
- let cap = op.info().full_capability();
+ let cap = op.info().capability();
```

Service implementation migration:

```diff
- info.set_native_capability(Capability { ... });
+ info.set_service_capability(Capability { ... });
```

Layer migration:

```diff
- info.update_full_capability(|mut cap| {
+ info.update_capability(|mut cap| {
      cap.delete_with_recursive = true;
      cap
  });
```

Bindings should expose the effective capability as `capability` or
`Capability`. Existing `full_capability` and `native_capability` binding APIs
should be deprecated where the binding has a stable deprecation mechanism.

No data migration is required. Capability values are runtime metadata and are
not persisted by OpenDAL.

# Drawbacks

This change adds another naming migration to an already large API surface.
OpenDAL services and layers contain many direct calls to the old methods, so the
implementation will touch many files even though the behavior stays the same.

Keeping deprecated aliases also means the old names will continue to appear in
the codebase until the next breaking window.

# Rationale and alternatives

## Keep `native_capability` and `full_capability`

This keeps the current implementation simple, but it preserves the main user
confusion. The public API continues to present two capability sets even though
only one of them answers whether a feature is available.

## Add a new `ServiceCapability` type

`ServiceCapability` would make the internal distinction stronger at the type
level. However, it would have the same fields as `Capability` today. If it hides
those fields, service and layer code becomes harder to write. If it dereferences
to `Capability`, the type boundary becomes weak.

The proposed design keeps the single `Capability` data model and uses method
names to define the semantic boundary. A dedicated `ServiceCapability` type can
be introduced later if service-declared capabilities diverge from effective
capabilities.

## Expose `service_capability()` to users

Exposing service capability could help advanced users reason about performance
or implementation provenance. It also makes it easy to use the wrong API for
feature gating. This RFC keeps the stable public model focused on
`capability()`. Service capability remains available inside raw/service/layer
implementation code.

# Prior art

RFC-2852 introduced `native_capability` and `full_capability` to distinguish
service-native behavior from behavior implemented by layers. This RFC keeps the
same internal distinction but changes the public teaching model.

`CapabilityOverrideLayer` already treats the effective capability as the value
that should be changed by endpoint-specific overrides. `SimulateLayer` already
uses the service-declared capability to decide whether simulation is needed and
then updates the effective capability. This RFC gives those existing roles
stable names.

# Unresolved questions

- Should `OperatorInfo::native_capability()` be deprecated immediately in all
  bindings, or only in Rust first?
- Should raw `AccessorInfo::service_capability()` be public within `opendal-core`
  or restricted to crate-private APIs where possible?

# Future possibilities

OpenDAL may introduce a dedicated `ServiceCapability` type if service-declared
capabilities need fields that are not meaningful for effective operator
capabilities.

OpenDAL may also expose service capability through a diagnostic-only API if real
applications need to distinguish service-native behavior for cost or performance
planning.
