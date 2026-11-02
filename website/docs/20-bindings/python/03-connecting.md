---
title: Connecting to your storage
sidebar_label: Connecting to your storage
description: Build an OpenDAL operator in Python for any backend — schemes, configuration, and credentials.
---

# Connecting to your storage

Switching backends is a configuration change, not a code change. For the
configuration keys of a specific service, see [Services](/services).

Unlike the Rust core, the Python wheel includes every service — there are no
build flags. Once `opendal` is installed, every backend is available.

## Build an operator

The first argument is the service scheme; configuration follows as keyword
arguments:

```python
import opendal

op = opendal.Operator(
    "s3",
    bucket="my-bucket",
    region="us-east-1",
    endpoint="https://s3.amazonaws.com",
)
```

A local filesystem operator roots every path under one directory:

```python
op = opendal.Operator("fs", root="/tmp/opendal")
```

Use `opendal.AsyncOperator` with the same arguments for the async API.

## Credentials

Credentials are just more configuration keys. Set them as keyword arguments — for
example S3's `access_key_id` and `secret_access_key`:

```python
op = opendal.Operator(
    "s3",
    bucket="my-bucket",
    region="us-east-1",
    access_key_id="...",
    secret_access_key="...",
)
```

Some services also load credentials from their platform's default sources (for
S3, the standard AWS environment variables and profiles). See each service's
page under [Services](/services) for the exact keys and credential behavior.

Avoid hard-coding secrets in source. Read them from the environment or a secret
manager and pass them in when you build the operator:

```python
import os
import opendal

op = opendal.Operator(
    "s3",
    bucket="my-bucket",
    region="us-east-1",
    access_key_id=os.environ["AWS_ACCESS_KEY_ID"],
    secret_access_key=os.environ["AWS_SECRET_ACCESS_KEY"],
)
```

## One operator per service and root

An operator maps to one service and one root path. To work with two buckets or
two roots, build two operators — they are lightweight and safe to share.
