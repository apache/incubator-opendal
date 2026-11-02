There two implementations of WebHDFS REST API:

- Native via HDFS Namenode and Datanode, data are transferred between nodes directly.
- [HttpFS](https://hadoop.apache.org/docs/stable/hadoop-hdfs-httpfs/index.html) is a gateway before hdfs nodes, data are proxied.

## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
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

## Differences with HDFS

The [HDFS service](https://docs.rs/opendal-service-hdfs) uses HDFS's native
Java client and requires a working Java and Hadoop environment. WebHDFS uses
the HTTP API instead.

## WebHDFS Compatibility Guidelines

### File Creation and Write

For [File creation and write](https://hadoop.apache.org/docs/r3.1.3/hadoop-project-dist/hadoop-hdfs/WebHDFS.html#Create_and_Write_to_a_File) operations,
OpenDAL WebHDFS is optimized for Hadoop Distributed File System (HDFS) versions 2.9 and later.
This involves two API calls in webhdfs, where the initial `put` call to the namenode is redirected to the datanode handling the file data.
The optional `noredirect` flag can be set to prevent redirection. If used, the API response body contains the datanode URL, which is then utilized for the subsequent `put` call with the actual file data.
OpenDAL automatically sets the `noredirect` flag with the first `put` call. This feature is supported starting from HDFS version 2.9.

### Multi-Write Support

OpenDAL WebHDFS supports multi-write operations by creating temporary files in the specified `atomic_write_dir`.
The final concatenation of these temporary files occurs when the writer is closed.
However, it's essential to be aware of HDFS concat restrictions for earlier versions,
where the target file must not be empty, and its last block must be full. Due to these constraints, the concat operation might fail for HDFS 2.6.
This issue, identified as [HDFS-6641](https://issues.apache.org/jira/browse/HDFS-6641), has been addressed in later versions of HDFS.

In summary, OpenDAL WebHDFS is designed for optimal compatibility with HDFS, specifically versions 2.9 and later.

## Configuration

Use [`crate::WebhdfsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Examples

### Via Builder

```rust,no_run
use std::sync::Arc;

use anyhow::Result;
use opendal_service_webhdfs::Webhdfs;
use opendal_core::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Webhdfs::default()
        // set the root for WebHDFS, all operations will happen under this root
        //
        // Note:
        // if the root exists, the builder will automatically create the
        // root directory for you
        // if the root exists and is a directory, the builder will continue working
        // if the root exists and is a file, the builder will fail on building backend
        .root("/path/to/dir")
        // set the endpoint of webhdfs namenode, controlled by dfs.namenode.http-address
        // default is http://127.0.0.1:9870
        .endpoint("http://127.0.0.1:9870")
        // set the delegation_token for builder
        .delegation("delegation_token")
        // set atomic_write_dir for builder
        .atomic_write_dir(".opendal_tmp/");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
