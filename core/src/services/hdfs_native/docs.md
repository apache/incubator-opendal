A distributed file system that provides high-throughput access to application data.
Using [Native Rust HDFS client](https://github.com/Kimahriman/hdfs-native).

## Capabilities

This service can be used to:

- [x] stat
- [x] read
- [x] write
- [ ] create_dir
- [x] delete
- [x] rename
- [x] list
- [x] blocking
- [ ] append

## Differences with webhdfs

[Webhdfs][crate::services::Webhdfs] is powered by hdfs's RESTful HTTP API.

## Differences with hdfs

[hdfs][crate::services::Hdfs] is powered by libhdfs and require the Java dependencies

## Features

HDFS-native support needs to enable feature `services-hdfs-native`.

## Configuration

- `root`: Set the work dir for backend.
- `url`: Set the url for backend.
- `enable_append`: enable the append capacity. Default is false. 

