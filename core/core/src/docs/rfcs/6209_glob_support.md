- Proposal Name: `glob_support`
- Start Date: 2025-05-21
- RFC PR: [apache/opendal#6209](https://github.com/apache/opendal/pull/6209)
- Tracking Issue: [apache/opendal#6210](https://github.com/apache/opendal/issues/6210)

# Summary

Add support for matching file paths against Unix shell style patterns (glob) in OpenDAL.

# Motivation

Glob patterns are a widely used way to filter files based on their paths. They provide a simple and intuitive syntax for matching multiple files with similar path patterns. Adding glob support to OpenDAL would enable users to easily filter and process files that match certain patterns without having to implement this functionality themselves.

Currently, users who want to filter objects based on patterns have to list all objects and then apply filters manually, which is verbose and not very intuitive. By providing native glob support, we can make this common operation more convenient and efficient.

# Guide-level explanation

With glob support, users can easily match files based on patterns. The API would be available as an option on the `list_with` and `lister_with` methods, allowing users to filter entries that match the provided glob pattern.

For example:

```rust
// Get all jpeg files in the media directory and its subdirectories
let entries: Vec<Entry> = op.list_with("media/").glob("**/*.jpg").await?;

// Process entries
for entry in entries {
   do_something(&entry);
}

// Or use a lister for streaming access
let mut lister = op.lister_with("media/").glob("**/*.jpg").await?;

while let Some(entry) = lister.next().await? {
   do_something(&entry);
}
```

The glob syntax would support common patterns like:

- `*` - Match any sequence of non-separator characters
- `?` - Match any single non-separator character
- `**` - Match any sequence of characters including separators
- `{a,b}` - Match either a or b
- `[ab]` - Match either a or b
- `[a-z]` - Match any character in range a-z

The API would be integrated into the existing builder pattern.

# Reference-level explanation

The implementation would involve:

1. Implementing a pattern matching logic for glob expressions. This can be a simplified version focusing on common use cases like `*`, `?`, and `**`.

2. Modifying the `FunctionLister` and `FutureLister` to accept a glob pattern and filter entries accordingly.

The `GlobMatcher` struct would be an internal implementation detail that encapsulates the parsed glob pattern and the matching logic.

```rust
// This is an internal implementation detail, not exposed in the public API
struct GlobMatcher {
    // internal representation of the pattern
}

impl GlobMatcher {
    fn new(pattern: &str) -> Result<Self> {
        // Parse the pattern string
        // ...
    }

    fn matches(&self, path: &str) -> bool {
        // Perform the matching logic
        // ...
    }
}
```

The implementation would be built on top of the existing listing capabilities. Pattern matching will primarily occur client-side. However, for services with native glob/pattern support (e.g., GCS `matchGlob`, Redis `SCAN` with `MATCH`), OpenDAL will delegate the pattern matching to the service where possible to improve efficiency.

# Drawbacks

- While the API surface change is minimized by integrating with the existing builder pattern, it still introduces a new concept (glob patterns) for users to learn.
- Implementing server-side delegation adds complexity, as OpenDAL needs to identify services with native support and translate glob patterns to their specific syntax.
- For services without native glob support, client-side matching still requires listing all potentially relevant entries first, which might be inefficient for very large directories or complex patterns.
- Ensuring consistent behavior between client-side and various server-side implementations of glob matching can be challenging.

# Rationale and alternatives

This design integrates glob filtering into the existing builder pattern API, providing a natural extension to current functionality. We will implement our own pattern matching logic, focusing on commonly used glob syntax (e.g., `*`, `?`, `**`, `*.parquet`) to avoid the complexity of full-featured glob libraries designed for local file systems. This approach allows for a lean implementation tailored to object storage path matching.

Where services offer native pattern matching capabilities, OpenDAL will delegate to them. This leverages server-side efficiencies. For other services, client-side filtering will be applied after listing entries.

Alternatives considered:

1. Not implementing this feature and letting users implement filtering manually
   - This puts the burden on users and leads to repetitive code.
   - Users might implement inefficient or buggy filtering.

2. Relying entirely on an external glob library
    - Most glob libraries include complex logic for local file systems (e.g., directory traversal, symlink handling) which is not needed for OpenDAL's path matching.
    - This can introduce unnecessary dependencies and overhead.

3. Implementing server-side filtering *only* for services that support it, without a client-side fallback.
   - This would lead to inconsistent feature availability across services.
   - A client-side fallback ensures glob functionality is universally available.

4. Adding a more general filtering API instead of specifically glob patterns
   - While potentially more flexible, this would be more complex to design and implement.
   - Glob patterns are a well-understood and widely used standard for this type of path matching, covering the majority of use cases.

Not providing a unified glob capability means users continue to write verbose code for a common operation, or face inconsistencies if trying to leverage service-specific features directly. OpenDAL aims to provide a consistent and ergonomic interface for such common tasks.

# Prior art

Many file system and storage APIs provide glob or similar pattern matching capabilities:

- The [glob](https://crates.io/crates/glob) crate in Rust
- Python's [glob](https://docs.python.org/3/library/glob.html) module
- Node.js [glob](https://www.npmjs.com/package/glob) package
- Unix shells like bash with built-in glob support

Most implementations provide similar syntax, though there are some variations. We should align with established Rust patterns.

# Unresolved questions

None

# Future possibilities

- If services add native support for glob filtering, we could optimize by pushing the filtering to the server side
- We could extend the API to support more advanced pattern matching like regex
