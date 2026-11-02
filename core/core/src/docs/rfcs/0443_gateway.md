- Proposal Name: `gateway`
- Start Date: 2022-07-18
- RFC PR: [apache/opendal#443](https://github.com/apache/opendal/pull/443)
- Tracking Issue: [apache/opendal#444](https://github.com/apache/opendal/issues/444)

# Summary

Add Gateway for OpenDAL.

# Motivation

Our users want features like [S3 Proxy](https://github.com/gaul/s3proxy) and [minio Gateway](https://blog.min.io/deprecation-of-the-minio-gateway/) so that they can access all their data in the same way.

By providing a native gateway, we can empower users to access different storage in the same API.

# Guide-level explanation

OpenDAL will provide a new binary called: `oay`. It's a shortcut of `OpenDAL Gateway`.

Uses can install this binary via:

```shell
cargo install oay
```

Or using they favourite package management:

```shell
# Archlinux
pacman -S oay
# Debian / Ubuntu
apt install oay
# Rocky Linux / Fedora
dnf install oay
# macOS
brew install oay
```

With `oay`, users can:

- Serve `fs` backend with S3 compatible API.
- Serve `s3` backend with Azblob API
- Serve as a s3 signing services

# Reference-level explanation

`oay` will be a separate crate apart from `opendal` so we will not pollute the dependencies of `opendal`. But `oay` will be releases at the same time with the same version of `opendal`. That means `oay` will always use the same (latest) version of opendal.

Most operations of `oay` should be trivial, we will propose new RFCs if requiring big changes.

`oay` won't keep configuration. All config will go through environment.

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

- [S3 Proxy](https://github.com/gaul/s3proxy)
- [minio Gateway](https://blog.min.io/deprecation-of-the-minio-gateway/)
- [oxyno-zeta/s3-proxy](https://github.com/oxyno-zeta/s3-proxy)

# Unresolved questions

None

# Future possibilities

None
