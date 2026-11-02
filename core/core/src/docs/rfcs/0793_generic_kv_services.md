- Proposal Name: `generic-kv-services`
- Start Date: 2022-10-03
- RFC PR: [apache/opendal#793](https://github.com/apache/opendal/pull/793)
- Tracking Issue: [apache/opendal#794](https://github.com/apache/opendal/issues/794)

# Summary

Add generic kv services support OpenDAL.

# Motivation

OpenDAL now has some kv services support:

- memory
- redis

However, maintaining them is complex and very easy to be wrong. We don't want to implement similar logic for every kv
service. This RFC intends to introduce a generic kv service so that we can:

- Implement OpenDAL Accessor on this generic kv service
- Add new kv service support via generic kv API.

# Guide-level explanation

No user-side changes.

# Reference-level explanation

OpenDAL will introduce a generic kv service:

```rust
trait KeyValueAccessor {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &[u8], value: &[u8]) -> Result<()>;
}
```

We will implement the OpenDAL service on `KeyValueAccessor`. To add new kv service support, users only need to implement
it against `KeyValueAccessor`.

## Spec

This RFC is mainly inspired
by [TiFS: FUSE based on TiKV](https://github.com/Hexilee/tifs/blob/main/contribution/design.md). We will use the
same `ScopedKey` idea in `TiFS`.

```rust
pub enum ScopedKey {
    Meta,
    Inode(u64),
    Block {
        ino: u64,
        block: u64,
    },
    Entry {
        parent: u64,
        name: String,
    },
}
```

We can encode a scoped key into a byte array as a key. Following is the common layout of an encoded key.

```text
+ 1byte +<----------------------------+ dynamic size +------------------------------------>+
|       |                                                                                  |
|       |                                                                                  |
|       |                                                                                  |
|       |                                                                                  |
|       |                                                                                  |
|       |                                                                                  |
|       v                                                                                  v
+------------------------------------------------------------------------------------------+
|       |                                                                                  |
| scope |                                       body                                       |
|       |                                                                                  |
+-------+----------------------------------------------------------------------------------+
```

### Meta

There is only one key in the meta scope. The meta key is designed to store metadata of our filesystem. Following is the
layout of an encoded meta key.

```text
+ 1byte +
|       |
|       |
|       |
|       |
|       |
|       |
|       v
+-------+
|       |
|   0   |
|       |
+-------+
```

This key will store data:

```rust
pub struct Meta {
    inode_next: u64,
}
```

The meta-structure contains only an auto-increasing counter `inode_next`, designed to generate an inode number.

### Inode

Keys in the inode scope are designed to store attributes of files. Following is the layout of an encoded inode key.

```text
+ 1byte +<-------------------------------+ 8bytes +--------------------------------------->+
|       |                                                                                  |
|       |                                                                                  |
|       |                                                                                  |
|       |                                                                                  |
|       |                                                                                  |
|       |                                                                                  |
|       v                                                                                  v
+------------------------------------------------------------------------------------------+
|       |                                                                                  |
|   1   |                               inode number                                       |
|       |                                                                                  |
+-------+----------------------------------------------------------------------------------+
```

This key will store data:

```rust
pub struct Inode {
    meta: Metadata,
    blocks: HashMap<u64, u32>,
}
```

blocks is the map from `block_id` -> `size`. We will use this map to calculate the correct blocks to read.

### Block

Keys in the block scope are designed to store blocks of a file. Following is the layout of an encoded block key.

```text
+ 1byte +<----------------- 8bytes ---------------->+<------------------- 8bytes ----------------->+
|       |                                           |                                              |
|       |                                           |                                              |
|       |                                           |                                              |
|       |                                           |                                              |
|       |                                           |                                              |
|       |                                           |                                              |
|       v                                           v                                              v
+--------------------------------------------------------------------------------------------------+
|       |                                           |                                              |
|   2   |              inode number                 |                  block index                 |
|       |                                           |                                              |
+-------+-------------------------------------------+----------------------------------------------+
```

### Entry

Keys in the file index scope are designed to store the entry of the file. Following is the layout of an encoded file
entry key.

```text
+ 1byte +<----------------- 8bytes ---------------->+<-------------- dynamic size ---------------->+
|       |                                           |                                              |
|       |                                           |                                              |
|       |                                           |                                              |
|       |                                           |                                              |
|       |                                           |                                              |
|       |                                           |                                              |
|       v                                           v                                              v
+--------------------------------------------------------------------------------------------------+
|       |                                           |                                              |
|   3   |     inode number of parent directory      |         file name in utf-8 encoding          |
|       |                                           |                                              |
+-------+-------------------------------------------+----------------------------------------------+
```

Store the correct inode number for this file.

```rust
pub struct Index {
    pub ino: u64,
}
```

# Drawbacks

None.

# Rationale and alternatives

None.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

None.
