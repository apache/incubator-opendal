- Proposal Name: `path_normalization_and_secure_hardening`
- Start Date: 2026-06-18
- RFC PR: [apache/opendal#7799](https://github.com/apache/opendal/pull/7799)
- Tracking Issue: [apache/opendal#0000](https://github.com/apache/opendal/issues)

# Summary

Align path normalization and add a utility for more secure enforcement for path
handling in `opendal-core`.

Together these changes make OpenDAL's path handling align with POSIX, URI, and
object store conventions: path normalization is strictly structural:

- separator collapsing
- leading-slash removal
- dot-segment removal
- local-filesystem root confinement for services that constructs OS paths.

# Motivation

OpenDAL is a storage access layer that sits between the host application and
many kinds of backends — local filesystems, cloud object stores, databases,
protocol services. Its path model needs to be a faithful abstraction: when a
user passes a path, OpenDAL should deliver that path to the backend without
silently rewriting it.

## The problem: whitespace trimming

`normalize_path` currently calls Rust's `.trim()` on the entire path string
before any other processing. `.trim()` removes all leading and trailing Unicode
whitespace — spaces, tabs, newlines, and exotic characters like em spaces.
This makes objects with whitespace in their names unreachable:

```rust
normalize_path("file ") // returns "file" — a different object
normalize_path(" dir/") // returns "dir/" — a different object
```

A file named `"file with trailing space "` on a local filesystem or an S3
object key `" report.csv"` cannot be accessed through OpenDAL.

## Alignment with filesystem and object store conventions

Every major filesystem and object store treats whitespace in path components as
significant content:

- **POSIX filesystems** allow any byte except `/` and `\0` in filenames.
  `"file"` and `"file "` are two distinct inodes. The POSIX Portable Filename
  Character Set is a portability recommendation, not a filesystem restriction.
- **S3, GCS, Azure Blob** allow arbitrary UTF-8 in object keys. Spaces are
  listed as "characters that might require special handling" but are explicitly
  valid. `"file"` and `"file "` are distinct objects.
- **Rust `std::path`**, Go `filepath.Clean`, Java `Path.normalize()`, and
  Python `pathlib` all perform structural normalization (collapsing separators,
  resolving `.`/`..`) without touching whitespace content.

The only system that strips trailing whitespace is the Win32 API, and that is a
legacy compatibility shim from the MS-DOS 8.3 filename era — the underlying
NTFS filesystem supports trailing spaces via the `\\?\` prefix. This is widely
regarded as a source of bugs, not a design to emulate.

OpenDAL's `.trim()` implementation goes beyond RFC-0112 specified — that RFC
defined two rules: collapse `//` into `/`, and strip leading `/`
to make paths relative to root.

## The problem: duplicated root confinement

A separate but related issue is how local filesystem services protect against
`..` traversal. The `fs` service has a `confined_join` function that rejects
paths containing `..` components before joining them onto the configured root.
The `compfs` and `monoiofs` services independently reimplemented the same logic
as `prepare_path`. This duplication is fragile — a new filesystem service
could easily forget to add the check.

Thus, we will add `confined_join` in `opendal-core`.

# Design

## Path normalization preserves content

After this change, `normalize_path` preserves all characters within path
components, including whitespace:

```rust
// Before (current behavior)
normalize_path("file ")    // => "file"   (WRONG: different object)
normalize_path(" file")    // => "file"   (WRONG: different object)
normalize_path(" a/b ")    // => "a/b"    (WRONG: components mangled)

// After (proposed behavior)
normalize_path("file ")    // => "file "  (correct: preserves name)
normalize_path(" file")    // => " file"  (correct: preserves name)
normalize_path(" a/b ")    // => " a/b "  (correct: preserves components)
```

The structural normalization rules are:

- Leading `/` is stripped: `/abc` → `abc`
- Internal `//` is collapsed: `abc///def` → `abc/def`
- `.` segments are removed: `./a/./b` → `a/b`, `./` → `/`
- Trailing `/` is preserved to distinguish directories: `abc/` → `abc/`
- Empty path becomes `/`: `` → `/`

Since `.trim()` removes all Unicode whitespace (tabs, newlines, em spaces,
etc.), this change preserves all of those characters too. This aligns with
POSIX filenames which can contain any byte except `/` and `\0`,
and object store keys are arbitrary UTF-8. If a backend genuinely cannot
handle certain characters, the service should return an error, **not** have
the core silently rewrite the path.

Users who relied on current behavior - `.trim()` on paths to clean up
sloppy input should sanitize paths in their application code before passing
them to OpenDAL:

```rust
// If your application needs trimmed paths, trim before calling OpenDAL:
let clean = user_input.trim();
op.read(clean).await?;
```

## Root confinement for local filesystem services

`confined_join` becomes a shared utility in `opendal_core::raw` for service
implementations that construct real OS paths. It is not part of the core
normalization API — it exists so that filesystem services do not need to
independently reimplement path confinement:

```rust
use opendal_core::raw::confined_join;

// In a filesystem service's Access implementation:
let abs_path = confined_join(&self.root, path)?;
```

This function rejects `..` traversal and joins the normalized path onto the
configured root using `PathBuf::join`. All local filesystem services (`fs`,
`compfs`, `monoiofs`) will use it instead of reimplementing path confinement.

# Detailed changes

## `normalize_path` change

Remove `.trim()` and filter `.` segments from `core/core/src/raw/path.rs`:

```rust
pub fn normalize_path(path: &str) -> String {
    // Before:
    // let path = path.trim().trim_start_matches('/');
    // After:
    let path = path.trim_start_matches('/');
    // ... rest unchanged except the filter adds `.` removal:
    let mut p = path
        .split('/')
        .filter(|v| !v.is_empty() && *v != ".")
        .collect::<Vec<&str>>()
        .join("/");
    // ...
}
```

We will also update doc comments to `normalize_path` and  `normalize_root`.

## `normalize_root` change

Add `.` segment filtering to the split/filter/join pipeline:

```rust
pub fn normalize_root(v: &str) -> String {
    let mut v = v
        .split('/')
        .filter(|v| !v.is_empty() && *v != ".")
        .collect::<Vec<&str>>()
        .join("/");
    // ...
}
```

This ensures paths like `./data/./root/` normalize to `/data/root/` rather
than preserving the `.` segments.

## `confined_join` extraction

Extract `confined_join` from `FsCore` into `opendal_core::raw::path` as a
free function. Replace the duplicate `prepare_path` implementations in
`compfs` and `monoiofs` with calls to this shared function.

```rust
/// Join a path onto a root directory, rejecting `..` traversal and
/// absolute path components.
///
/// This function is for service implementations that construct real OS paths
/// from OpenDAL paths — typically local filesystem services (`fs`, `compfs`,
/// `monoiofs`). It is NOT part of the core path normalization API.
///
/// ## Why `..` is rejected rather than resolved
///
/// Lexical `..` resolution (collapsing `a/../b` → `b`) is unsound in the
/// presence of symlinks. If `a` is a symlink to `/elsewhere`, then
/// `a/../b` on a real filesystem resolves to `/elsewhere/../b` — a
/// completely different location than `b`. Resolving `..` lexically would
/// silently produce the wrong path. Rejecting it is the only safe option
/// without hitting the filesystem.
///
/// ## Why not `canonicalize` here
///
/// `canonicalize` (realpath) resolves symlinks by making syscalls to the
/// filesystem. Calling it on every operation would be:
/// 1. A performance problem — every read/write/stat pays a realpath.
/// 2. A TOCTOU race — the path could change between canonicalize and use.
/// 3. Wrong for non-filesystem backends — `confined_join` is only called
///    by local-fs services, but the path has already been normalized by
///    the core layer which knows nothing about the local filesystem.
///
/// Service-level root canonicalization at init time is a separate concern
/// (see Future possibilities).
///
/// `normalize_path` strips leading `/` and collapses empty segments, but
/// intentionally does not resolve `.` or `..`. Since `PathBuf::join` is
/// purely lexical, a key like `../../etc/passwd` would escape the
/// configured root at syscall time. This function prevents that.
///
/// Use this function in any service that constructs real OS paths from
/// OpenDAL paths.
pub fn confined_join(root: &Path, path: &str) -> Result<PathBuf> {
    use std::path::Component;

    let trimmed = path.trim_end_matches('/');

    if trimmed.is_empty() {
        return Ok(root.to_path_buf());
    }

    let p = Path::new(trimmed);
    if p.components().any(|c| matches!(c,
        Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err(Error::new(
            ErrorKind::NotFound,
            "path escapes the configured root",
        )
        .with_context("path", path));
    }

    Ok(root.join(trimmed))
}
```

Key design choices:

- **Rejects `..` (ParentDir):** prevents traversal above the configured root.
- **Rejects `/` (RootDir) and drive prefixes (Prefix):** prevents absolute
  paths from replacing the root via `PathBuf::join`. The Operator's
  `normalize_path` strips leading `/`, so this is defense-in-depth for the
  function's own API contract.
- **Strips trailing `/`:** avoids creating OS paths that look like directories
  to the kernel when the OpenDAL path denotes a file. The trailing `/`
  distinction is an OpenDAL convention for dir-vs-file; the OS path should not
  carry it.
- **Empty path returns the root:** consistent with how services handle the root
  path `"/"` (which `normalize_path` represents as the empty-like `"/"`).

Each filesystem service keeps a thin `root_join` wrapper:

```rust
#[inline]
pub fn root_join(&self, path: &str) -> Result<PathBuf> {
    confined_join(&self.root, path)
}
```

## Rename functions

To make functions more readable, rename:

- `build_abs_path` -> `build_absolute_path`
- `build_rooted_abs_path` -> `build_rooted_absolute_path`
- `build_rel_path`: `build_relative_path`

## Usages in services

**Remote services** (S3, GCS, Azure Blob, databases, HTTP, WebDAV, FTP, etc.)
use `build_abs_path` to construct backend keys. Whitespace passes through as
literal content, matching the backend's own key semantics. No change needed.

**Local filesystem services** (`fs`, `compfs`, `monoiofs`) use `confined_join`
to construct OS paths. `PathBuf::join` treats whitespace as literal content,
matching POSIX semantics. No change to behavior — these services already
received whitespace-containing paths for *internal* components (e.g.,
`"a/ b /c"` was never trimmed by `.trim()`); only leading/trailing whitespace
on the full path string is new.

**Language bindings** (Python, Java, Node.js, etc.) pass paths through to the
Rust core without independent normalization. This change applies uniformly.

# Drawbacks

- **Behavior change for existing users.** Any user who relied on `normalize_path`
  to clean up sloppy whitespace will see different behavior. Paths like
  `"  report.csv  "` were previously mapped to `"report.csv"` and will now be
  sent verbatim to the backend.

- **Whitespace-only paths become valid.** A path like `"   "` currently maps to
  `"/"` (root). After this change it becomes the path `"   "` — a three-space
  filename. This is a valid POSIX filename but may be surprising.

Both drawbacks are mitigated by the fact that this aligns OpenDAL with every
other path API and storage system. The previous behavior was the surprising one.

# Rationale and alternatives

## Why this design

Removing `.trim()` is the minimal change that fixes the correctness bug. It
does not introduce new abstractions, configuration, or types. Extracting
`confined_join` eliminates code duplication and makes the security boundary for
local filesystem services explicit and centralized.

## Alternatives considered

### Per-service path validation

Let each service decide which characters are valid. This is correct in
principle but adds implementation burden across 50+ services, and the problem
at hand (whitespace is universally valid) does not require per-service logic.

### Typed path with validation pipeline

Introduce a `NormalizedPath` newtype and make validation composable. This is a
large API change across all services, layers, and bindings. The scope far
exceeds the bug being fixed.

### Prior art

Some research:

#### POSIX.1-2017 §4.13: Pathname Resolution

IEEE Std 1003.1-2017 defines pathname resolution by splitting on `/`, resolving
`.` and `..`, and traversing directory entries. Only `/` (separator) and `\0`
(terminator) have special meaning. All other bytes, including whitespace, are
literal filename content. The POSIX Portable Filename Character Set
(`A-Z a-z 0-9 . _ -`) is a portability recommendation, not a restriction.

Reference: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap04.html

#### RFC 3986: Uniform Resource Identifier (URI)

RFC 3986 §6.2.2 defines syntax-based normalization: remove dot segments,
normalize percent-encoding case, normalize scheme/host case. No whitespace
trimming. Spaces are percent-encoded (`%20`), and the decoded content is
preserved verbatim.

Reference: https://www.rfc-editor.org/rfc/rfc3986#section-6.2.2

#### Rust `std::path::Path`

`Path::new("file ").file_name()` returns `Some("file ")`.
`Path::new("/a/ b /c").components()` yields `Normal("a")`, `Normal(" b ")`,
`Normal("c")`. Whitespace is literal content. OpenDAL's normalization should
not be more aggressive than the language's own path handling.

Reference: https://doc.rust-lang.org/std/path/struct.Path.html

#### Go `filepath.Clean`

Applies four rules: collapse separators, eliminate `.`, eliminate inner `..`,
eliminate root `..`. Whitespace is preserved. `filepath.Clean("file ")` returns
`"file "`.

Reference: https://pkg.go.dev/path/filepath#Clean

#### Java `java.nio.file.Path.normalize()`

Eliminates redundant `.` and `..` name elements. No whitespace trimming.
`Paths.get("file ").normalize()` returns `"file "`.

Reference: https://docs.oracle.com/javase/8/docs/api/java/nio/file/Path.html#normalize--

#### AWS S3 Object Key Naming

Object keys are UTF-8, max 1024 bytes. Spaces are explicitly allowed. Leading
and trailing spaces are semantically significant — `"file"` and `"file "` are
distinct objects.

Reference: https://docs.aws.amazon.com/AmazonS3/latest/userguide/object-keys.html

#### Win32 Path Normalization (counter-example)

The Win32 API strips trailing spaces and periods — a legacy behavior from
MS-DOS 8.3 filenames. NTFS itself supports them via `\\?\`. Microsoft's docs
acknowledge this as a UI limitation. OpenDAL should not emulate it.

Reference: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file

# Future possibilities

- **`compfs` root canonicalization.** The `compfs` service does not currently
  canonicalize its configured root at init time. A future change could
  `canonicalize` the root once during `CompfsCore` construction so that the
  service operates on a resolved, absolute base path. This is safe at init
  time (no TOCTOU concern for the root itself) and would make debug output
  and error messages more predictable.

- **Remote object storage service root canonicalization.** For remote services
  (S3, GCS, Azure Blob, etc.) the configured root is a key prefix, not a
  filesystem path. A future RFC could standardize root normalization at
  `Operator` init time — for example, stripping trailing/leading slashes,
  collapsing separators, and validating that the prefix is well-formed for
  the target backend. This would be an init-time concern on the Operator
  builder, not a per-operation cost.

- Consider providing better explanation on:
  - `build_rooted_abs_path`
  - `confined_join`
  including:
  - how to use these functions
  - who should consider using these functions
