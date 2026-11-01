---
title: Java
sidebar_label: Overview
slug: /bindings/java
description: The OpenDAL Java binding — install it, see what it supports, and where to go next.
---

# Apache OpenDAL™ Java Binding

A native Java binding for OpenDAL: access S3, GCS, Azure Blob, HDFS, the local
filesystem, and [50+ more services](/services) through one API, with the
performance of the Rust core underneath.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::note
This binding has its own version number, independent of the Rust core. Check
compatibility against the binding's version, not the core's.
:::

## Status

Released and stable, published to
[Maven Central](https://central.sonatype.com/search?q=opendal&smo=true) under
the `org.apache.opendal` group.

## Capabilities

- **Sync and async** — use [`Operator`] for blocking calls or [`AsyncOperator`]
  for `CompletableFuture`-based calls, with the same verbs.
- **Typed or map-based config** — build from a typed `ServiceConfig` builder or
  a plain `Map<String, String>`.
- **All services included** — the native library bundles every backend; there
  are no build flags to enable.
- **Presigned URLs** for services that support them (async API).

## Installation

The binding ships a platform-specific native library, so you depend on the main
artifact plus a classifier for your platform. The simplest setup uses an OS
detector plugin to fill in the classifier automatically.

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

## Useful Links

- **API reference**: [opendal.apache.org/docs/java](https://opendal.apache.org/docs/java/)
- **Services & configuration**: [/services](/services)
- **Upgrade guide**: [`upgrade.md`](https://github.com/apache/opendal/blob/main/bindings/java/upgrade.md)
- **Source & examples**: [`bindings/java/`](https://github.com/apache/opendal/tree/main/bindings/java)

## Next steps

1. [Getting started](./02-getting-started.md) — a runnable program, then a real backend.
2. [Connecting to your storage](./03-connecting.md) — build an operator for any service.
3. [Common tasks](./04-tasks.md) — read, write, stream, list, presign, and more.
4. [Going to production](./05-production.md) — retries, errors, and capability checks.

[`Operator`]: https://opendal.apache.org/docs/java/org/apache/opendal/Operator.html
[`AsyncOperator`]: https://opendal.apache.org/docs/java/org/apache/opendal/AsyncOperator.html
</content>
</invoke>
