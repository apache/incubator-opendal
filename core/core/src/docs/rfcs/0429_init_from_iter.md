- Proposal Name: `init_from_iter`
- Start Date: 2022-07-10
- RFC PR: [apache/opendal#429](https://github.com/apache/opendal/pull/429)
- Tracking Issue: [apache/opendal#430](https://github.com/apache/opendal/issues/430)

# Summary

Allow initializing opendal operators from an iterator.

# Motivation

To init OpenDAL operators, users have to init an accessor first.

```rust
let root = &env::var("OPENDAL_S3_ROOT").unwrap_or_else(|_| "/".to_string());
let root = format!("/{}/{}", root, uuid::Uuid::new_v4());

let mut builder = opendal::services::s3::Backend::build();
builder.root(&root);
builder.bucket(&env::var("OPENDAL_S3_BUCKET").expect("OPENDAL_S3_BUCKET must set"));
builder.endpoint(&env::var("OPENDAL_S3_ENDPOINT").unwrap_or_default());
builder.access_key_id(&env::var("OPENDAL_S3_ACCESS_KEY_ID").unwrap_or_default());
builder.secret_access_key(&env::var("OPENDAL_S3_SECRET_ACCESS_KEY").unwrap_or_default());
builder
    .server_side_encryption(&env::var("OPENDAL_S3_SERVER_SIDE_ENCRYPTION").unwrap_or_default());
builder.server_side_encryption_customer_algorithm(
    &env::var("OPENDAL_S3_SERVER_SIDE_ENCRYPTION_CUSTOMER_ALGORITHM").unwrap_or_default(),
);
builder.server_side_encryption_customer_key(
    &env::var("OPENDAL_S3_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY").unwrap_or_default(),
);
builder.server_side_encryption_customer_key_md5(
    &env::var("OPENDAL_S3_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY_MD5").unwrap_or_default(),
);
builder.server_side_encryption_aws_kms_key_id(
    &env::var("OPENDAL_S3_SERVER_SIDE_ENCRYPTION_AWS_KMS_KEY_ID").unwrap_or_default(),
);
if env::var("OPENDAL_S3_ENABLE_VIRTUAL_HOST_STYLE").unwrap_or_default() == "on" {
    builder.enable_virtual_host_style();
}
Ok(Some(builder.finish().await?))
```

We can simplify this logic if opendal has its native `from_iter` support.

# Guide-level explanation

Users can init an operator like the following:

```rust
// OPENDAL_S3_BUCKET = <bucket>
// OPENDAL_S3_ENDPOINT = <endpoint>
let op = Operator::from_env(Scheme::S3)?;
```

Or from a prefixed env:

```rust
// OIL_PROFILE_<name>_S3_BUCKET = <bucket>
// OIL_PROFILE_<name>_S3_ENDPOINT = <endpoint>
let op = Operator::from_env(Scheme::S3, "OIL_PROFILE_<name>")?;
```

Also, we call the underlying function directly:

```rust
// var it: impl Iterator<Item=(String, String)>
let op = Operator::from_iter(Scheme::S3, it)?;
```

# Reference-level explanation

Internally, every service's backend will implement the following functions:

```rust
fn from_iter(it: impl Iterator<Item=(String, String)>) -> Backend {}
```

Note: it's not a public API of `Accessor`, and it will never be. Instead, we will use this function inside the crate to keep the ability to refactor or even remove it.

# Drawbacks

None.

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

## Connection string

It sounds a good idea to implement something like:

```rust
let op = Operator::open("s3://bucket?region=test")?
```

But there are no valid use cases. Let's implement this in the future if needed.
