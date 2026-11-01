- Proposal Name: `limited_read`
- Start Date: 2026-07-24
- RFC PR: [apache/opendal#7945](https://github.com/apache/opendal/pull/7945)
- Tracking Issue: [apache/opendal#7938](https://github.com/apache/opendal/issues/7938)

# Summary

Add `limit` to `Operator::read_with` and `ReadOptions`.

`range` selects the bytes to read. `limit` returns at most the first `limit`
selected bytes and accepts EOF after a valid starting byte. Bounded ranges
without `limit` remain exact, and a forward selection with a non-zero limit that
starts at or beyond EOF remains `RangeNotSatisfied`.

For an absolute start, Core lowers the cap into the existing service
`BytesRange`. For a suffix, Core preserves the suffix request and stops
collecting at the cap. A private `exact` boolean controls completion validation;
the raw read API and service request types remain unchanged.

# Motivation

Callers often need only an object's signature, header, or other leading metadata
to detect its format.

Today a caller can use `range(0..N)`, but a bounded range is exact. When its
starting byte exists but its end crosses EOF, the read fails even if the
available bytes are sufficient. Calling `stat` first avoids that error but adds
a request and introduces a race between metadata lookup and data access.

# Guide-level explanation

Use `limit` when fewer bytes than the cap are useful:

```rust
let header = op
    .read_with("path/to/file")
    .limit(16 * 1024)
    .await?;
```

If the object contains at least 16 KiB, this returns 16 KiB. If its non-empty
content is shorter, this returns the whole object. For this forward read, a
non-zero limit on an empty object returns `RangeNotSatisfied`. OpenDAL does not
perform a `stat` before reading.

`limit` can start at an offset:

```rust
let data = op
    .read_with("path/to/file")
    .range(4096..)
    .limit(1024)
    .await?;
```

This returns at most 1024 bytes from offset 4096. A valid starting byte followed
by EOF is successful; an offset at or beyond EOF returns `RangeNotSatisfied`.

`limit` also applies after a suffix range:

```rust
let data = op
    .read_with("path/to/file")
    .range(BytesRange::suffix(1024))
    .limit(512)
    .await?;
```

This selects the last 1024 bytes, or the whole object when it is shorter, then
returns up to the first 512 selected bytes.

`range` without `limit` still requires the complete bounded range. Missing
objects, failed conditions, permission failures, and transport errors also
remain errors.

# Reference-level explanation

## Public API

`ReadOptions` gains one field:

```rust,ignore
pub struct ReadOptions {
    pub range: BytesRange,
    pub limit: Option<u64>,
    // Existing fields.
}
```

`FutureRead` gains the matching builder:

```rust,ignore
pub fn limit(mut self, limit: u64) -> Self {
    self.args.limit = Some(limit);
    self
}
```

The order is fixed: `range` selects bytes, then `limit` caps the prefix returned
from that selection. `limit` does not move the selection's starting position.
Core pushes the cap into the service range when the selection has an absolute
start:

| Options | Service range | Completion |
| --- | --- | --- |
| no `range`, `limit(n)` | offset 0, size `n` | at most `n` |
| `range(offset..)`, `limit(n)` | offset `offset`, size `n` | at most `n` |
| `range(start..end)`, `limit(n)` | offset `start`, size `min(end - start, n)` | at most that size |
| `suffix(s)`, `limit(n)` | suffix `s` | at most the first `n` selected bytes |
| bounded `range` without `limit` | unchanged | exact |

`limit(0)` returns an empty buffer without checking the object, consistent with
an empty range. A non-zero forward limit on an empty object is
`RangeNotSatisfied` because its first byte does not exist.

Combining `limit` with an explicit `chunk` size or a `concurrent` value greater
than one returns `ErrorKind::ConfigInvalid` before storage I/O.

## Request and completion ownership

Core carries a private boolean named `exact`: it is `true` for a regular bounded
non-suffix range and `false` for a limited, open-ended, or suffix range. The
buffer stream tracks emitted bytes, slices the final buffer at the limit, and
stops at the cap. At EOF, it checks the bounded range size only when `exact` is
`true`.

Services receive the existing operation choice and `BytesRange`: `open` for a
stream or `read` for exact bounded materialization. For an absolute start, that
range already contains the limit, so the service has all information needed to
bound its I/O. The `exact` flag stays in Core because it changes only whether
Core accepts clean EOF before the requested size.

A suffix has no absolute start before the object length is known. `BytesRange`
cannot express both the suffix and a cap on its prefix, so Core sends the suffix
and caps the collected stream without `stat`. A service may transfer more than
the limit; exact suffix-limit pushdown would require service-facing state.

## Raw read contract

This proposal keeps the raw `oio::Read` API unchanged:

```rust,ignore
pub trait Read {
    fn open(&self, range: BytesRange) -> ...;
    fn read(&self, range: BytesRange) -> ...;
}
```

The two existing methods already provide the required split:

- `open(range)` returns the bytes available inside a satisfiable range and never
  crosses its boundary. For a non-empty bounded forward range, EOF after at
  least one requested byte is clean stream completion; an offset at or beyond
  EOF is `RangeNotSatisfied`.
- `read(range)` remains an exact bounded read for chunked and concurrent
  planning. It returns the complete range or an error.

`PositionReadStream` returns `RangeNotSatisfied` if its first read for a
non-empty bounded range returns no bytes, but treats a later empty read as clean
completion. `PositionReader::read` keeps rejecting any EOF before its exact
bounded read completes. Stream-based services follow the same rule.

`CompleteLayer::read` continues to require the exact bounded size.
`CompleteLayer::open` rejects bytes beyond the requested range and, when
`RpRead` contains the full object length, requires exactly the bytes available
in a satisfiable range. Without that metadata, it relies on the service to
distinguish clean EOF from a truncated response.

HTTP services still validate the response body's `Content-Length`, so accepting
object EOF does not turn a truncated network response into success.

## Execution

A limited read opens one stream and collects until the limit or EOF; it does not
issue speculative exact chunks across EOF. An explicit `chunk` size or
`concurrent` value greater than one is `ConfigInvalid`. No capability is needed
because every readable service supports `open(range)`.

`presign_read_options` also rejects `limit` because OpenDAL cannot apply its
completion check to a response executed by the caller.

## Compatibility and validation

Existing reads omit `limit` and retain their behavior. Callers that construct
`ReadOptions` without `..Default::default()` must initialize the new field.

Tests must cover empty objects and forward and suffix ranges around the limit
and EOF, including `RangeNotSatisfied` at or beyond EOF, unchanged exact-range
and non-EOF errors, invalid chunked or concurrent combinations, and truncated
HTTP bodies. Both stream-based and positioned-read services need coverage.

# Drawbacks

`range` and `limit` are similar size controls with different EOF semantics.
Suffix ranges may transfer more data than OpenDAL returns, and the initial
implementation does not support parallel chunk planning.

# Rationale and alternatives

- A separate `read_up_to` operation would duplicate the read options and builder
  surface; `limit` composes with them.
- Making every bounded range accept EOF would weaken file validation and safe
  concurrent chunk planning.
- Calling `stat` first adds latency and cannot make the read atomic with its
  metadata.
- A new raw method or planning type is unnecessary for absolute ranges because
  `open`, `read`, and the lowered `BytesRange` express the storage request. A
  private `exact` boolean expresses the remaining Core decision. Service-facing
  state solely for suffix-limit pushdown would expand every service's contract
  for an optional optimization.
