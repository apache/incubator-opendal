# Apache OpenDAL SMB Service

`opendal-service-smb` accesses files and directories on SMB2 and SMB3 shares
through the pure Rust [`smb-rs`](https://github.com/afiffon/smb-rs) client.

## Use through `opendal`

Enable the `services-smb` feature:

```shell
cargo add opendal --features services-smb
```

Configure `opendal::services::Smb`, then pass it to `opendal::Operator::new`.
Operations require a Tokio runtime. The connection is established on the first
operation and shared by cloned operators.

## Use with `opendal-core`

Add the split crates directly:

```shell
cargo add opendal-core opendal-service-smb
```

```rust,no_run
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_smb::{register_smb_service, Smb};

fn build_operator() -> Result<Operator> {
    Operator::new(
        Smb::default()
            .endpoint("server.example.com:445")
            .share("documents")
            .root("/reports/")
            .user(r"DOMAIN\alice")
            .password("password"),
    )
}

fn register_for_uri() {
    register_smb_service(OperatorRegistry::get());
}
```

Registration is required for scheme-driven construction through
`Operator::from_uri` or `Operator::via_iter`. The facade registers SMB
automatically when `auto-register-services` is enabled.

## Capabilities

The service supports stat, ranged reads, empty and streaming writes,
conditional creation with `if_not_exists`, recursive directory creation,
non-recursive directory listing, deletion, file rename, and streamed file copy.
Copy and rename support `if_not_exists` for the destination. Writes, copies,
and renames create missing parent directories. Writes and copies overwrite
existing files in place; they are not atomic. Writes do not support append or
abort. Copy uses a bounded transfer buffer and does not cache whole files.

The service uses TCP transport and NTLM authentication. Signing and encryption
are negotiated with the server. SMB1, Kerberos, DFS referrals, SMB server-side
copy, and versioning are not supported.

## Behavior tests

Start the Samba fixture from the repository root:

```shell
docker compose -f fixtures/smb/docker-compose.yml up -d --wait
```

Run the behavior suite from `core/`:

```shell
OPENDAL_TEST=smb \
OPENDAL_SMB_ENDPOINT=127.0.0.1:1445 \
OPENDAL_SMB_SHARE=data \
OPENDAL_SMB_ROOT=/opendal/ \
OPENDAL_SMB_USER=opendal \
OPENDAL_SMB_PASSWORD=opendal \
cargo test --test behavior --features tests,services-smb
```

With the same `OPENDAL_SMB_*` environment variables, run the service's real
Samba tests for pagination, cancellation, authentication errors, and dropping
resources after runtime shutdown:

```shell
cargo test -p opendal-service-smb --test samba --locked -- --ignored
```

Stop the fixture after testing:

```shell
docker compose -f fixtures/smb/docker-compose.yml down -v
```

## License

Licensed under the Apache License, Version 2.0.
