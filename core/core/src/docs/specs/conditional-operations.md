# Conditional operations

Conditional operations let callers observe or mutate storage only when a file
is in an expected state. A file is the object currently stored at an OpenDAL
path.

This specification defines the portable contract, and OpenDAL promises only
the behavior it explicitly defines. A service may support a subset of the
conditions through its advertised capabilities. Service-specific extensions do
not change the meanings defined here.

## Which file a condition checks

Each condition checks one file:

- `stat` and `read` check the file being observed.
- `write` and `delete` check the file at the operation path.
- `copy` and `rename` check the destination file.
- `restore` checks the file currently stored at the restore path. The selected
  version remains the source of the restore.

A version selector chooses the stored version an operation acts on. A version
condition compares the version of the file checked by the condition. They are
different concepts.

A copy or rename may also fail because its source does not exist, and a
restore may also fail because its selected version does not exist. When such an
error and a condition error occur together, this specification does not define
which error the operation returns.

## Capability contract

OpenDAL validates conditional options before dispatching an operation. If the
service does not advertise the required capability, the operation returns
`Unsupported`. OpenDAL never silently ignores a condition.

A service that advertises a conditional capability must implement the exact
contract for that condition. Separate capability fields do not imply support
for combinations of those conditions. If an implementation accepts a
combination, the file must satisfy every condition. If it cannot preserve the
complete conjunction, it returns `Unsupported` instead of dropping a
condition.

## Behavior when the file exists

The ETag conditions in this table use concrete ETag values. A wildcard ETag
has no portable meaning unless the documentation for a specific operation
defines one.

| Condition | Succeeds when |
| --- | --- |
| `if_match(etag)` | The file's ETag equals `etag`. |
| `if_none_match(etag)` | The file's ETag differs from `etag`. |
| `if_version_match(version)` | The file's version equals `version`. |
| `if_version_not_match(version)` | The file's version differs from `version`. |
| `if_not_exists()` | Never. |
| `if_modified_since(time)` | The file was modified after `time`. |
| `if_unmodified_since(time)` | The file was not modified after `time`. |
| `if_not_changed(metadata)` | The file has the identity represented by `metadata`. |

## Behavior when the file does not exist

`stat` and `read` return `NotFound` regardless of the condition.

For mutations:

| Condition | Result |
| --- | --- |
| `if_match(etag)` | `ConditionNotMatch`. |
| `if_version_match(version)` | `ConditionNotMatch`. |
| `if_not_changed(metadata)` | `ConditionNotMatch`. |
| `if_none_match(etag)` with a concrete ETag | The condition succeeds. |
| `if_not_exists()` | The condition succeeds. |
| `if_version_not_match(version)` | No portable behavior is defined. |

After a condition succeeds, the operation's normal semantics apply. Deleting a
file that does not exist remains a successful no-op. A write or copy may create
the file. Rename and restore may still fail because their source or selected
version does not exist.

## `if_not_changed` lowering

`if_not_changed(metadata)` is a positive identity condition derived from
metadata previously returned by OpenDAL. OpenDAL lowers it as follows:

1. Use a version match when the operation advertises version matching and the
   metadata contains a version.
2. Otherwise, use an ETag match when the metadata contains an ETag.
3. Otherwise, use a version match when the metadata contains a version.
4. Return `ConfigInvalid` when the metadata contains neither identity token.
5. Validate the derived primitive condition normally and return `Unsupported`
   when the operation does not support it.

An identical explicit equality condition is redundant. A conflicting equality
condition returns `ConditionNotMatch`. Other accepted conditions combine with
the derived condition using AND semantics.

Lowering happens while the mutable public options still own the source
metadata and after OpenDAL knows the composed service capability. The finalized
raw operation arguments contain only the selected primitive equality condition.

## Error contract

- A false explicit condition returns `ConditionNotMatch`.
- `stat` and `read` return `NotFound` when the file does not exist.
- A conflict reported by the service that is unrelated to an explicit OpenDAL
  condition returns `Conflict`.
- An unavailable condition or condition combination returns `Unsupported`.

Protocol status codes do not determine the error kind on their own. For
example, an HTTP `304 Not Modified` or `412 Precondition Failed` maps to
`ConditionNotMatch` only when it is the service's response to a condition
explicitly supplied by OpenDAL.

## Atomicity and visibility

A service advertises a mutation condition only when it can evaluate the
condition atomically with the mutation's visible commit. An implementation must
not emulate a conditional mutation by issuing `stat` followed by an
unconditional mutation.

For staged or multipart writes, the service evaluates the condition at the
final commit. If the condition fails, the previously visible file remains
unchanged.

The point at which an operation surfaces a conditional error is unspecified.
Depending on the operation and service, it may surface while constructing an
I/O handle, during I/O, or when closing the handle. The final result and
atomicity contract remain the same.

A conditional batch delete evaluates every entry independently. A service may
batch entries only when it preserves each entry's condition and error.
Detailed partial-success reporting follows the delete operation's separate
contract.

## Metadata contract

A service that advertises ETag or version match support must populate the
corresponding `Metadata` field whenever the native response provides it. The
identity token returned from a successful mutation or observation must be
usable as input to a later matching condition for the same file.

## Conformance

Capability-gated behavior tests under `core/tests/behavior` validate portable
conditions against real services. They cover only the behavior this
specification defines and do not impose cross-service expectations beyond it.
A service must pass the relevant portable behavior tests before advertising a
conditional capability.

## Historical decisions

- [RFC 5485: Conditional Reader](https://github.com/apache/opendal/blob/main/core/core/src/docs/rfcs/5485_conditional_reader.md)
- [RFC 7818: Rename If Not Exists](https://github.com/apache/opendal/blob/main/core/core/src/docs/rfcs/7818_rename_if_not_exists.md)
- [RFC 8145: Conflict Error Kind](https://github.com/apache/opendal/blob/main/core/core/src/docs/rfcs/8145_conflict_error_kind.md)
- [RFC 8147: Write If Not Changed](https://github.com/apache/opendal/blob/main/core/core/src/docs/rfcs/8147_write_if_not_changed.md)
