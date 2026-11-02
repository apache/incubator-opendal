- Proposal Name: `conditional_reader`
- Start Date: 2024-12-31
- RFC PR: [apache/opendal#5485](https://github.com/apache/opendal/pull/5485)
- Tracking Issue: [apache/opendal#5486](https://github.com/apache/opendal/issues/5486)

# Summary

Add `if_match`, `if_none_match`, `if_modified_since` and `if_unmodified_since` options to OpenDAL's `reader_with` API.

# Motivation

OpenDAL currently supports conditional `reader_with` operations based only on `version`. However, many storage services 
also support conditional operations based on Etag and/or modification time.

Adding these options will:

- Provide more granular control over read operations.
- Align OpenDAL with features provided by modern storage services, meeting broader use cases.

# Guide-level explanation

Four new options will be added to the `reader_with` API:

## `if_match`

Return the content only if its Etag matches the specified Etag; otherwise,
an error kind `ErrorKind::ConditionNotMatch` will be returned:

```rust
let reader = op.reader_with("path/to/file")
    .if_match(etag)
    .await?;
```

## `if_none_match`

Return the content only if its Etag does NOT match the specified Etag; otherwise,
an error kind `ErrorKind::ConditionNotMatch` will be returned:

```rust
let reader = op.reader_with("path/to/file")
    .if_none_match(etag)
    .await?;
```

## `if_modified_since`

Return the content if it has been modified since the specified time; otherwise, 
an error kind `ErrorKind::ConditionNotMatch` will be returned:

```rust
use chrono::{Duration, Utc};

let last_check = Utc::now() - Duration::seconds(3600); // 1 hour ago
let reader = op.reader_with("path/to/file")
    .if_modified_since(last_check)
    .await?;
```


## `if_unmodified_since` 

Return the content if it has NOT been modified since the specified time; otherwise, 
an error kind `ErrorKind::ConditionNotMatch` will be returned:

```rust
use chrono::{Duration, Utc};

let timestamp = Utc::now() - Duration::seconds(86400); // 24 hours ago
let reader = op.reader_with("path/to/file")
    .if_unmodified_since(timestamp)
    .await?;
```

All conditional reads resolve the current live target before evaluating the
condition. If no live target exists, the operation returns
`ErrorKind::NotFound`.


# Reference-level explanation

The main implementation will include:

1. Add new fields(`if_modified_since`, `if_unmodified_since`) and related functions to `OpRead`.

2. Add the related functions to `FutureReader`

3. Add new capability flags:
```rust
pub struct Capability {
    // ... other fields
    pub read_with_if_modified_since: bool,
    pub read_with_if_unmodified_since: bool,
}
```
4. implement `if_modified_since`, `if_unmodified_since` for the underlying storage service.

# Drawbacks

- Add complexity to the API

# Rationale and alternatives

- Follows existing OpenDAL patterns for conditional operations

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
