# Upgrade to v0.50

## Breaking change

### `OperatorInfo` exposes a single capability

`OperatorInfo.fullCapability` and `OperatorInfo.nativeCapability` have been replaced by `OperatorInfo.capability`.

```diff
-Capability caps = operator.info().fullCapability;
+Capability caps = operator.info().capability;
```

# Upgrade to v0.49

## Breaking changes

### Hugging Face config requires `repoType`

`ServiceConfig.Hf` no longer relies on the backend's implicit `model` default. Set `repoType` explicitly when creating Hugging Face operators:

```diff
 ServiceConfig.Hf config = ServiceConfig.Hf.builder()
+    .repoType("model")
     .repoId("username/repo")
     .build();
```

Use the appropriate value for the repository: `model`, `dataset`, `space`, or `bucket`.

### Generated service configs use canonical service names

Generated `ServiceConfig` entries now use canonical OpenDAL service names.

- Replace `ServiceConfig.Huggingface` with `ServiceConfig.Hf`.
- `scheme()` now returns hyphenated canonical service names for affected services, such as `aliyun-drive`, `cloudflare-kv`, `hdfs-native`, `vercel-artifacts`, `vercel-blob`, and `yandex-disk`, instead of the previous underscore variants.

### Removed services

The following services have been removed:

- **Chainsafe** service has been removed ([PR-5744](https://github.com/apache/opendal/pull/5744/)) - The service has been sunset.
- **libsql** service has been removed ([PR-5616](https://github.com/apache/opendal/pull/5616/)) - Dead service removal.

### Batch operations removed

[PR-5393](https://github.com/apache/opendal/pull/5393/) removes the batch concept from OpenDAL. All batch-related operations and capabilities have been removed.

### Capability changes

- `write_multi_align_size` capability has been removed ([PR-5322](https://github.com/apache/opendal/pull/5322/))
- Added new `shared` capability ([PR-5328](https://github.com/apache/opendal/pull/5328/))

### Options-based API

New options classes have been introduced for structured operation configuration:

- `ReadOptions` - for read operations
- `WriteOptions` - for write operations  
- `ListOptions` - for list operations
- `StatOptions` - for stat operations

Example usage:

```java
// Read with options
ReadOptions options = ReadOptions.builder()
    .range(0, 1024)
    .ifMatch("etag")
    .build();
byte[] data = operator.read("path/to/file", options);

// Write with options
WriteOptions options = WriteOptions.builder()
    .contentType("text/plain")
    .cacheControl("max-age=3600")
    .build();
operator.write("path/to/file", data, options);
```

# Upgrade to v0.48

## Breaking change

[PR-6169](https://github.com/apache/opendal/pull/6169/) The `append` method in `AsyncOperator` has been deprecated. Please use the `write` method with `WriteOptions.builder().append(true).build()` instead.

# Upgrade to v0.47

## Breaking change

artifactId of the `opendal-java` has changed from to `opendal` to align with the convention of entire OpenDAL project.

```diff
<dependencies>
<dependency>
  <groupId>org.apache.opendal</groupId>
-  <artifactId>opendal-java</artifactId>
+  <artifactId>opendal</artifactId>
  <version>${opendal.version}</version>
</dependency>
<dependency>
  <groupId>org.apache.opendal</groupId>
-  <artifactId>opendal-java</artifactId>
+  <artifactId>opendal</artifactId>
  <version>${opendal.version}</version>
  <classifier>${os.detected.classifier}</classifier>
</dependency>
</dependencies>
```

# Upgrade to v0.46

## Breaking change

[PR-4641](https://github.com/apache/opendal/pull/4641/) renames async `Operator` to `AsyncOperator` and `BlockingOperator` to `Operator`.

# Upgrade to v0.44

## Breaking change

Because of [a TLS lib issue](https://github.com/apache/opendal/issues/3650), we temporarily disable the `services-ftp` feature.

# Upgrade to v0.41

## Breaking change for constructing operators

[PR-3166](https://github.com/apache/opendal/pull/3166) changes the API for constructing operators:

Previous:

```java
new BlockingOperator(scheme, config);
new Operator(scheme, config);
```

Current:

```java
BlockingOperator.of(scheme, config);
Operator.of(scheme, config);
```

Now, there is no public constructor for operators, but only factory methods. In this way, the APIs are free to do arbitrary verifications and preparations before constructing operators.
