- Proposal Name: `error_handle`
- Start Date: 2022-02-23
- RFC PR: [apache/opendal#44](https://github.com/apache/opendal/pull/44)
- Tracking Issue: [apache/opendal#43](https://github.com/apache/opendal/pull/43)

# Summary

Enhanced error handling for OpenDAL.

# Motivation

OpenDAL didn't handle errors correctly.

```rust
fn parse_unexpected_error<E>(_: SdkError<E>, path: &str) -> Error {
    Error::Unexpected(path.to_string())
}
```

Most time, we return a path that is meaningless for debugging.

There are two issues about this shortcoming:

- [error: Split ErrorKind and Context for error check easier](https://github.com/apache/opendal/issues/24)
- [Improvement: provides more information about the cause of DalTransportError](https://github.com/apache/opendal/issues/29)

First, we can't check `ErrorKind` quickly. We have to use `matches` for the help:

```rust
assert!(
    matches!(
        result.err().unwrap(),
        opendal::error::Error::ObjectNotExist(_)
    ),
);
```

Then, we didn't bring enough information for users to debug what happened inside OpenDAL.

So we must handle errors correctly, so that:

- We can check the `Kind` to know what error happened.
- We can read `context` to know more details.
- We can get the source of this error to know more details.

# Guide-level explanation

Now we are trying to get an object's metadata:

```rust
let meta = o.metadata().await;
```

Unfortunately, the `Object` does not exist, so we can check out what happened.

```rust
if let Err(e) = meta {
    if e.kind() == Kind::ObjectNotExist {
        // Handle this error
    }
}
```

It's possible that we don't care about other errors. It's OK to log it out:

```rust
if let Err(e) = meta {
    if e.kind() == Kind::ObjectNotExist {
        // Handle this error
    } else {
        error!("{e}");
    }
}
```

For a backend implementer, we can provide as much information as possible. For example, we can return `bucket is empty` to let the user know:

```rust
return Err(Error::Backend {
    kind: Kind::BackendConfigurationInvalid,
    context: HashMap::from([("bucket".to_string(), "".to_string())]),
    source: anyhow!("bucket is empty"),
});
```

Or, we can return an underlying error to let users figure out:

```rust
Error::Object {
    kind: Kind::Unexpected,
    op,
    path: path.to_string(),
    source: anyhow::Error::from(err),
}
```

So our application users will get enough information now:

```shell
Object { kind: ObjectNotExist, op: "stat", path: "/tmp/998e4dec-c84b-4164-a7a1-1f140654934f", source: No such file or directory (os error 2) }
```


# Reference-level explanation

We will split `Error` into `Error` and `Kind`.

`Kind` is an enum organized by different categories.

Every error will map to a kind, which will be in the error message.

```rust
pub enum Kind {
    #[error("backend not supported")]
    BackendNotSupported,
    #[error("backend configuration invalid")]
    BackendConfigurationInvalid,

    #[error("object not exist")]
    ObjectNotExist,
    #[error("object permission denied")]
    ObjectPermissionDenied,

    #[error("unexpected")]
    Unexpected,
}
```

In `Error`, we will have different struct to carry different contexts:

```rust
pub enum Error {
    #[error("{kind}: (context: {context:?}, source: {source})")]
    Backend {
        kind: Kind,
        context: HashMap<String, String>,
        source: anyhow::Error,
    },

    #[error("{kind}: (op: {op}, path: {path}, source: {source})")]
    Object {
        kind: Kind,
        op: &'static str,
        path: String,
        source: anyhow::Error,
    },

    #[error("unexpected: (source: {0})")]
    Unexpected(#[from] anyhow::Error),
}
```

Every one of them will carry a source: `anyhow::Error` so that users can get the complete picture of this error. We have implemented `Error::kind()`, other helper functions are possible, but they are out of this RFC's scope.

```rust
pub fn kind(&self) -> Kind {
    match self {
        Error::Backend { kind, .. } => *kind,
        Error::Object { kind, .. } => *kind,
        Error::Unexpected(_) => Kind::Unexpected,
    }
}
```

The implementer should do their best to carry as much context as possible. Such as, they should return `Error::Object` to carry the `op` and `path`, instead of just returns `Error::Unexpected(anyhow::Error::from(err))`.

```rust
Error::Object {
    kind: Kind::Unexpected,
    op,
    path: path.to_string(),
    source: anyhow::Error::from(err),
}
```

# Drawbacks

None

# Rationale and alternatives

## Why don't we implement `backtrace`?

`backtrace` is not stable yet, and `OpenDAL` must be compilable on stable Rust.

This proposal doesn't erase the possibility to add support once `backtrace` is stable.

# Prior art

None

# Unresolved questions

None

# Future possibilities

- `Backtrace` support.
