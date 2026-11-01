- Proposal Name: `write_if_not_changed`
- Start Date: 2026-08-25
- RFC PR: [apache/opendal#8147](https://github.com/apache/opendal/pull/8147)
- Tracking Issue: [apache/opendal#7889](https://github.com/apache/opendal/issues/7889)

# Summary

Add version match and non-match preconditions to OpenDAL's target-object
operations, and add `if_not_changed(&Metadata)` as the portable
optimistic-concurrency API. Before service dispatch, OpenDAL lowers
`if_not_changed` to one supported primitive condition. A service evaluates that
primitive atomically with the operation that changes the target.

ETags and versions remain separate identity axes implemented by services. S3
and Azure lower to ETag match, while GCS lowers to version match using the
object generation exposed as `Metadata::version()`. The metadata represents
caller-provided expected state and may come from OpenDAL or another source.

# Problem

OpenDAL exposes ETag conditions and historical version selectors, but it lacks
one portable read-modify-write condition. S3 guards replacement with the
current ETag, while GCS uses the current generation. Applications must
currently select a service-specific condition, and an unconditional mutation
after `stat` cannot prevent a concurrent update.

# Proposed design

## Public API

Add `if_version_match` and `if_version_not_match` to stat, read, write, delete,
and copy options. Add the missing `if_none_match` to delete and copy options.
Write, delete, and copy options also accept expected metadata:

```rust
pub if_version_match: Option<String>,
pub if_version_not_match: Option<String>,
pub if_not_changed: Option<Metadata>,

pub fn if_not_changed(self, metadata: &Metadata) -> Self;
```

Operator futures, reader and writer builders expose the corresponding fluent
methods. Blocking APIs accept the same options structs, and `DeleteInput`
accepts the same target conditions as `DeleteOptions`.

`if_not_changed` supports a read-modify-write flow without exposing the
service's identity choice:

```rust,ignore
let mut stream = op.reader(path).await?.into_bytes_stream(..).await?;
let expected = stream.metadata().await?;
let content = stream.try_collect::<Vec<_>>().await?;

op.write_with(path, modify(content))
    .if_not_changed(&expected)
    .await?;
```

Callers may also construct expected metadata from identities obtained through
another trusted channel:

```rust,ignore
let mut expected = Metadata::builder(EntryMode::Unknown);
expected.etag(saved_etag).version(saved_version);
let expected = expected.build();
```

Preserving both fields keeps externally stored metadata portable across
services. Both fields must describe the same observed object state.

## Condition semantics

All token values are opaque. A `version` option selects a historical object;
an `if_version_*` option compares the current live target without selecting a
historical version.

| Condition | Successful when |
| --- | --- |
| `if_match(e)` | A live target exists and its current ETag equals `e`. |
| `if_none_match(e)` | No live target exists or its current ETag differs from `e`. |
| `if_version_match(v)` | A live target exists and its current version equals `v`. |
| `if_version_not_match(v)` | No live target exists or its current version differs from `v`. |
| `if_not_exists` | No live target exists. |
| `if_not_changed(meta)` | One supported identity in `meta` still matches the target. |

Only concrete ETag values have portable delete semantics. Wildcard values such
as `"*"` are outside the portable delete contract. Multiple supplied conditions
are combined with logical AND.

`if_not_changed` is a derived condition. At the shared write, delete, or copy
entry point, OpenDAL first freezes the public options into raw arguments. It
then uses the effective capabilities after all layers and selects the primitive
condition before raw service dispatch:

1. If the operation supports `if_version_match` and `meta.version()` exists,
   use `if_version_match(version)`.
2. Otherwise, if the operation supports `if_match` and `meta.etag()` exists,
   use `if_match(etag)`.
3. If neither primitive is supported, return `Unsupported`.
4. If a primitive is supported but metadata contains no usable identity for
   any supported primitive, return `ConfigInvalid`.

Raw arguments preserve the complete owned `if_not_changed` metadata together
with the selected primitive. Services enforce the selected primitive; retaining
the logical condition lets future shared dispatch logic use another identity
without changing the public option or losing caller intent during structural
conversion. Version match takes precedence because a version is intended to
identify an object revision and avoids ETag ABA where the service can enforce
it. Adding version-match support may therefore make a call carrying both fields
stricter, but never weaker. Other metadata fields do not participate today.
OpenDAL does not validate provenance, path, or storage namespace, so the caller
must associate externally constructed metadata with the correct target.

OpenDAL preserves explicit target conditions and version selectors while
lowering `if_not_changed`. If the selected primitive match field already
contains the same opaque token, OpenDAL deduplicates it. If that field contains
a different token, OpenDAL returns `ConditionNotMatch` because both equality
conditions cannot hold. OpenDAL forwards every other representable combination
unchanged. After OpenDAL validates each condition's capability, the service
decides whether it accepts the combination and reports any invalid combination.

Missing-target results are part of the portable contract:

| Condition | Stat or read | Mutation where supported |
| --- | --- | --- |
| `if_match` | `NotFound` | `ConditionNotMatch` |
| `if_none_match` | `NotFound` | The condition succeeds. Delete is an idempotent no-op. |
| `if_version_match` | `NotFound` | `ConditionNotMatch` |
| `if_version_not_match` | `NotFound` | The condition succeeds. Delete is an idempotent no-op. |
| `if_not_exists` | N/A | The condition succeeds. |
| `if_not_changed` | N/A | `ConditionNotMatch`, inherited from the selected primitive. |

OpenDAL normalizes native condition failures, including HTTP 304 and 412, to
`ConditionNotMatch`. A successful condition only proves equality under the
selected identity. In particular, ETag-backed services do not provide total
ordering, fencing, or protection from ABA changes that return to the same
ETag.

## Atomicity and capabilities

A service advertises a mutation condition only when it can evaluate the
condition in the native operation that makes the mutation visible. It must not
simulate a mutation precondition with `stat` followed by an unconditional
operation. Staged and multipart writes evaluate the condition at the final
commit; a failure leaves the previously visible target unchanged. Copy
conditions guard the destination.

Conditional deletes may use a native batch only when it preserves each
condition and reports per-object condition failures. Otherwise, OpenDAL routes
conditional entries through a conforming single-object path. Existing
`Deleter` partial-success and retry semantics remain unchanged.

Capabilities follow the existing per-operation naming scheme:

- `{stat,read,write,delete,copy}_with_if_version_{match,not_match}`.
- `{delete,copy}_with_if_none_match`.

All new capabilities default to `false`. There is no independently advertised
`if_not_changed` capability; support is derived from the applicable version and
ETag match capabilities plus the supplied metadata.

Capabilities describe support for individual conditions. They do not guarantee
that a service accepts any particular combination of conditions or selectors.

A service advertising a version-match primitive must populate
`Metadata::version()` on stat, read, write, copy, and list whenever the native
response provides it. The same rule applies to `Metadata::etag()` for an
advertised ETag-match primitive. Every execution path reachable by a primitive
capability must preserve its condition and must not degrade to an unconditional
request.

## GCS mapping

GCS maps version conditions to JSON API generation parameters:

| OpenDAL | GCS JSON API |
| --- | --- |
| `if_version_match(v)` | `ifGenerationMatch=v` |
| `if_version_not_match(v)` | `ifGenerationNotMatch=v` |
| `if_not_exists` | `ifGenerationMatch=0` |
| `if_not_changed(meta)` | Lower to `if_version_match(meta.version())`. |

GCS applies these parameters to JSON API get, insert, delete, and destination
rewrite requests. Stat and read preserve missing-object responses as
`NotFound`. Mutations apply the portable condition predicate to missing targets
and map native precondition failures to `ConditionNotMatch`. Multi-request
rewrite keeps the destination condition across requests. Because GCS advertises
version match rather than ETag match for mutations, the shared lowering rule
selects the object generation for `if_not_changed`.

GCS must use a write path that can preserve generation conditions, such as
JSON resumable upload, before advertising a conditional write capability. It
must also populate `Metadata::version()` from generation values returned by
stat, read, write, copy, and list so OpenDAL-produced metadata can round-trip
through `if_not_changed`.

# Compatibility and migration

The new options and primitive capabilities are additive and default to
disabled. Version conditions on stat and read follow the existing ETag
condition contract for missing targets. Existing version selectors, ETag
conditions, and `if_not_exists` retain their behavior except for conditional
delete when its required live target is absent. Lowering `if_not_changed` in
core produces the same native conditions and errors as choosing the identity
separately in each service.

Conditional delete currently inherits unconditional delete's idempotent 404
handling on some services. After this RFC, a delete guarded by `if_match`,
`if_version_match`, or `if_not_changed` returns `ConditionNotMatch` when the
target disappears before commit. A delete guarded by `if_none_match` or
`if_version_not_match` succeeds as a no-op in the same case. This user-visible
correction applies to every binding that exposes the condition.

GCS currently advertises `write_with_if_not_exists` while one multipart path
cannot preserve the condition. The implementation must route that case through
a conforming API or disable the capability until it can honor the contract.

# Rationale and tradeoffs

A single `revision` field would duplicate GCS version and S3 ETag while
suggesting ordering, per-write uniqueness, or ABA resistance that ETags do not
provide. Keeping the axes explicit preserves their native meaning.

Applications could choose `if_match` or `if_version_match` after inspecting
capabilities, but that would expose service differences at every call site.
`if_not_changed` centralizes the choice as Operator-level policy and reuses
`Metadata`, which is already returned by object operations and can be
constructed by callers. Lowering it before raw dispatch keeps service contracts
orthogonal and gives custom services the API automatically when they implement
a primitive. Carrying the metadata costs more option space than carrying one
token, but avoids another public identity type and keeps the explicit
conditions available when callers need a specific axis.
