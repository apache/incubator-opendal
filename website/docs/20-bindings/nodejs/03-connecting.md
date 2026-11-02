---
title: Connecting to your storage
sidebar_label: Connecting to your storage
description: Build an OpenDAL operator in Node.js for any backend — schemes, configuration, URIs, and credentials.
---

# Connecting to your storage

Switching backends is a configuration change, not a code change. For the
configuration keys of a specific service, see [Services](/services).

Unlike the Rust core, the Node.js package includes every service — there are no
build flags. Once `opendal` is installed, every backend is available.

## Build an operator

The first argument is the service scheme; configuration follows as an options
object. Option keys are `snake_case`, matching each service's documented keys:

```javascript
import { Operator } from "opendal";

const op = new Operator("s3", {
  bucket: "my-bucket",
  region: "us-east-1",
  endpoint: "https://s3.amazonaws.com",
});
```

A local filesystem operator roots every path under one directory:

```javascript
const op = new Operator("fs", { root: "/tmp/opendal" });
```

## From a URI

You can also encode the scheme and configuration in a single URI. Extra options
override or supplement values from the URI:

```javascript
import { Operator } from "opendal";

const op = Operator.fromUri("memory://");
const op2 = Operator.fromUri("s3://my-bucket/", { region: "us-east-1" });
```

## Credentials

Credentials are just more configuration keys. Set them in the options object —
for example S3's `access_key_id` and `secret_access_key`:

```javascript
const op = new Operator("s3", {
  bucket: "my-bucket",
  region: "us-east-1",
  access_key_id: "...",
  secret_access_key: "...",
});
```

Some services also load credentials from their platform's default sources (for
S3, the standard AWS environment variables and profiles). See each service's
page under [Services](/services) for the exact keys and credential behavior.

Avoid hard-coding secrets in source. Read them from the environment or a secret
manager and pass them in when you build the operator:

```javascript
const op = new Operator("s3", {
  bucket: "my-bucket",
  region: "us-east-1",
  access_key_id: process.env.AWS_ACCESS_KEY_ID,
  secret_access_key: process.env.AWS_SECRET_ACCESS_KEY,
});
```

## One operator per service and root

An operator maps to one service and one root path. To work with two buckets or
two roots, build two operators — they are lightweight handles.
</content>
