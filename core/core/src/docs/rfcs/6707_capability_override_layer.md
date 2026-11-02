- Proposal Name: capability_override_layer
- Start Date: 2025-10-20
- RFC PR: [apache/opendal#6707](https://github.com/apache/opendal/pull/6707)
- Tracking Issue: [apache/opendal#6708](https://github.com/apache/opendal/issues/6708)

# Summary

Introduce a tiny `CapabilityOverrideLayer` that lets integrators adjust an accessor's *full* capability by supplying a closure that mutates the existing [`Capability`](../../types/capability.rs) instance. The layer supplements the default pipeline and replaces the service-specific configuration toggles such as `delete_max_size`, `disable_stat_with_override`, or `write_with_if_match`. We will deprecate those fields and direct users (and our own behavior harness) to the new layer instead.

# Motivation

Today we scatter capability tweaks across several service configs. S3 alone exposes `delete_max_size`, `disable_stat_with_override`, and `disable_write_with_if_match` on `S3Config` (see `core/src/services/s3/config.rs:184-208`). OSS and Azblob mirror the same knobs in their configs (`core/src/services/oss/config.rs:76-78`, `core/src/services/azblob/config.rs:78`). These options exist only to downgrade capabilities when a compatible service lacks full S3 semantics. They dilute the purpose of configuration, create redundancy between services, and complicate documentation.

Our behavior tests already bypass those configs and mutate the capability directly via `info().update_full_capability(...)` to emulate lower limits (`core/tests/behavior/async_delete.rs:288-329`). That ad-hoc pattern hints that the real need is an explicit hook to patch capabilities after construction, not extra config fields.

We want a single, explicit, and reusable spot for such adjustments that stays outside service implementations and keeps native capabilities untouched.

# Guide-level explanation

With this RFC, builders stay focused on connection details while users layer capability overrides explicitly:

```rust
use opendal::layers::CapabilityOverrideLayer;
use opendal::services::S3;

let op = Operator::new(S3::default().bucket("demo"))?
    .layer(CapabilityOverrideLayer::new(|mut cap| {
        // Cloudflare R2 rejects large batch deletes and stat overrides
        cap.delete_max_size = Some(700);
        cap.stat_with_override_cache_control = false;
        cap.stat_with_override_content_disposition = false;
        cap.stat_with_override_content_type = false;
        cap.write_with_if_match = false;
        cap
    }))
    .finish();
```

The closure receives the current `Capability` (already filled by the backend) and must return the adjusted value. Only the accessor's *full* capability is patched; native capability remains intact so completion layers can still infer what the backend supports natively (`core/src/layers/complete.rs:23-62`). Downstream layers—`CorrectnessCheckLayer`, retry, delete helpers—observe the overridden full capability (`core/src/layers/correctness_check.rs:23-115`), so they continue to guard unsupported options.

Behavior tests and diagnostic tools can read environment variables and pass a closure to the same layer instead of mutating capability structs manually.

# Reference-level explanation

## Layer structure

The implementation introduces:

```rust
pub struct CapabilityOverrideLayer {
    apply: Arc<dyn Fn(Capability) -> Capability + Send + Sync>,
}
```

- `CapabilityOverrideLayer::new` accepts any closure that maps the current capability to the desired full capability.
- `Layer::layer` clones the closure, reads `info.full_capability()`, applies the function, and writes the result back via `info.update_full_capability`. Native capability is untouched.
- The layer returns the original accessor unwrapped, so it composes with existing layers transparently.

## Layer placement

`Operator::new` currently installs `ErrorContextLayer → CompleteLayer → CorrectnessCheckLayer`. We insert `CapabilityOverrideLayer` between error context and completion:

```
ErrorContextLayer
→ CapabilityOverrideLayer
→ CompleteLayer
→ CorrectnessCheckLayer
```

Positioning before `CompleteLayer` let us patch the full capability before completion synthesizes derived operations (e.g., auto-creating directories). Completion still reads the native capability to decide what to fill in, so we do not break its invariants.

## API exposure

- Rust: expose `CapabilityOverrideLayer` within `opendal::layers`.
- Other language bindings wrap the same primitive with a thin helper that accepts a closure-equivalent (e.g., a lambda or struct of booleans) and forwards to the Rust layer through FFI.
- No new `OperatorBuilder` method is required; users call `.layer(...)` explicitly.

## Deprecations and migration

The following config fields become deprecated aliases that internally forward to `CapabilityOverrideLayer` until they are removed:

- `S3Config::{batch_max_operations, delete_max_size, disable_stat_with_override, disable_write_with_if_match}` (`core/src/services/s3/config.rs:184-208`)
- `OssConfig::{batch_max_operations, delete_max_size}` (`core/src/services/oss/config.rs:76-78`)
- `AzblobConfig::batch_max_operations` (`core/src/services/azblob/config.rs:78`)

Migration path:

1. Emit a warning when these fields are set, pointing to the new layer-based approach.
2. In a compatibility shim, convert the old configuration into a `CapabilityOverrideLayer` injected automatically (only while the fields exist).
3. Behavior tests stop calling `info().update_full_capability` manually and instead always register the layer, keeping env-driven overrides centralised.
4. Remove the fields in a later release once downstream bindings migrate.

# Drawbacks

- Users must write code (a closure) rather than toggling a config string. For trivial overrides this is more verbose, especially in non-Rust bindings.
- The approach assumes callers are comfortable with the capability semantics; there is no compile-time guarantee that they only *downgrade* capabilities.

# Rationale and alternatives

Alternatives considered:

1. **Retain per-service config toggles**. This keeps the status quo but perpetuates redundancy and drifts farther from capability-centric design (`core/src/docs/rfcs/0409_accessor_capabilities.md`, `core/src/docs/rfcs/2852_native_capability.md`).
2. **Parse override values during config deserialization.** That hides the layer but still demands new schema for each field and duplicates logic across services.
3. **Automatic detection at runtime.** Allowing writes to fail before downgrading capabilities complicates error handling and caching, and is harder to reason about.

The chosen design centralises capability edits, keeps services agnostic, and matches the lightweight infrastructure we already use inside tests.

Not acting would leave us with an ever-growing list of service-specific toggles and scattered override logic, making future capability changes harder.

# Prior art

- Our own RFCs on capabilities already advocate for a layered approach (`core/src/docs/rfcs/0409_accessor_capabilities.md`, `core/src/docs/rfcs/2852_native_capability.md`). This proposal follows that direction by giving layers first-class control.
- The behavior test harness demonstrates manual capability mutation (`core/tests/behavior/async_delete.rs:288-329`), validating that adjusting `full_capability` is viable today; we generalise it into a formal layer.

# Unresolved questions

- How should non-Rust bindings expose the closure-based API ergonomically? Do we offer predefined helpers for common toggles?
- What is the precise deprecation schedule for each config field, and how noisy should the warnings be?
- Do we need guardrails to prevent capability *upgrades* that claim support the backend truly lacks?

# Future possibilities

- Provide convenience constructors such as `CapabilityOverrideLayer::disable_stat_overrides()` or `::limit_delete(max)` to ease usage from other languages.
- Offer an environment-driven helper (`CapabilityOverrideLayer::from_env`) for CLI tools and tests.
- Build curated profiles for popular S3-compatible services (Cloudflare R2, Wasabi, etc.) once the foundational layer lands.
- Investigate runtime detection or telemetry that suggests required overrides to users instead of manual discovery.
