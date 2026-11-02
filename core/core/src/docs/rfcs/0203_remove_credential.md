- Proposal Name: `remove_credential`
- Start Date: 2022-04-02
- RFC PR: [apache/opendal#203](https://github.com/apache/opendal/pull/203)
- Tracking Issue: [apache/opendal#203](https://github.com/apache/opendal/issues/203)

# Summary

Remove the concept of credential.

# Motivation

`Credential` intends to carry service credentials like `access_key_id` and `secret_access_key`. At OpenDAL, we designed a global `Credential` enum for services and users to use.

```rust
pub enum Credential {
    /// Plain refers to no credential has been provided, fallback to services'
    /// default logic.
    Plain,
    /// Basic refers to HTTP Basic Authentication.
    Basic { username: String, password: String },
    /// HMAC, also known as Access Key/Secret Key authentication.
    HMAC {
        access_key_id: String,
        secret_access_key: String,
    },
    /// Token refers to static API token.
    Token(String),
}
```

However, every service only supports one kind of `Credential` with different `Credential` load methods covered by [reqsign](https://github.com/Xuanwo/reqsign). As a result, only `HMAC` is used. Both users and services need to write the same logic again and again.

# Guide-level explanation

`Credential` will be removed, and the services builder will provide native credential representation directly.

For s3:

```rust
pub fn access_key_id(&mut self, v: &str) -> &mut Self {}
pub fn secret_access_key(&mut self, v: &str) -> &mut Self {}
```

For azblob:

```rust
pub fn account_name(&mut self, account_name: &str) -> &mut Self {}
pub fn account_key(&mut self, account_key: &str) -> &mut Self {}
```

All builders must implement `Debug` by hand and redact sensitive fields to avoid credentials being a leak.

```rust
impl Debug for Builder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Builder");

        ds.field("root", &self.root);
        ds.field("container", &self.container);
        ds.field("endpoint", &self.endpoint);

        if self.account_name.is_some() {
            ds.field("account_name", &"<redacted>");
        }
        if self.account_key.is_some() {
            ds.field("account_key", &"<redacted>");
        }

        ds.finish_non_exhaustive()
    }
}
```

# Reference-level explanation

Simple change without reference-level explanation needs.

# Drawbacks

API Breakage.

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
