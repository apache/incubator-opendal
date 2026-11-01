- Proposal Name: `list_metadata_reuse`
- Start Date: 2022-08-23
- RFC PR: [apache/opendal#561](https://github.com/apache/opendal/pull/561)
- Tracking Issue: [apache/opendal#570](https://github.com/apache/opendal/pull/570)

# Summary

Reuse metadata returned during listing, by extending `DirEntry` with some metadata fields.

# Motivation

Users may expect to browse metadata of some directories' child files and directories. Using `walk()` of `BatchOperator` seems to be an ideal way to complete this job. 

Thus, they start iterating on it, but soon they realized the `DirEntry`, could only offer the name (or path, more precisely) and access mode of the object, and it's not enough.

So they have to call `metadata()` for each name they extracted from the iterator.

The final example looks like:

```rust
let op = Operator::from_env(Scheme::Gcs)?.batch();

// here is a network request
let mut dir_stream = op.walk("/dir/to/walk")?;

while let Some(Ok(file)) = dir_stream.next().await {
    let path = file.path();

    // here is another network request
    let size = file.metadata().await?.content_length();
    println!("size of file {} is {}B", path, size);
}
```

But...wait! many storage-services returns object metadata when listing, like HDFS, AWS and GCS. The rust standard library returns metadata when listing local file systems, too.

In the previous versions of OpenDAL those fields were just get ignored. This wastes users' time on requesting on metadata.

# Guide-level explanation

The loop in main will be changed to the following code with this RFC:

```rust
while let Some(Ok(file)) = dir_stream.next().await {
    let size = if let Some(len) = file.content_length() {
        len
    } else {
        file.metadata().await?.content_length();
    };
    let name = file.path();
    println!("size of file {} is {}B", path, size);
}
```

# Reference-level explanation

Extend `DirEntry` with metadata fields:

```rust
pub struct DirEntry {
    acc: Arc<dyn Accessor>,

    mode: ObjectMode,
    path: String,

    // newly add metadata fields
    content_length: Option<u64>,  // size of file
    content_md5: Option<String>,
    last_modified: Option<OffsetDateTime>,
}

impl DirEntry {
    pub fn content_length(&self) -> Option<u64> {
        self.content_length
    }
    pub fn last_modified(&self) -> Option<OffsetDateTime> {
        self.last_modified
    }
    pub fn content_md5(&self) -> Option<OffsetDateTime> {
        self.content_md5
    }
}
```

For all services that supplies metadata during listing, like AWS, GCS and HDFS. Those optional fields will be filled up; Meanwhile for those services doesn't return metadata during listing, like in memory storages, just left them as `None`.

As you can see, for those services returning metadata when listing, the operation of listing metadata will save many unnecessary requests.

# Drawbacks
 
Add complexity to `DirEntry`. To use the improved features of `DirEntry`, users have to explicitly check the existence of metadata fields.

The size of `DirEntry` increased from 40 bytes to 80 bytes, a 100% percent growth requires more memory.

# Rational and alternatives

The largest drawback of performance usually comes from network or hard disk operations. By letting `DirEntry` storing some metadata, many redundant requests could be avoided.

## Embed a Structure Containing Metadata

Define a `MetaLite` structure containing some metadata fields, and embed it in `DirEntry`

```rust
struct MetaLite {
    pub content_length: u64,  // size of file
    pub content_md5: String,
    pub last_modified: OffsetDateTime,
}

pub struct DirEntry {
    acc: Arc<dyn Accessor>,
    
    mode: ObjectMode,
    path: String,
    
    // newly add metadata struct
    metadata: Option<MetaLite>,
}

impl DirEntry {
    // get size of file
    pub fn content_length(&self) -> Option<u64> {
        self.metadata.as_ref().map(|m| m.content_length)
    }
    // get the last modified time
    pub fn last_modified(&self) -> Option<OffsetDateTime> {
        self.metadata.as_ref().map(|m| m.last_modified)
    }
    // get md5 message digest
    pub fn content_md5(&self) -> Option<String> {
        self.metadata.as_ref().map(|m| m.content_md5)
    }
}
```

The existence of those newly added metadata fields is highly correlated. If one field does not exist, the others neither.

By wrapping them together in an embedded structure, 8 bytes of space for each `DirEntry` object could be saved. In the future, more metadata fields may be added to `DirEntry`, then a lot more space could be saved.

This approach could be slower because some intermediate functions are involved. But it's worth sacrificing rarely used features' performance to save memory.

## Embed a `ObjectMetadata` into `DirEntry`

- Embed a `ObjectMetadata` struct into `DirEntry`
- Remove the `ObjectMode` field in `DirEntry`
- Change `ObjectMetadata`'s `content_length` field's type to `Option<u64>`.

```rust
pub struct DirEntry {
    acc: Arc<dyn Accessor>,

    // - mode: ObjectMode, removed
    path: String,

    // newly add metadata struct
    metadata: ObjectMetadata,
}

impl DirEntry {
    pub fn mode(&self) -> ObjectMode {
        self.metadata.mode()
    }
    pub fn content_length(&self) -> Option<u64> {
        self.metadata.content_length()
    }
    pub fn content_md5(&self) -> Option<&str> {
        self.metadata.content_md5()
    }
    // other metadata getters...
}
```

In the degree of memory layout, it's the same as proposed way in this RFC. This approach offers more metadata fields and fewer changes to code.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

## Switch to Alternative Implement Approaches

As the growing of metadata fields, someday the alternatives could be better. And other RFCs will be raised then.

## More Fields

Add more metadata fields to DirEntry, like:

- accessed: the last access timestamp of object

## Simplified Get

Users have to explicitly check if those metadata fields actual present in the DirEntry. This may be done inside the getter itself.

```rust
let path = file.path();

// if content_length is not exist
// this getter will automatically fetch from the storage service.
let size = file.content_length().await?;

// the previous getter can cache metadata fetched from service
// so this function could return instantly.
let md5 = file.content_md5().await?;
println!("size of file {} is {}B, md5 outcome of file is {}", path, size, md5);
```
