---
title: Getting started
sidebar_label: Getting started
description: Run your first OpenDAL program in Java, then point it at a real storage backend.
---

# Getting started

## Your first program

This program builds an operator, writes a file, reads it back, inspects its
metadata, and deletes it. It runs against the in-memory service with no
credentials, so you can run it right after adding the dependency.

The operator holds a native resource, so close it when you are done — a
try-with-resources block does this for you:

```java file=bindings/java/src/test/java/org/apache/opendal/examples/Main.java region=quickstart
```

`write` accepts a `String` or `byte[]`; `read` returns `byte[]`.

## Point it at a real backend

Only the service changes; the operations stay identical. The first argument is
the scheme, and configuration is a `Map<String, String>`:

```java
import java.util.HashMap;
import java.util.Map;
import org.apache.opendal.Operator;

Map<String, String> conf = new HashMap<>();
conf.put("bucket", "my-bucket");
conf.put("region", "us-east-1");

try (Operator op = Operator.of("s3", conf)) {
    op.write("hello.txt", "Hello from S3!");
    System.out.println(new String(op.read("hello.txt")));
}
```

The native library bundles every service, so there is nothing extra to install.
The next page, [Connecting to your storage](./03-connecting.md), covers typed
config builders and credentials in depth; [Services](/services) lists every
backend and its keys.

## Sync vs async {#sync-vs-async}

`Operator` runs every call synchronously. For non-blocking code, use
[`AsyncOperator`]: it has the same verbs, but each returns a
`CompletableFuture`:

```java
import java.util.HashMap;
import java.util.Map;
import org.apache.opendal.AsyncOperator;

Map<String, String> conf = new HashMap<>();
conf.put("bucket", "my-bucket");
conf.put("region", "us-east-1");

try (AsyncOperator op = AsyncOperator.of("s3", conf)) {
    op.write("hello.txt", "Hello, World!")
        .thenCompose(v -> op.read("hello.txt"))
        .thenAccept(data -> System.out.println(new String(data)))
        .join();
}
```

Call `op.blocking()` on an `AsyncOperator` to get a synchronous `Operator`
backed by the same configuration.

[`AsyncOperator`]: https://opendal.apache.org/docs/java/org/apache/opendal/AsyncOperator.html
</content>
