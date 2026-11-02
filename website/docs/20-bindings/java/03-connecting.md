---
title: Connecting to your storage
sidebar_label: Connecting to your storage
description: Build an OpenDAL operator in Java for any backend — schemes, typed config, and credentials.
---

# Connecting to your storage

Switching backends is a configuration change, not a code change. For the
configuration keys of a specific service, see [Services](/services).

Unlike the Rust core, the Java native library includes every service — there are
no build flags. Once the dependency is on the classpath, every backend is
available.

## Build an operator

### From a scheme and map

The first argument is the service scheme; configuration follows as a
`Map<String, String>` whose keys match the service's configuration keys:

```java
import java.util.HashMap;
import java.util.Map;
import org.apache.opendal.Operator;

Map<String, String> conf = new HashMap<>();
conf.put("bucket", "my-bucket");
conf.put("region", "us-east-1");
conf.put("endpoint", "https://s3.amazonaws.com");

Operator op = Operator.of("s3", conf);
```

A local filesystem operator roots every path under one directory:

```java
Map<String, String> conf = new HashMap<>();
conf.put("root", "/tmp/opendal");

Operator op = Operator.of("fs", conf);
```

### From a typed config builder

`ServiceConfig` exposes a typed builder per service, so configuration keys are
checked at compile time. Pass the built config to `Operator.of`:

```java
import org.apache.opendal.Operator;
import org.apache.opendal.ServiceConfig;

Operator op = Operator.of(
    ServiceConfig.S3.builder()
        .bucket("my-bucket")
        .region("us-east-1")
        .endpoint("https://s3.amazonaws.com")
        .build());
```

Use `AsyncOperator.of(...)` with the same arguments for the async API.

## Credentials

Credentials are just more configuration keys. Set them in the map (or on the
typed builder) — for example S3's `access_key_id` and `secret_access_key`:

```java
Map<String, String> conf = new HashMap<>();
conf.put("bucket", "my-bucket");
conf.put("region", "us-east-1");
conf.put("access_key_id", System.getenv("AWS_ACCESS_KEY_ID"));
conf.put("secret_access_key", System.getenv("AWS_SECRET_ACCESS_KEY"));

Operator op = Operator.of("s3", conf);
```

Some services also load credentials from their platform's default sources (for
S3, the standard AWS environment variables and profiles). See each service's
page under [Services](/services) for the exact keys and credential behavior.

Avoid hard-coding secrets in source. Read them from the environment or a secret
manager and pass them in when you build the operator.

## One operator per service and root

An operator maps to one service and one root path. To work with two buckets or
two roots, build two operators. Each operator owns a native resource, so close
it when you are done — a try-with-resources block does this for you, or call
`close()` explicitly.
</content>
