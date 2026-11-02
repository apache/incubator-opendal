# Apache OpenDAL™: *One Layer, All Storage.*

[![GitHub Discussions](https://img.shields.io/github/discussions/apache/opendal)](https://github.com/apache/opendal/discussions)
[![Discord](https://img.shields.io/discord/1081052318650339399?logo=discord&label=discord)](https://opendal.apache.org/discord)
[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/apache/opendal)

Apache OpenDAL™ (`/ˈoʊ.pən.dæl/`, pronounced "OH-puhn-dal") is an Open Data Access Layer that gives every language a unified way to access object storage, file storage, cloud SaaS, databases, protocols, and key-value services.

Apache OpenDAL™ is guided by its vision of **One Layer, All Storage** and its core principles: **Open Community**, **Solid Foundation**, **Fast Access**, **Object Storage First**, and **Extensible Architecture**. Read the explained vision at [OpenDAL Vision](https://opendal.apache.org/vision).

<img src="./website/static/img/architectural.png" alt="OpenDAL Architecture" width="100%" />

## At a Glance

- Project: Apache OpenDAL™
- Vision: **One Layer, All Storage**
- Core package: Rust crate [`opendal`][Rust Core Link]
- Main abstraction: `Operator`
- Extension points: language bindings, layers, and services
- Common layers: retry, timeout, logging, tracing, metrics, throttling, and concurrency control
- Access targets: object storage, file systems, cloud SaaS, databases, protocols, and key-value services

## Why OpenDAL

Apache OpenDAL™ turns the vision of **One Layer, All Storage** into a practical data access layer for applications, libraries, and data systems.

- **Zero-cost core**: built in Rust with composable services and layers, so applications only enable the backends and capabilities they use.
- **Production-ready access**: add retry, timeout, logging, tracing, metrics, throttling, and concurrency limits through reusable layers.
- **One API, all storage**: access object storage, file systems, cloud SaaS, databases, protocols, and key-value services through the same interface.
- **Open and extensible**: add new services, layers, and language bindings while keeping the same unified access model.

## Choose Your Language

Start with the binding for your application runtime. Each binding provides access to the same OpenDAL service model while following its language ecosystem.

> **Note**: Each binding has its own independent version number, which may differ from the Rust core version. When checking for updates or compatibility, always refer to the specific binding's version rather than the core version.

| | | | |
| :---: | :---: | :---: | :---: |
| <img src="./website/static/img/bindings/rust.svg" width="42" height="42" alt="Rust logo"><br>**[Rust Core]**<br>[Package][Rust Core Link] · [Docs][Rust Core Release Docs] · [Dev Docs][Rust Core Dev Docs] | <img src="./website/static/img/bindings/c.svg" width="42" height="42" alt="C logo"><br>**[C Binding]**<br>[Dev Docs][C Binding Dev Docs] | <img src="./website/static/img/bindings/cpp.svg" width="42" height="42" alt="C++ logo"><br>**[Cpp Binding]**<br>[Dev Docs][Cpp Binding Dev Docs] | <img src="./website/static/img/bindings/d.svg" width="42" height="42" alt="D logo"><br>**[D Binding]** |
| <img src="./website/static/img/bindings/dart.svg" width="42" height="42" alt="Dart logo"><br>**[Dart Binding]** | <img src="./website/static/img/bindings/dotnet.svg" width="42" height="42" alt=".NET logo"><br>**[Dotnet Binding]** | <img src="./website/static/img/bindings/go.svg" width="42" height="42" alt="Go logo"><br>**[Go Binding]**<br>[Package][Go Binding Link] · [Docs][Go Release Docs] | <img src="./website/static/img/bindings/haskell.svg" width="42" height="42" alt="Haskell logo"><br>**[Haskell Binding]** |
| <img src="./website/static/img/bindings/java.svg" width="42" height="42" alt="Java logo"><br>**[Java Binding]**<br>[Package][Java Binding Link] · [Docs][Java Binding Release Docs] · [Dev Docs][Java Binding Dev Docs] | <img src="./website/static/img/bindings/lua.svg" width="42" height="42" alt="Lua logo"><br>**[Lua Binding]** | <img src="./website/static/img/bindings/nodejs.svg" width="42" height="42" alt="Node.js logo"><br>**[Node.js Binding]**<br>[Package][Node.js Binding Link] · [Dev Docs][Node.js Binding Dev Docs] | <img src="./website/static/img/bindings/ocaml.svg" width="42" height="42" alt="OCaml logo"><br>**[OCaml Binding]** |
| <img src="./website/static/img/bindings/php.svg" width="42" height="42" alt="PHP logo"><br>**[PHP Binding]** | <img src="./website/static/img/bindings/python.svg" width="42" height="42" alt="Python logo"><br>**[Python Binding]**<br>[Package][Python Binding Link] · [Dev Docs][Python Binding Dev Docs] | <img src="./website/static/img/bindings/ruby.svg" width="42" height="42" alt="Ruby logo"><br>**[Ruby Binding]**<br>[Package][Ruby Binding Link] · [Docs][Ruby Binding Docs] | <img src="./website/static/img/bindings/swift.svg" width="42" height="42" alt="Swift logo"><br>**[Swift Binding]** |
| <img src="./website/static/img/bindings/zig.svg" width="42" height="42" alt="Zig logo"><br>**[Zig Binding]** |  |  |  |

## Choose Your Layers

Add layers when your application needs cross-service behavior such as retries, timeouts, observability, or traffic control.

| | | | |
| :---: | :---: | :---: | :---: |
| <img src="./website/static/img/layers/retry.svg" width="42" height="42" alt="Retry icon"><br>**[RetryLayer]**<br>Retry temporary failures. | <img src="./website/static/img/layers/timeout.svg" width="42" height="42" alt="Timeout icon"><br>**[TimeoutLayer]**<br>Bound slow or hanging operations. | <img src="./website/static/img/layers/logging.svg" width="42" height="42" alt="Logging icon"><br>**[LoggingLayer]**<br>Emit structured operation logs. | <img src="./website/static/img/layers/tracing.svg" width="42" height="42" alt="Tracing icon"><br>**[TracingLayer]**<br>Trace requests across systems. |
| <img src="./website/static/img/layers/metrics.svg" width="42" height="42" alt="Metrics icon"><br>**[MetricsLayer]**<br>Export operation metrics. | <img src="./website/static/img/layers/prometheus.svg" width="42" height="42" alt="Prometheus icon"><br>**[PrometheusLayer]**<br>Expose Prometheus metrics. | <img src="./website/static/img/layers/otel.svg" width="42" height="42" alt="OpenTelemetry icon"><br>**[OtelMetricsLayer]**<br>Export OpenTelemetry metrics. | <img src="./website/static/img/layers/traffic-control.svg" width="42" height="42" alt="Traffic control icon"><br>**Traffic Control**<br>[ThrottleLayer] · [ConcurrentLimitLayer] |
| <img src="./website/static/img/layers/content-type.svg" width="42" height="42" alt="Content type icon"><br>**[MimeGuessLayer]**<br>Infer `Content-Type` from paths. | <img src="./website/static/img/layers/route.svg" width="42" height="42" alt="Route icon"><br>**[RouteLayer]**<br>Route operations by path. | <img src="./website/static/img/layers/cache.svg" width="42" height="42" alt="Cache icon"><br>**[FoyerLayer]**<br>Add hybrid cache behavior. | <img src="./website/static/img/layers/layers.svg" width="42" height="42" alt="Layers icon"><br>**[All Layers][Layers Docs]**<br>Explore the full layer list. |

Explore all available layers in the [layers documentation][Layers Docs].

## Choose Your Services

Pick the storage services that your application needs. See the full OpenDAL service configuration docs in the [services documentation][Services Docs].

<table width="100%">
  <tr>
    <th colspan="3" align="left">Object Storage</th>
    <th colspan="3" align="left">File Storage</th>
  </tr>
  <tr>
    <td width="16.66%"><a href="https://aws.amazon.com/s3/"><img src="./website/static/img/services/s3.svg" width="18" height="18" alt="AWS logo"> s3</a></td>
    <td width="16.66%"><a href="https://cloud.google.com/storage"><img src="./website/static/img/services/gcs.png" width="18" height="18" alt="Google Cloud logo"> gcs</a></td>
    <td width="16.66%"><a href="https://azure.microsoft.com/en-us/services/storage/blobs/"><img src="./website/static/img/services/azure.svg" width="18" height="18" alt="Azure logo"> azblob</a></td>
    <td width="16.66%"><a href="https://opendal.apache.org/docs/rust/opendal/services/struct.Fs.html"><img src="./website/static/img/services/opendal.svg" width="18" height="18" alt="OpenDAL logo"> fs</a></td>
    <td width="16.66%"><a href="https://hadoop.apache.org/docs/r3.3.4/hadoop-project-dist/hadoop-hdfs/HdfsDesign.html"><img src="./website/static/img/services/hadoop.ico" width="18" height="18" alt="Apache Hadoop logo"> hdfs</a></td>
    <td width="16.66%"><a href="https://github.com/Kimahriman/hdfs-native"><img src="./website/static/img/services/hadoop.ico" width="18" height="18" alt="Apache Hadoop logo"> hdfs-native</a></td>
  </tr>
  <tr>
    <td><a href="https://www.aliyun.com/product/oss"><img src="./website/static/img/services/oss.svg" width="26" height="16" alt="Alibaba Cloud logo"> oss</a></td>
    <td><a href="https://www.huaweicloud.com/intl/en-us/product/obs.html"><img src="./website/static/img/services/obs.png" width="40" height="16" alt="Huawei Cloud logo"> obs</a></td>
    <td><a href="https://www.tencentcloud.com/products/cos"><img src="./website/static/img/services/cos.svg" width="26" height="18" alt="Tencent Cloud logo"> cos</a></td>
    <td><a href="https://hadoop.apache.org/docs/stable/hadoop-project-dist/hadoop-hdfs/WebHDFS.html"><img src="./website/static/img/services/hadoop.ico" width="18" height="18" alt="Apache Hadoop logo"> webhdfs</a></td>
    <td><a href="https://lakefs.io/"><img src="./website/static/img/services/lakefs.ico" width="18" height="18" alt="lakeFS logo"> lakefs</a></td>
    <td><a href="https://ipfs.tech/"><img src="./website/static/img/services/ipfs.ico" width="18" height="18" alt="IPFS logo"> ipfs</a></td>
  </tr>
  <tr>
    <td><a href="https://www.volcengine.com/product/tos"><img src="./website/static/img/services/volcengine.png" width="18" height="18" alt="Volcengine logo"> tos</a></td>
    <td><a href="https://www.backblaze.com/"><img src="./website/static/img/services/backblaze.png" width="18" height="18" alt="Backblaze logo"> b2</a></td>
    <td><a href="https://docs.openstack.org/swift/latest/"><img src="./website/static/img/services/openstack.png" width="18" height="18" alt="OpenStack logo"> swift</a></td>
    <td><a href="https://docs.ipfs.tech/concepts/file-systems/"><img src="./website/static/img/services/ipfs.ico" width="18" height="18" alt="IPFS logo"> ipmfs</a></td>
    <td><a href="https://learn.microsoft.com/en-us/rest/api/storageservices/file-service-rest-api"><img src="./website/static/img/services/azure.svg" width="18" height="18" alt="Azure logo"> azfile</a></td>
    <td><a href="https://azure.microsoft.com/en-us/products/storage/data-lake-storage/"><img src="./website/static/img/services/azure.svg" width="18" height="18" alt="Azure logo"> azdls</a></td>
  </tr>
  <tr>
    <td><a href="https://www.upyun.com/"><img src="./website/static/img/services/upyun.png" width="18" height="18" alt="Upyun logo"> upyun</a></td>
    <td><a href="https://vercel.com/docs/storage/vercel-blob"><img src="./website/static/img/services/vercel.png" width="18" height="18" alt="Vercel logo"> vercel-blob</a></td>
    <td></td>
    <td><a href="https://docs.alluxio.io/os/user/stable/en/api/REST-API.html"><img src="./website/static/img/services/alluxio.svg" width="18" height="18" alt="Alluxio logo"> alluxio</a></td>
    <td><a href="https://github.com/Tencent/tencent-goosefs-rust-sdk"><img src="./website/static/img/services/cos.svg" width="26" height="18" alt="Tencent Cloud logo"> goosefs</a></td>
    <td><a href="https://docs.databricks.com/en/dbfs/index.html"><img src="./website/static/img/services/databricks.png" width="18" height="18" alt="Databricks logo"> dbfs</a></td>
  </tr>
  <tr>
    <td></td>
    <td></td>
    <td></td>
    <td><a href="https://www.mongodb.com/docs/manual/core/gridfs/"><img src="./website/static/img/services/mongodb.ico" width="18" height="18" alt="MongoDB logo"> gridfs</a></td>
    <td><a href="https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system"><img src="./website/static/img/services/mdn.svg" width="18" height="18" alt="MDN Web Docs logo"> opfs</a></td>
    <td><a href="https://github.com/bytedance/monoio"><img src="./website/static/img/services/github.svg" width="18" height="18" alt="GitHub logo"> monoiofs</a></td>
  </tr>
  <tr>
    <td></td>
    <td></td>
    <td></td>
    <td><a href="https://github.com/compio-rs/compio/"><img src="./website/static/img/services/github.svg" width="18" height="18" alt="GitHub logo"> compfs</a></td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <th colspan="3" align="left">Cloud SaaS</th>
    <th colspan="3" align="left">Standard Protocols</th>
  </tr>
  <tr>
    <td><a href="https://www.google.com/drive/"><img src="./website/static/img/services/gdrive.png" width="18" height="18" alt="Google Drive logo"> gdrive</a></td>
    <td><a href="https://www.dropbox.com/"><img src="./website/static/img/services/dropbox.ico" width="18" height="18" alt="Dropbox logo"> dropbox</a></td>
    <td><a href="https://www.microsoft.com/en-us/microsoft-365/onedrive/online-cloud-storage"><img src="./website/static/img/services/onedrive.svg" width="18" height="18" alt="OneDrive logo"> onedrive</a></td>
    <td><a href="https://developer.mozilla.org/en-US/docs/Web/HTTP"><img src="./website/static/img/services/http.png" width="18" height="18" alt="HTTP icon"> http</a></td>
    <td><a href="https://datatracker.ietf.org/doc/html/rfc959"><img src="./website/static/img/services/ftp.png" width="18" height="18" alt="FTP icon"> ftp</a></td>
    <td><a href="https://datatracker.ietf.org/doc/html/rfc4918"><img src="./website/static/img/services/webdav.png" width="18" height="18" alt="WebDAV icon"> webdav</a></td>
  </tr>
  <tr>
    <td><a href="https://www.aliyundrive.com/"><img src="./website/static/img/services/aliyundrive.png" width="18" height="18" alt="Aliyun Drive logo"> aliyun-drive</a></td>
    <td><a href="https://huggingface.co/"><img src="./website/static/img/services/huggingface.ico" width="18" height="18" alt="Hugging Face logo"> hf</a></td>
    <td><a href="https://github.com/"><img src="./website/static/img/services/github.svg" width="18" height="18" alt="GitHub logo"> github</a></td>
    <td><a href="https://datatracker.ietf.org/doc/html/draft-ietf-secsh-filexfer-02"><img src="./website/static/img/services/sftp.png" width="18" height="18" alt="SFTP icon"> sftp</a></td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://www.pcloud.com/"><img src="./website/static/img/services/pcloud.png" width="18" height="18" alt="pCloud logo"> pcloud</a></td>
    <td><a href="https://koofr.eu/"><img src="./website/static/img/services/koofr.ico" width="18" height="18" alt="Koofr logo"> koofr</a></td>
    <td><a href="https://www.seafile.com/"><img src="./website/static/img/services/seafile.png" width="18" height="18" alt="Seafile logo"> seafile</a></td>
    <td></td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://360.yandex.com/disk/"><img src="./website/static/img/services/yandex.png" width="18" height="18" alt="Yandex Disk logo"> yandex-disk</a></td>
    <td></td>
    <td></td>
    <td></td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <th colspan="3" align="left">Databases</th>
    <th colspan="3" align="left">Key-Value &amp; Embedded</th>
  </tr>
  <tr>
    <td><a href="https://www.sqlite.org/"><img src="./website/static/img/services/sqlite.ico" width="18" height="18" alt="SQLite logo"> sqlite</a></td>
    <td><a href="https://www.mysql.com/"><img src="./website/static/img/services/mysql.ico" width="18" height="18" alt="MySQL logo"> mysql</a></td>
    <td><a href="https://www.postgresql.org/"><img src="./website/static/img/services/postgresql.ico" width="18" height="18" alt="PostgreSQL logo"> postgresql</a></td>
    <td><img src="./website/static/img/services/opendal.svg" width="18" height="18" alt="OpenDAL logo"> memory</td>
    <td><a href="https://redis.io/"><img src="./website/static/img/services/redis.png" width="18" height="18" alt="Redis logo"> redis</a></td>
    <td><a href="https://etcd.io/"><img src="./website/static/img/services/etcd.png" width="18" height="18" alt="etcd logo"> etcd</a></td>
  </tr>
  <tr>
    <td><a href="https://www.mongodb.com/"><img src="./website/static/img/services/mongodb.ico" width="18" height="18" alt="MongoDB logo"> mongodb</a></td>
    <td><a href="https://surrealdb.com/"><img src="./website/static/img/services/surrealdb.svg" width="18" height="18" alt="SurrealDB logo"> surrealdb</a></td>
    <td><a href="https://developers.cloudflare.com/d1/"><img src="./website/static/img/services/cloudflare.ico" width="18" height="18" alt="Cloudflare logo"> d1</a></td>
    <td><a href="http://rocksdb.org/"><img src="./website/static/img/services/rocksdb.png" width="18" height="18" alt="RocksDB logo"> rocksdb</a></td>
    <td><a href="https://memcached.org/"><img src="./website/static/img/services/memcached.png" width="18" height="18" alt="Memcached logo"> memcached</a></td>
    <td><a href="https://developers.cloudflare.com/kv/">cloudflare-kv</a></td>
  </tr>
  <tr>
    <td></td>
    <td></td>
    <td></td>
    <td><a href="https://tikv.org/"><img src="./website/static/img/services/tikv.png" width="42" height="16" alt="TiKV logo"> tikv</a></td>
    <td><a href="https://www.foundationdb.org/"><img src="./website/static/img/services/foundationdb.png" width="22" height="12" alt="FoundationDB logo"> foundationdb</a></td>
    <td><a href="https://crates.io/crates/sled"><img src="./website/static/img/services/crates.ico" width="18" height="18" alt="crates.io logo"> sled</a></td>
  </tr>
  <tr>
    <td></td>
    <td></td>
    <td></td>
    <td><a href="https://crates.io/crates/redb"><img src="./website/static/img/services/crates.ico" width="18" height="18" alt="crates.io logo"> redb</a></td>
    <td><a href="https://crates.io/crates/persy"><img src="./website/static/img/services/crates.ico" width="18" height="18" alt="crates.io logo"> persy</a></td>
    <td><a href="https://github.com/xacrimon/dashmap"><img src="./website/static/img/services/github.svg" width="18" height="18" alt="GitHub logo"> dashmap</a></td>
  </tr>
  <tr>
    <td></td>
    <td></td>
    <td></td>
    <td><a href="https://crates.io/crates/cacache"><img src="./website/static/img/services/crates.ico" width="18" height="18" alt="crates.io logo"> cacache</a></td>
    <td><a href="https://github.com/moka-rs/moka"><img src="./website/static/img/services/github.svg" width="18" height="18" alt="GitHub logo"> moka</a></td>
    <td><a href="https://github.com/moka-rs/mini-moka"><img src="./website/static/img/services/github.svg" width="18" height="18" alt="GitHub logo"> mini-moka</a></td>
  </tr>
  <tr>
    <td></td>
    <td></td>
    <td></td>
    <td><a href="https://github.com/foyer-rs/foyer"><img src="./website/static/img/services/github.svg" width="18" height="18" alt="GitHub logo"> foyer</a></td>
    <td><a href="https://docs.github.com/en/actions/using-workflows/caching-dependencies-to-speed-up-workflows"><img src="./website/static/img/services/github.svg" width="18" height="18" alt="GitHub logo"> ghac</a></td>
    <td><a href="https://vercel.com/docs/concepts/monorepos/remote-caching">vercel-artifacts</a></td>
  </tr>
</table>

## Examples

See [examples](./examples/) for runnable usage examples.

## Documentation

- Website: <https://opendal.apache.org>
- Vision: <https://opendal.apache.org/vision>
- Rust release docs: <https://docs.rs/opendal>
- Rust dev docs: <https://opendal.apache.org/docs/rust/opendal/>

## Contribute

OpenDAL is an active open-source project. We are always open to people who want to use it or contribute to it. Here are some ways to go.

- Start with [Contributing Guide](CONTRIBUTING.md).
- Submit [Issues](https://github.com/apache/opendal/issues/new) for bug report or feature requests.
- Start [Discussions](https://github.com/apache/opendal/discussions/new?category=q-a) for questions or ideas.
- Talk to community directly at [Discord](https://opendal.apache.org/discord).
- Report security vulnerabilities to [private mailing list](mailto:private@opendal.apache.org)

## Branding

The first and most prominent mentions must use the full form: **Apache OpenDAL™** of the name for any individual usage (webpage, handout, slides, etc.) Depending on the context and writing style, you should use the full form of the name sufficiently often to ensure that readers clearly understand the association of both the OpenDAL project and the OpenDAL software product to the ASF as the parent organization.

For more details, see the [Apache Product Name Usage Guide](https://www.apache.org/foundation/marks/guide).

## License and Trademarks

Licensed under the Apache License, Version 2.0: <http://www.apache.org/licenses/LICENSE-2.0>

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.

<!-- Link references -->

<!-- Binding references -->

[Rust Core]: core/README.md
[Rust Core Link]: https://crates.io/crates/opendal
[Rust Core Release Docs]: https://docs.rs/opendal
[Rust Core Dev Docs]: https://opendal.apache.org/docs/rust/opendal/
[C Binding]: bindings/c/README.md
[C Binding Dev Docs]: https://opendal.apache.org/docs/c/
[Cpp Binding]: bindings/cpp/README.md
[Cpp Binding Dev Docs]: https://opendal.apache.org/docs/cpp/
[D Binding]: bindings/d/README.md
[Dart Binding]: bindings/dart/README.md
[Dotnet Binding]: bindings/dotnet/README.md
[Go Binding]: bindings/go/README.md
[Go Binding Link]: https://pkg.go.dev/github.com/apache/opendal/bindings/go
[Go Release Docs]: https://pkg.go.dev/github.com/apache/opendal/bindings/go
[Haskell Binding]: bindings/haskell/README.md
[Java Binding]: bindings/java/README.md
[Java Binding Link]: https://central.sonatype.com/artifact/org.apache.opendal/opendal-java
[Java Binding Release Docs]: https://javadoc.io/doc/org.apache.opendal/opendal-java
[Java Binding Dev Docs]: https://opendal.apache.org/docs/java/
[Lua Binding]: bindings/lua/README.md
[Node.js Binding]: bindings/nodejs/README.md
[Node.js Binding Link]: https://www.npmjs.com/package/opendal
[Node.js Binding Dev Docs]: https://opendal.apache.org/docs/nodejs/
[OCaml Binding]: bindings/ocaml/README.md
[PHP Binding]: bindings/php/README.md
[Python Binding]: bindings/python/README.md
[Python Binding Link]: https://pypi.org/project/opendal/
[Python Binding Dev Docs]: https://opendal.apache.org/docs/python/
[Ruby Binding]: bindings/ruby/README.md
[Ruby Binding Link]: https://rubygems.org/gems/opendal
[Ruby Binding Docs]: https://opendal.apache.org/docs/ruby/
[Swift Binding]: bindings/swift/README.md
[Zig Binding]: bindings/zig/README.md

<!-- Layer references -->

[Layers Docs]: https://opendal.apache.org/docs/rust/opendal/layers/
[RetryLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.RetryLayer.html
[TimeoutLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.TimeoutLayer.html
[LoggingLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.LoggingLayer.html
[TracingLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.TracingLayer.html
[MetricsLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.MetricsLayer.html
[PrometheusLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.PrometheusLayer.html
[OtelMetricsLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.OtelMetricsLayer.html
[ThrottleLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.ThrottleLayer.html
[ConcurrentLimitLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.ConcurrentLimitLayer.html
[MimeGuessLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.MimeGuessLayer.html
[RouteLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.RouteLayer.html
[FoyerLayer]: https://opendal.apache.org/docs/rust/opendal/layers/struct.FoyerLayer.html

<!-- Service references -->

[Services Docs]: https://opendal.apache.org/docs/rust/opendal/services/
