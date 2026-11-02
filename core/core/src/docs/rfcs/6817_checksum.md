- Proposal Name: checksum
- Start Date: 2025-11-24
- RFC PR: [apache/opendal#6817](https://github.com/apache/opendal/pull/6817)
- Tracking Issue: [apache/opendal#5549](https://github.com/apache/opendal/issues/5549)

# Summary

Add a single full-file checksum abstraction (`Checksum { algo, value }`), capability booleans for supported algorithms, write options for user-provided checksums, metadata return of the final checksum, and a `ChecksumLayer` that can auto-compute and enforce end-to-end verification using a preferred algorithm order.

# Motivation

- Give users a storage-agnostic way to attach and receive full-file checksums.
- Detect corruption or mismatched uploads early by comparing expected vs actual values.
- Provide an opt-in layer to fill gaps where backends cannot verify or return checksums.
- Keep changes minimal and consistent with existing `Capability` boolean style.

# Guide-level explanation

## New concepts
- `ChecksumAlgo`: algorithms we support (`Crc64Nvme`, `Crc32c`, `Md5`, `Sha256`, extensible).
- `Checksum`: holds exactly one algorithm and the full-file checksum bytes.
- `ChecksumLayer`: optional layer that computes/checks checksums with a preferred algorithm list and an `enforce` flag.

## Examples

### Write with a user-computed checksum (no layer)
```rust,no_run
use opendal::services;
use opendal::{Checksum, ChecksumAlgo, Operator, Result};

fn crc64_nvme_of(data: &[u8]) -> Vec<u8> {
    // user-side computation (placeholder)
    vec![0; 8]
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = services::Memory::default();
    let op = Operator::new(builder)?.finish();

    let data = b"hello checksum".to_vec();
    let expected = Checksum::new(ChecksumAlgo::Crc64Nvme, crc64_nvme_of(&data));

    // Backend supports CRC64-NVMe. Mismatch returns ErrorKind::ChecksumMismatch.
    op.write_with("foo.txt", data)
        .checksum(expected)
        .await?;
    Ok(())
}
```

### Read and inspect checksum from metadata
```rust,no_run
use opendal::services;
use opendal::{Operator, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = services::Memory::default();
    let op = Operator::new(builder)?.finish();

    let meta = op.stat("foo.txt").await?;
    if let Some(cs) = meta.checksum() {
        println!("algo={:?}, value={:x?}", cs.algo, cs.value);
    }
    Ok(())
}
```

### Enable end-to-end verification via ChecksumLayer (auto-compute)
```rust,no_run
use opendal::layers::ChecksumLayer;
use opendal::services;
use opendal::{ChecksumAlgo, Operator, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = services::Memory::default();

    // Prefer CRC64-NVMe, fall back to Sha256. 
    // Enforce=true: if backend lacks support, compute locally; 
    // any mismatch errors out.
    let op = Operator::new(builder)?
        .layer(ChecksumLayer::new().preferred(vec![ChecksumAlgo::Crc64Nvme, ChecksumAlgo::Sha256]).enforce(true))
        .finish();

    // User does not provide checksum; layer will compute and attach automatically.
    op.write("bar.bin", b"data".to_vec()).await?;

    // If metadata lacks the preferred checksum, the layer will stream-read and compute.
    let _ = op.read("bar.bin").await?;
    Ok(())
}
```

### Error on mismatch
```rust,no_run
use opendal::services;
use opendal::{Checksum, ChecksumAlgo, Operator, Result, ErrorKind};

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = services::Memory::default();
    let op = Operator::new(builder)?.finish();

    let wrong = Checksum::new(ChecksumAlgo::Sha256, vec![0; 32]);
    let res = op
        .write_with("bad.bin", b"payload".to_vec())
        .checksum(wrong)
        .await;

    assert!(matches!(res, Err(err) if err.kind() == ErrorKind::ChecksumMismatch));
    Ok(())
}
```

# Reference-level explanation

## Data types
- `ChecksumAlgo`: enum of supported algorithms. Extending this enum is allowed.
- `Checksum`: `{ algo: ChecksumAlgo, value: Vec<u8> }`; represents the full file only.
- `Metadata`: add `checksum: Option<Checksum>` plus helpers (`checksum()`, `crc64_nvme()`, etc.).

## Capability
- Add boolean fields to `Capability`: `checksum_crc64_nvme`, `checksum_crc32c`, `checksum_md5`, `checksum_sha256`.
- Semantics: `true` means the backend can accept and return that algorithm for full-file checksum.

## Write path
- `WriteOptions` / `OpWrite` gains `checksum: Option<Checksum>`.
- Flow:
  1. If `checksum` is provided and its algo flag is `false` in capability, return `Unsupported`.
  2. If supported, pass to backend; mismatch returns `ChecksumMismatch`.
  3. Response metadata includes the final checksum (from backend or layer).

## Read/stat path
- If backend provides checksum, fill `Metadata::checksum`.
- Otherwise leave `None`; `ChecksumLayer` may compute and inject.

## ChecksumLayer
- Config: `preferred(Vec<ChecksumAlgo>)`, `enforce(bool)`.
- Selection: pick the first preferred algo whose capability flag is true; if none and `enforce=false`, skip; if `enforce=true`, compute locally anyway.
- Write: if backend cannot verify, stream-compute chosen algo; compare against provided `checksum` (if any); mismatch -> `ChecksumMismatch`; inject result into returned metadata.
- Read: if metadata lacks chosen algo, stream-compute; mismatch -> `ChecksumMismatch`; if `enforce=true` and cannot obtain, surface `Unsupported` or mismatch.

## Errors
- New `ErrorKind::ChecksumMismatch` for value differences.
- Unsupported algorithm keeps using existing `Unsupported` error kind.

## Backward compatibility
- `content_md5` stays; when backends return MD5, it can populate both `content_md5` and `checksum(algo=Md5)`.
- No behavior change for users who ignore checksum features.

# Drawbacks
- More boolean fields in `Capability`; adding many algorithms enlarges the struct.
- `ChecksumLayer` can add CPU cost for large objects when enforce is enabled.

# Rationale and alternatives
- Chose capability booleans to match existing style and keep `Capability: Copy`.
- Rejected multi-checksum containers to keep the surface small and semantics single-valued.
- Rejected HashSet/bitmask because booleans are already the established pattern in `Capability`.

# Prior art
- Cloud SDKs commonly expose a single MD5/CRC32C field (e.g., GCS, OSS); we generalize to multiple algorithms via booleans.
- Middleware-style checksum verification mirrors S3 client behaviors but made storage-agnostic here.

# Unresolved questions
- Default preferred order for `ChecksumLayer` (proposed: `Crc64Nvme`, `Sha256`, `Crc32c`, `Md5`).
- Per-backend capability matrix (which algorithms to mark true by default).

# Future possibilities
- Add more algorithms (e.g., `Sha1`) with new booleans.
- Optional `reverify_on_read` flag in `ChecksumLayer` to recompute even when a checksum exists.
- Expose checksum info in presign responses when services support checksum headers.
