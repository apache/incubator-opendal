---
title: Connecting to your storage
sidebar_label: Connecting to your storage
description: Build an OpenDAL operator for any backend — typed builders, key-value config, URIs, credentials, and feature flags.
---

# Connecting to your storage

Switching backends is a configuration change, not a code change. This page
covers the ways to build an operator and the things that trip people up:
feature flags and credentials. For the configuration keys of a specific service,
see [Services](/services).

## Enable the service feature

Every service is behind a `services-*` Cargo feature. Enable the ones you use:

```toml
[dependencies]
opendal = { version = "0.57", features = ["services-s3", "services-gcs"] }
```

The in-memory service is always available; every other service needs its
`services-*` feature. Forgetting the feature is the most common reason a service
"doesn't exist" at compile time.

## Build an operator

### From a typed builder (recommended)

Each service has a builder with checked, discoverable options:

```rust
use opendal::services;
use opendal::Operator;

let op = Operator::new(
    services::S3::default()
        .bucket("my-bucket")
        .region("us-east-1")
        .endpoint("https://s3.amazonaws.com"),
)?;
```

A local filesystem operator roots every path under one directory:

```rust
let op = Operator::new(services::Fs::default().root("/tmp/opendal"))?;
```

### From a key-value map

When configuration comes from a file or environment, build from key-value pairs
instead of naming each setter. The keys are the same as the builder fields:

```rust
use std::collections::HashMap;
use opendal::services;
use opendal::Operator;

let cfg = HashMap::from([
    ("bucket".to_string(), "my-bucket".to_string()),
    ("region".to_string(), "us-east-1".to_string()),
]);

let op = Operator::from_iter::<services::S3>(cfg)?;
```

### From a URI

Registered services can be constructed from a URI, with the scheme selecting the
backend at runtime:

```rust
use opendal::Operator;

let op = Operator::from_uri("memory://")?;
```

Use `Operator::via_iter(scheme, pairs)` for the same runtime-selected
construction with explicit key-value config.

## Credentials

Credentials are just more configuration keys. Set them on the builder (for
example S3's `access_key_id` / `secret_access_key`), or supply them through the
key-value map above. Some services can also load credentials from their
platform's default sources — see each service's page under [Services](/services)
for the exact keys and credential behavior.

Avoid hard-coding secrets in source. Read them from the environment or a secret
manager and pass them in when you build the operator.

## One operator per service and root

An operator maps to one service and one root path. To work with two buckets or
two roots, build two operators — they are cheap, lightweight handles that are
safe to share across threads and tasks.
