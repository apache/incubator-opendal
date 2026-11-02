## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [ ] copy
- [ ] rename
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Note

OpenDAL will build rocksdb from source by default.

To link with existing rocksdb lib, please set one of the following:

- `ROCKSDB_LIB_DIR` to the dir that contains `librocksdb.so`
- `ROCKSDB_STATIC` to the dir that contains `librocksdb.a`

If the version of RocksDB is below 6.0, you may encounter compatibility
issues. It is advisable to follow the steps provided in the [`INSTALL`](https://github.com/facebook/rocksdb/blob/main/INSTALL.md)
file to build rocksdb, rather than relying on system libraries that
may be outdated and incompatible.

## Configuration

Use [`crate::RocksdbConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_core::Result;
use opendal_service_rocksdb::Rocksdb;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Rocksdb::default()
        .datadir("/tmp/opendal/rocksdb");

    let op: Operator = Operator::new(builder)?;
    Ok(())
}
```
