- Proposal Name: `multipart`
- Start Date: 2022-07-11
- RFC PR: [apache/opendal#438](https://github.com/apache/opendal/pull/438)
- Tracking Issue: [apache/opendal#439](https://github.com/apache/opendal/issues/439)

# Summary

Add multipart support in OpenDAL.

# Motivation

[Multipart Upload](https://docs.aws.amazon.com/AmazonS3/latest/userguide/mpuoverview.html) APIs are widely used in object storage services to upload large files concurrently and resumable.

A successful multipart upload includes the following steps:

- `CreateMultipartUpload`: Start a new multipart upload.
- `UploadPart`: Upload a single part with the previously uploaded id.
- `CompleteMultipartUpload`: Complete a multipart upload to get a regular object.

To cancel a multipart upload, users need to call `AbortMultipartUpload`.

Apart from those APIs, most object services also provide a list API to get the current multipart uploads status:

- `ListMultipartUploads`: List current ongoing multipart uploads
- `ListParts`: List already uploaded parts.

Before `CompleteMultipartUpload` has been called, users can't read already uploaded parts.

After `CompleteMultipartUpload` or `AbortMultipartUpload` has been called, all uploaded parts will be removed.

Object storage services commonly allow 10000 parts, and every part will allow up to 5 GiB. This way, users can upload a file up to 48.8 TiB.

OpenDAL users can upload objects larger than 5 GiB via supporting multipart uploads.

# Guide-level explanation

Users can start a multipart upload via:

```rust
let mp = op.object("path/to/file").create_multipart().await?;
```

Or build a multipart via already known upload id:

```rust
let mp = op.object("path/to/file").into_multipart("<upload_id>");
```

With `Multipart`, we can upload a new part:

```rust
let part = mp.write(part_number, content).await?;
```

After all parts have been uploaded, we can finish this upload:

```rust
let _ = mp.complete(parts).await?;
```

Or, we can abort already uploaded parts:

```rust
let _ = mp.abort().await?;
```

# Reference-level explanation

`Accessor` will add the following APIs:

```rust
pub trait Accessor: Send + Sync + Debug {
    async fn create_multipart(&self, args: &OpCreateMultipart) -> Result<String> {
        let _ = args;
        unimplemented!()
    }

    async fn write_multipart(&self, args: &OpWriteMultipart) -> Result<PartWriter> {
        let _ = args;
        unimplemented!()
    }

    async fn complete_multipart(&self, args: &OpCompleteMultipart) -> Result<()> {
        let _ = args;
        unimplemented!()
    }

    async fn abort_multipart(&self, args: &OpAbortMultipart) -> Result<()> {
        let _ = args;
        unimplemented!()
    }
}
```

While closing a `PartWriter`, a `Part` will be generated.

`Operator` will build APIs based on `Accessor`:

```rust
impl Object {
    async fn create_multipart(&self) -> Result<Multipart> {}
    fn into_multipart(&self, upload_id: &str) -> Multipart {}
}

impl Multipart {
    async fn write(&self, part_number: usize, bs: impl AsRef<[u8]>) -> Result<Part> {}
    async fn writer(&self, part_number: usize, size: u64) -> Result<impl PartWrite> {}
    async fn complete(&self, ps: &[Part]) -> Result<()> {}
    async fn abort(&self) -> Result<()> {}
}
```

# Drawbacks

None.

# Rationale and alternatives

## Why not add new object modes?

It seems natural to add a new object mode like `multipart`.

```rust
pub enum ObjectMode {
    FILE,
    DIR,
    MULTIPART,
    Unknown,
}
```

However, to make this work, we need big API breaks that introduce `mode` in Object.

And we need to change every API call to accept `mode` as args.

For example:

```rust
let _ = op.object("path/to/dir/").list(ObjectMODE::MULTIPART);
let _ = op.object("path/to/file").stat(ObjectMODE::MULTIPART)
```

## Why not split Object into File and Dir?

We can split `Object` into `File` and `Dir` to avoid requiring `mode` in API. There is a vast API breakage too.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

## Support list multipart uploads

We can support listing multipart uploads to list ongoing multipart uploads so we can resume an upload or abort them.

## Support list part

We can support listing parts to list already uploaded parts for an upload.
