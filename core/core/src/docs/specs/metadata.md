# Metadata

`Metadata` is an owned, immutable description of a storage entry. It does not
borrow from a response, lister, reader, or operation, so callers may retain it
for any lifetime. Clones may share immutable storage internally. Consuming one
clone through `Metadata::into_builder` and building a modified value does not
change any other clone.

## Construction and access

Callers construct metadata with `MetadataBuilder::file(content_length)`,
`MetadataBuilder::dir()`, or `MetadataBuilder::unknown()`, then finish it with
`MetadataBuilder::build`. Existing metadata can be consumed with
`Metadata::into_builder`; `set_file(content_length)`, `set_dir()`, and
`set_unknown()` change its mode before rebuilding it.

User metadata is exposed through a borrowed `UserMetadata` view. The view
supports lookup, length checks, and iteration without requiring a `HashMap`
allocation. An absent user-metadata map and an explicitly supplied empty map
remain distinguishable.

Rust metadata and finalized operation arguments use compact blocks whose
encoded size, including indexes, must not exceed `u16::MAX` bytes. Construction
panics when a block exceeds this bound. Services and callers should enforce
their own smaller protocol limits before reaching this representation boundary.

## Content length

Every file `Metadata` contains the complete object length by construction.
`MetadataBuilder::file(0)` represents an empty file. Directory and unknown
metadata report zero.

Raw services and intermediate layers use `Unknown` while either the entry mode
or its full length is unavailable. They promote that value to `FILE` only at a
boundary that owns an authoritative length:

- stat and read-response parsers construct `FILE` only after obtaining the
  complete object length from the service response.
- list completion stats unknown entries, resolves their mode, and promotes
  files with the stat result's length.
- write completion uses the number of bytes written for replacements. Append
  adapters use the final object offset instead of the number of appended bytes.
- copy completion uses an authoritative service result or successful copy
  progress when the copy protocol provides one. Otherwise, it trusts a
  caller-asserted source length without reading source metadata, or reads source
  metadata before copying when no assertion is present. It does not issue a
  target `stat` and fails when none of these sources can provide an authoritative
  length.

Delete markers that do not carry an object length use a content length of zero.

## Operation boundary

Public operation options are mutable logical construction values. Before
OpenDAL freezes them into raw `Op*` arguments, it resolves logical conditions
such as `if_not_changed` against the composed service capability. Raw arguments
contain only the primitive conditions that a service must execute and do not
retain the source `Metadata`.

## Historical decisions

- [RFC 8194: Compact metadata and operation arguments](https://github.com/apache/opendal/blob/main/core/core/src/docs/rfcs/8194_compact_metadata_and_operation_args.md)
- [Tracking issue 8195](https://github.com/apache/opendal/issues/8195)
