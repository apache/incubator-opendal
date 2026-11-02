- Proposal Name: `align_list_api`
- Start Date: 2023-10-07
- RFC PR: [apache/opendal#3232](https://github.com/apache/opendal/pull/3232)
- Tracking Issue: [apache/opendal#3236](https://github.com/apache/opendal/issues/3236)

# Summary

Refactor internal `Page` API to `List` API.

# Motivation

OpenDAL's `Lister` is implemented by `Page`:

```rust
#[async_trait]
pub trait Page: Send + Sync + 'static {
    /// Fetch a new page of [`Entry`]
    ///
    /// `Ok(None)` means all pages have been returned. Any following call
    /// to `next` will always get the same result.
    async fn next(&mut self) -> Result<Option<Vec<Entry>>>;
}
```

Each call to `next` will retrieve a page of `Entry` objects. This design is modeled after the `list_object` API used in object storage services. However, this design has several drawbacks:

- Services like `fs`, `hdfs` needs to buffer the whole page in memory before returning it to the caller.
- `Page` is not aligned with `opendal::Lister` make it hard to understand the code.
- `Page` is not aligned with `Read` & `Write` which is poll based.

# Guide-level explanation

No user-facing changes.

# Reference-level explanation

We will rename `Page` to `List` and change the API to:

```rust
pub trait List: Send + Sync + 'static {
    /// Fetch a new [`Entry`]
    ///
    /// `Ok(None)` means all entries have been returned. Any following call
    /// to `next` will always get the same result.
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Result<Option<Entry>>;
}
```

All `page` related code will be replaced by `list`.

# Drawbacks

Breaking changes for raw API.

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
