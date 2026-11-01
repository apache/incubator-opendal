- Proposal Name: `compact_metadata_and_operation_args`
- Start Date: 2026-08-29
- RFC PR: [apache/opendal#8194](https://github.com/apache/opendal/pull/8194)
- Tracking Issue: [apache/opendal#0000](https://github.com/apache/opendal/issues/0000)

# Summary

Represent `Metadata` and raw operation arguments with a private, copy-on-write
value block. `Metadata` keeps its required content length and optional timestamp
inline. Raw arguments place scalars according to their access and allocation
profile: `OpRead` packs its optional content-length hint, while `OpList` keeps
its limit inline. Optional strings, user metadata, listed paths, and raw
conditional timestamps use one immutable, exactly sized allocation per owner.
Each `Metadata` remains independently owned, and existing public `*Options`
remain the sole mutable operation-construction types.

On the measured 64-bit target, the prototype reduces `Metadata` from 264
to 40 bytes, `OpRead` from 256 to 16 bytes, `OpWrite` from 256 to 32 bytes, and
the other affected raw operations to 16-64 bytes. Clones share the block.
Empty metadata and metadata containing only inline scalars allocate nothing.

# Motivation

`Metadata` is returned by stat, read, and list operations and is often retained
in application caches, indexes, or work queues. It must remain independently
owned because a single listed entry can outlive its producing page. This makes
per-value size, allocation count, and clone cost part of the steady-state cost
of common OpenDAL workloads rather than temporary response-decoding overhead.

The current representation reserves one `Option<String>` slot for every known
string field even when most fields are absent. On the measured 64-bit target,
an empty `Metadata` occupies 264 bytes; 10,000 list results therefore reserve
about 2.5 MiB for metadata structs before paths, string payloads, collection
capacity, and allocator overhead. Each populated string allocates separately.
User metadata adds `HashMap` buckets and two independently allocated strings per
pair. Derived `Clone` repeats every populated string, bucket array, key, and
value, so cloning cost grows with the logical payload instead of remaining
constant.

Raw operation arguments have the same shape after their public options have
already been finalized. A measured `OpRead` occupies 256 bytes, owns each
condition or override string separately, and is retained by read contexts and
cloned when opening or splitting reads. Other raw operations repeat the same
pattern. Existing metadata mutation is concentrated in service response
assembly and normalization; downstream code primarily reads and retains the
completed value. Separating mutable construction from immutable results matches
this lifecycle. Compact immutable blocks make clone cost constant and reduce
resident memory and allocator traffic for large listings, concurrent reads,
retries, and downstream metadata retention.

Release-mode benchmarks ran on an Apple M4 Max with 128 GiB RAM using Rust
1.97.0. They compare the prototype types with the previous field-for-field
representations under the same allocator and benchmark process. Retaining
10,000 independently owned values produced these peak live-heap measurements.
Times are the median of three benchmark invocations and include constructing
the complete batch:

| Retained batch | Previous heap | Compact heap | Heap change | Previous time | Compact time |
| --- | ---: | ---: | ---: | ---: | ---: |
| List: length, ETag, version | 3.480 MB | 1.520 MB | -56.3% | 256.5 us | 597.5 us |
| List entries including paths | 3.980 MB | 1.760 MB | -55.8% | 528.5 us | 1.517 ms |
| Stat: seven strings | 4.540 MB | 2.720 MB | -40.1% | 712.0 us | 1.618 ms |
| Stat plus eight user pairs | 14.070 MB | 4.721 MB | -66.4% | 2.945 ms | 7.208 ms |

The retained allocation count falls from 20,001 to 10,003 for the list metadata
profile, 30,001 to 10,004 for list entries including their paths, 70,001 to
10,008 for stat, and 240,001 to 10,025 with user metadata. Packing is therefore
2.3-2.9 times slower during initial construction, while removing 40-66% of
retained heap and 50-96% of retained allocations.
An empty build changes from 13.17 ns to 24.64 ns. Cloning an empty value and
adding its first ETag changes from 35.36 ns to 53.38 ns because the compact form
creates its value block at that point.

Clone performance measures the lifecycle after construction:

| Operation | Previous median | Compact median | Change |
| --- | ---: | ---: | ---: |
| Clone list metadata | 39.12 ns | 2.13 ns | 18.4x faster |
| Clone stat metadata | 118.0 ns | 1.99 ns | 59.3x faster |
| Clone metadata with user pairs | 451.5 ns | 1.89 ns | 239.4x faster |
| Clone then replace stat ETag | 141.4 ns | 86.58 ns | 38.8% faster |
| Clone then replace user-metadata ETag | 484.6 ns | 83.97 ns | 82.7% faster |
| Read list version | 0.74 ns | 1.55 ns | 2.1x slower |
| Read user-metadata version | 0.73 ns | 1.52 ns | 2.1x slower |

Raw-operation measurements show the same exchange. Freezing populated
`ReadOptions` into `OpRead` changes from 62.34 ns to about 110 ns, while cloning
the compact result takes about 1.56 ns and no allocation instead of roughly 145
ns and eight allocations. For `OpWrite`, freezing changes from 186.9 ns to 437.8 ns,
while cloning changes from 480.0 ns and 25 allocations to 1.87 ns and no
allocation. Reading a packed read version takes 1.54 ns versus 1.63 ns from the
previous representation. Decoding a packed hint together with the version adds
about 0.8-1.4 ns compared with keeping the hint inline. Looking up one of eight
write user metadata entries takes 6.39 ns versus 10.82 ns. Direct final-block
encoding keeps lookup cheap and avoids temporary nested value blocks during the
mutable-to-immutable freeze.

The smaller owned condition also reduces public option layouts that embed
`Option<Metadata>`: `DeleteOptions` falls from 392 to 176 bytes (-55.1%),
`WriteOptions` from 536 to 320 bytes (-40.3%), and `CopyOptions` from 432 to 216
bytes (-50.0%).

Integer placement was measured separately because placing every integer in the
value block minimizes the carrier size but can introduce an allocation for a
value that previously needed none. The benchmark compares equivalent private
layouts and reports peak live heap while retaining 10,000 values:

| Metadata profile | Length and timestamp inline | Timestamp packed | All scalars packed |
| --- | ---: | ---: | ---: |
| Length only | 400 KB | 320 KB | 640 KB |
| Length and timestamp | 400 KB | 720 KB | 720 KB |
| Length, timestamp, ETag, version | 1.520 MB | 1.600 MB | 1.600 MB |
| Full stat | 2.720 MB | 2.800 MB | 2.800 MB |

The inline layout constructs those batches in 90.2 us, 97.3 us, 561.8 us, and
1.540 ms respectively. Packing the timestamp takes 159.3 us, 213.7 us, 596.1
us, and 1.645 ms; packing every scalar takes 206.5 us, 252.1 us, 601.9 us, and
1.678 ms. A scalar-only inline batch performs one allocation for the containing
`Vec`; either packed scalar layout performs 10,001 allocations when its packed
field is present. `Metadata` therefore keeps `content_length` and
`last_modified` inline. Their direct getters also avoid value-block decoding.

`OpRead::content_length_hint` has a different lifecycle. Packing it reduces
every `OpRead` from 32 to 16 bytes and reuses the value block whenever another
packed read option is present:

| Retained `OpRead` profile | Inline hint heap / time | Packed hint heap / time |
| --- | ---: | ---: |
| Empty | 320 KB / 127.5 us | 160 KB / 191.5 us |
| Hint only | 320 KB / 125.0 us | 560 KB / 243.6 us |
| Version only | 1.040 MB / 409.5 us | 880 KB / 401.6 us |
| Version and hint | 1.040 MB / 396.6 us | 960 KB / 422.1 us |
| All read options | 3.040 MB / 2.134 ms | 2.960 MB / 1.770 ms |

Single-value lowering remains 11.3 ns for an empty value and changes from
106.0 ns to 109.9 ns for all read options. The isolated hint profile changes
from 11.5 ns with no allocation to 22.5 ns with one allocation. This RFC accepts
that narrow case because the hint is optional, consumers read it during
operation setup rather than each streamed read, and the 16-byte reduction
applies to every retained or cloned `OpRead`.

Packing `OpList::limit` would reduce the carrier from 32 to 24 bytes, but a
limit-only batch would grow from 320 KB to 640 KB and slow from 26.58 us to
121.3 us. A batch containing both `start_after` and `limit` would grow from 800
KB to 880 KB and slow from 290.7 us to 317.6 us. `OpList` therefore keeps
`limit` inline.

# Guide-level explanation

`Metadata` remains an owned, immutable value with existing scalar and string
getters:

```rust
let metadata = op.stat("path/to/file").await?;
let size = metadata.content_length();
let etag = metadata.etag();
let cloned = metadata.clone();
```

The clone can outlive its producer. Callers construct metadata with
`MetadataBuilder` and consume existing metadata into a builder when they need a
modified value:

```rust
let mut builder = Metadata::builder(EntryMode::FILE);
builder.content_length(size).etag(etag);
let metadata = builder.build();

let mut builder = metadata.into_builder();
builder.content_length(actual_size);
let metadata = builder.build();
```

User metadata is exposed through a borrowed view:

```rust
if let Some(attributes) = metadata.user_metadata() {
    println!("{:?}", attributes.get("owner"));
}
```

The view supports lookup and iteration without materializing a map. Callers use
`IntoIterator` directly and collect into the collection required by their
boundary.

# Reference-level explanation

## Public API

`MetadataBuilder` is the only public construction and modification surface for
metadata. `Metadata` exposes no `new`, `Default`, `set_*`, or `with_*` API:

```rust
impl Metadata {
    pub fn builder(mode: EntryMode) -> MetadataBuilder;
    pub fn into_builder(self) -> MetadataBuilder;
    pub fn user_metadata(&self) -> Option<UserMetadata<'_>>;
}

pub struct MetadataBuilder {
    // Private mutable construction state.
}

impl MetadataBuilder {
    pub fn mode(&mut self, value: EntryMode) -> &mut Self;
    pub fn content_length(&mut self, value: u64) -> &mut Self;
    pub fn etag(&mut self, value: impl Into<String>) -> &mut Self;
    pub fn user_metadata(
        &mut self,
        value: impl IntoIterator<Item = (String, String)>,
    ) -> &mut Self;
    pub fn build(self) -> Metadata;
}

impl OpWrite {
    pub fn user_metadata(&self) -> Option<UserMetadata<'_>>;
}

#[derive(Clone, Copy)]
pub struct UserMetadata<'a> {
    // Private view into an immutable value block.
}

impl<'a> UserMetadata<'a> {
    pub fn get(&self, key: &str) -> Option<&'a str>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

The builder provides the same setter shape for every other logical metadata
field.

`UserMetadata` and `&UserMetadata` implement
`IntoIterator<Item = (&str, &str)>`. Callers can collect borrowed pairs
directly; boundaries that require ownership map the pairs to owned strings and
collect them. The builder accepts any owned string pairs and preserves the
distinction between an explicitly empty collection and an absent value.
Existing metadata getters, `Lister`, `Operator`, public operation options, and
raw operation getters retain their signatures and ownership semantics.

## Representation and ownership

The conceptual metadata representation is:

```rust
pub struct Metadata {
    header: MetadataHeader,
    values: CompactValues,
}

struct CompactValues(Option<Arc<[u8]>>);

struct MetadataHeader {
    content_length: u64,
    last_modified_seconds: i64,
    last_modified_nanoseconds: u32,
    flags: MetadataFlags,
}

#[repr(transparent)]
struct MetadataFlags(u16);
```

`MetadataFlags` encodes mode, scalar presence, deletion state, the three states
of `is_current`, and whether user metadata was supplied. Separate seconds and
nanoseconds preserve the range and precision of the existing `last_modified`
field.

On 64-bit targets, `MetadataHeader` is 24 bytes. `Arc<[u8]>` is a 16-byte fat
pointer, and `Option<Arc<[u8]>>` has the same size because `None` uses the null
pointer niche. `Metadata` is therefore 40 bytes. These are implementation
targets, not a stable ABI.

Each owner defines a closed internal `FieldId` set. A value block contains a
presence bitmap, a `u16` end offset for each present field, and payloads ordered
by `FieldId`; absent fields consume no offset slot. Known fields use bitmap and
offset arithmetic rather than hashing. User metadata follows them as sorted
key/value ranges and uses binary search, giving deterministic iteration and
order-independent equality.

Fields may contain UTF-8 strings or fixed-width scalar encodings. Private typed
helpers encode and decode scalars in little-endian byte order and require the
exact width for the field type. Integer type alone does not determine field
placement; each owner chooses whether a scalar remains inline or becomes a
typed value-block field.

String ranges enter the block only from Rust `str` or `String` values. Internal
decoding accepts blocks produced by the same encoders, so getters preserve this
UTF-8 invariant without validating the same bytes on every access.

Every encoded `CompactValues` block, including its bitmap, offsets, and entry
path when present, is limited to `u16::MAX` bytes. A builder panics before
encoding a larger block. There is no fallback representation.

For listed entries, the entry path occupies a private range in the same value
record. Retaining one entry can retain its own path but never its siblings.
Equality, debug output, bindings, and serialization use logical getters and do
not expose the byte representation as an ABI or wire format.

## Construction and transformation

A fresh `MetadataBuilder` stores the inline header and mutable values until
`build` encodes one exact block. `Metadata::into_builder` moves the existing
header and `CompactValues` into the builder without eagerly decoding or copying
the block. Header-only changes reuse the existing block. Packed-field changes
are recorded by the builder and encoded together once at `build`, leaving
clones of the original metadata unchanged. User metadata and nested conditional
metadata write directly into the final block instead of allocating intermediate
encoded buffers.

An `Entry` stores its path in the same block as its metadata.
`Entry::into_parts` retains its return type and materializes the returned path
`String`. `oio::Entry::metadata_mut` is removed; code that needs a modified
entry consumes its metadata into a builder and replaces the completed value.

## Raw operation arguments

Public `ReadOptions`, `StatOptions`, `WriteOptions`, `DeleteOptions`,
`CopyOptions`, `ListOptions`, and `RestoreOptions` remain ordinary owned,
mutable structs. Converting an options value into its raw `Op*` is a structural
freeze: every option is moved into the corresponding raw argument without
capability-dependent rewriting. The conversion does not accept or inspect
`Capability` and does not interpret conditional values.

For example:

```rust
pub struct OpRead {
    values: CompactValues,
}
```

`ReadField::ContentLengthHint` stores the hint as a fixed-width `u64` entry.
Existing optional fields use typed value entries; strings use UTF-8 and
fixed-width values use canonical little-endian encoding. A default `OpRead`
does not allocate. A hint-only value allocates one exact block; a hint combined
with another packed option shares that option's allocation. The measured
previous and proposed 64-bit sizes are:

| Type | Previous | Compact |
| --- | ---: | ---: |
| `OpRead` | 256 bytes | 16 bytes |
| `OpStat` | 240 bytes | 16 bytes |
| `OpWrite` | 256 bytes | 32 bytes |
| `OpDelete` | 128 bytes | 24 bytes |
| `OpCopy` | 168 bytes | 64 bytes |
| `OpList` | 48 bytes | 32 bytes |
| `OpRestore` | 32 bytes | 24 bytes |

`OpWrite` packs user metadata using the same sorted key/value representation as
`Metadata`. `OpReader`, `OpWriter`, and `OpCopy` retain their scalar tuning
fields, `OpList` retains its inline limit, and `OpRename` retains its one-byte
representation. The `OpCopy` measurement reflects the current single raw
argument that also owns copy concurrency, chunk size, and source length hint.

Raw `Op*` values expose getters plus `Clone`, `Default`, and `new` for empty
arguments, but no public `with_*` methods. Options-to-raw conversion encodes all
fields in one pass. `WriteOptions`, `DeleteOptions`, and `CopyOptions` preserve
the complete logical `if_not_changed` value in the corresponding raw arguments;
its fields are packed into the operation's value block rather than lowered to a
selected ETag or version condition. How a service evaluates the condition is
outside this representation RFC.

## Required content length

Every file `Metadata` returned by an `Operator` has an explicit content length
for the complete object. Directory metadata reports zero. A private presence
bit distinguishes an explicit zero from a missing value during assembly. A
completion layer may fill a missing value; publishing file metadata without one
returns `Unexpected`. The guarantee applies at the `Operator` response boundary.

# Compatibility and migration

`Metadata::user_metadata` and `OpWrite::user_metadata` break callers that name
or clone the returned `HashMap`; lookup and length checks retain the same shape,
while iteration uses `IntoIterator` instead of an `iter` method. Bindings
materialize their maps by collecting owned pairs at the boundary.

Metadata construction migrates from `Metadata::new`, `Default`, `set_*`, and
`with_*` to `Metadata::builder`; modifying an owned value uses
`Metadata::into_builder`. Raw callers migrate chained `Op*::with_*` construction
to the corresponding public `*Options` and structural conversion. Empty raw
arguments continue to use `new` or `Default`. Encoded value blocks larger than
`u16::MAX`, including codec overhead, now panic. Raw list batching, scheduling,
retry, serialization, and conditional-operation semantics remain unchanged.

# Drawbacks

Variable-field lookup performs bitmap and offset arithmetic instead of
dereferencing an `Option<String>`. A one-field transformation requires a
builder round trip, and removing direct mutation is a source-breaking API
change. Clone and drop perform an atomic reference-count operation when a block
exists. The private codec must uphold offset bounds and UTF-8 validity and is
less convenient to inspect in a debugger. A hint-only `OpRead` adds one
allocation and fixed-width decoding to save 16 bytes in every `OpRead`.

# Rationale and alternatives

Direct `Option<Box<str>>` fields remove string capacity but keep a pointer and
length for every absent field and retain one allocation per populated field. A
map adds bucket overhead and hashing for a closed field set. A page arena lets
one retained entry keep all siblings alive. A separate `Arc<Record>` and boxed
payload needs two allocations. The inline metadata header preserves direct
access to required or frequently read scalars and prevents scalar-only metadata
from allocating.

Scalar placement follows the measured lifecycle of its owner. The required
metadata content length and optional metadata timestamp stay inline because
packing either creates per-value allocations in scalar-only profiles.
`OpList::limit` stays inline because its standalone profile regresses both heap
and construction time. The setup-only, optional
`OpRead::content_length_hint` uses an existing typed value entry because halving
every `OpRead` outweighs the isolated hint-only allocation. This field-specific
rule retains a single compact codec without adding a second container or a
public scalar-storage abstraction.

Compacting the public `*Options` types would move block rebuilding into the
short-lived configuration path. Public options already express the mutable
construction state, so freezing them once at their existing conversion
boundary keeps mutation cheap without another builder or pending-options type.

# Prior art

Cloudflare's [DNS cache memory optimization][cloudflare-dns-cache] applies the
same principles to immutable Rust cache entries: remove unused capacity, pack
variable data into one exactly sized byte buffer, reuse scratch storage during
construction, and measure allocation and resident memory. OpenDAL keeps hot
scalars decoded and uses an independent entry allocation because downstream
users retain metadata outside list operations.

[cloudflare-dns-cache]: https://blog.cloudflare.com/dns-cache-memory-optimization-1111/

# Validation

The implementation must include layout tests for all sizes in this RFC on
supported 64-bit targets. Contract tests cover getters, equality, debug output,
timestamp range and precision, builder round trips, transformation after clone,
empty-versus-absent user metadata, field-for-field options conversion,
dispatch-time conditional lowering, and required file content length. Boundary
tests prove that a `u16::MAX`-byte block succeeds and the next byte panics before
encoding. Prototype benchmarks cover construction, retained heap, lookup, clone,
builder transformation, raw operations, and scalar placement. The implementation
must check in equivalent benchmarks with the optimized types.
