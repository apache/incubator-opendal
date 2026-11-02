A distributed file system that provides high-throughput access to application data.

## Capabilities

Depending on its configuration and the backing system, this service can expose:

- [x] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [x] copy
- [x] rename
- [ ] ~~presign~~

Inspect the effective capability set with [`opendal_core::Operator::info`] and
[`opendal_core::OperatorInfo::capability`] after building an operator.

## Copy behavior

The HDFS service copies files to exact destination file paths and rejects
directory sources and destinations. Copy overwrites an existing destination by
removing it before copying the source. This replacement is not atomic: if the
copy fails after removing the destination, the destination can be missing or
incomplete.

## Differences with webhdfs

The [WebHDFS service](https://docs.rs/opendal-service-webhdfs) uses HDFS's
RESTful HTTP API.

## Features

HDFS support needs to enable feature `services-hdfs`.

## Configuration

Use [`crate::HdfsConfig`] for serializable configuration and this builder's
methods for direct construction. The field and method documentation defines
accepted values, defaults, and environment interaction.

## Environment

HDFS needs some environment set correctly.

- `JAVA_HOME`: the path to java home, could be found via `java -XshowSettings:properties -version`
- `HADOOP_HOME`: the path to hadoop home, opendal relays on this env to discover hadoop jars and set `CLASSPATH` automatically.

Most of the time, setting `JAVA_HOME` and `HADOOP_HOME` is enough. But there are some edge cases:

- If meeting errors like the following:

```shell
error while loading shared libraries: libjvm.so: cannot open shared object file: No such file or directory
```

Java's lib are not including in pkg-config find path, please set `LD_LIBRARY_PATH`:

```shell
export LD_LIBRARY_PATH=${JAVA_HOME}/lib/server:${LD_LIBRARY_PATH}
```

The path of `libjvm.so` could be different, please keep an eye on it.

- If meeting errors like the following:

```shell
(unable to get stack trace for java.lang.NoClassDefFoundError exception: ExceptionUtils::getStackTrace error.)
```

`CLASSPATH` is not set correctly or your hadoop installation is incorrect.

To set `CLASSPATH`:
```shell
export CLASSPATH=$(find $HADOOP_HOME -iname "*.jar" | xargs echo | tr ' ' ':'):${CLASSPATH}
```

- If HDFS has High Availability (HA) enabled with multiple available NameNodes, some configuration is required:
1. Obtain the entire HDFS config folder (usually located at HADOOP_HOME/etc/hadoop).
2. Set the environment variable HADOOP_CONF_DIR to the path of this folder.
```shell
export HADOOP_CONF_DIR=<path of the config folder>
```
3. Append the HADOOP_CONF_DIR to the `CLASSPATH`
```shell
export CLASSPATH=$HADOOP_CONF_DIR:$HADOOP_CLASSPATH:$CLASSPATH
```
4. Use the `cluster_name` specified in the `core-site.xml` file (located in the HADOOP_CONF_DIR folder) to replace namenode:port.

```ignore
builder.name_node("hdfs://cluster_name");
```

### macOS Specific Note

If you encounter an issue during the build process on macOS with an error message similar to:

```shell
ld: unknown file type in $HADOOP_HOME/lib/native/libhdfs.so.0.0.0
clang: error: linker command failed with exit code 1 (use -v to see invocation)
```
This error is likely due to the fact that the official Hadoop build includes the libhdfs.so file for the x86-64 architecture, which is not compatible with aarch64 architecture required for MacOS.

To resolve this issue, you can add hdrs as a dependency in your Rust application's Cargo.toml file, and enable the vendored feature:

```toml
[dependencies]
hdrs = { version = "<version_number>", features = ["vendored"] }
```
Enabling the vendored feature ensures that hdrs includes the necessary libhdfs.so library built for the correct architecture.

## Example

### Via Builder

```rust,no_run
use opendal_core::Operator;
use opendal_service_hdfs::Hdfs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create fs backend builder.
    let builder = Hdfs::default()
        // Set the name node for hdfs.
        // If the string starts with a protocol type such as file://, hdfs://, or gs://, this protocol type will be used.
        .name_node("hdfs://127.0.0.1:9000")
        // Set the root for hdfs, all operations will happen under this root.
        //
        // NOTE: the root must be absolute path.
        .root("/tmp");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
