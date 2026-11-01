# Apache OpenDAL™ Java Bindings

[![](https://img.shields.io/badge/status-released-blue)](https://central.sonatype.com/search?q=opendal&smo=true)
[![Maven Central](https://img.shields.io/maven-central/v/org.apache.opendal/opendal.svg?logo=Apache+Maven&logoColor=blue)](https://central.sonatype.com/search?q=opendal&smo=true)
[![Website](https://img.shields.io/badge/opendal-OpenDAL_Website-red?logo=Apache&logoColor=red)](https://opendal.apache.org/docs/java/)

A native Java binding for Apache OpenDAL™: access S3, GCS, Azure Blob, HDFS, the
local filesystem, and many more services through one API.

We release the OpenDAL Java binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the Java binding version instead of the `opendal` crate
version.

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/java](https://opendal.apache.org/docs/bindings/java) — install, connect, common tasks, and going to production.
- **API reference**: [opendal.apache.org/docs/java](https://opendal.apache.org/docs/java/)
- **Services & configuration**: [opendal.apache.org/services](https://opendal.apache.org/services)
- **Upgrade guide**: [`upgrade.md`](./upgrade.md)

## Installation

The binding ships a platform-specific native library, so you depend on the main
artifact plus a classifier for your platform. An OS detector plugin fills in the
classifier automatically.

### Maven

```xml
<build>
<extensions>
  <extension>
    <groupId>kr.motd.maven</groupId>
    <artifactId>os-maven-plugin</artifactId>
    <version>1.7.0</version>
  </extension>
</extensions>
</build>

<dependencies>
  <dependency>
    <groupId>org.apache.opendal</groupId>
    <artifactId>opendal</artifactId>
    <version>${opendal.version}</version>
  </dependency>
  <dependency>
    <groupId>org.apache.opendal</groupId>
    <artifactId>opendal</artifactId>
    <version>${opendal.version}</version>
    <classifier>${os.detected.classifier}</classifier>
  </dependency>
</dependencies>
```

### Gradle

```groovy
plugins {
    id "com.google.osdetector" version "1.7.3"
}

dependencies {
    implementation "org.apache.opendal:opendal:$opendalVersion"
    implementation "org.apache.opendal:opendal:$opendalVersion:$osdetector.classifier"
}
```

On musl-based Linux distributions such as Alpine, use the `linux-x86_64-musl` or
`linux-aarch_64-musl` classifier instead of the detected one.

## Quickstart

```java
import java.util.HashMap;
import java.util.Map;
import org.apache.opendal.AsyncOperator;

public class Main {
  public static void main(String[] args) {
    final Map<String, String> conf = new HashMap<>();
    conf.put("root", "/tmp");

    try (AsyncOperator op = AsyncOperator.of("fs", conf)) {
      op.write("/path/to/data", "Hello world").join();
      System.out.println(new String(op.read("/path/to/data").join()));
    }
  }
}
```

Use the synchronous `Operator` for blocking calls, or `AsyncOperator` for
`CompletableFuture`-based calls.

## Reuse a reader

`Operator.createReader(path, readerOptions)` creates an `OperatorReader` for repeated
reads. `ReaderOptions` selects versions, conditions, and execution controls;
each call selects its own byte range.

```java
ReaderOptions options = ReaderOptions.builder().chunk(8 * 1024 * 1024L).build();
try (OperatorReader reader = op.createReader("large.bin", options)) {
    byte[] first = reader.read(0, 1024);
    byte[] next = reader.read(1024, 1024);
}
```

The [Java task guide](../../website/docs/20-bindings/java/04-tasks.md#read-part-of-a-file)
covers streams and resource lifetimes.
See [ReaderOptions](src/main/java/org/apache/opendal/ReaderOptions.java) for all
supported options, defaults, and constraints.

## Documentation

The full user guide — getting started, connecting to services, common tasks, and
going to production — lives at
[opendal.apache.org/docs/bindings/java](https://opendal.apache.org/docs/bindings/java).

## Contributing

The Maven `source-release.zip` includes `bindings/java` and the Rust `core`
workspace. After extracting it, run `./mvnw package` from `bindings/java`.
The wrapper downloads its JAR on first use; the source archive does not bundle it.

This project is built upon the native OpenDAL library and depends on JDK 8 or
later. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build the binding, run
tests, and apply the code style.

## Used by

Check out the [users](./users.md) list for more details on who is using OpenDAL.

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.

Compiled distributions include the following Mozilla Public License 2.0
components. Their source code is available from the linked project pages:

### Third-party software

- `colored` 3.1.1 ([source](https://crates.io/crates/colored/3.1.1), [homepage](https://github.com/mackwic/colored)), licensed under MPL-2.0.
- `option-ext` 0.2.0 ([source](https://crates.io/crates/option-ext/0.2.0), [homepage](https://github.com/soc/option-ext)), licensed under MPL-2.0.
- `persy` 1.8.1 ([source](https://crates.io/crates/persy/1.8.1), [homepage](https://persy.rs)), licensed under MPL-2.0.

The distribution includes the MPL-2.0 text in `META-INF/LICENSE-MPL-2.0.txt`.
</content>
