## Configuration

Use [`crate::SmbConfig`] for serializable configuration or the builder methods
for direct construction.

- `endpoint` is a server hostname or IP address with an optional port. The
  default port is 445.
- `share` names the SMB share.
- `root` selects a directory within the share and defaults to `/`.
- `user` is optional and supports `DOMAIN\user` and `user@domain` forms.
- `password` defaults to an empty password.

Paths use forward slashes. Backslashes, NUL characters, and `.` or `..`
components are rejected with [`opendal_core::ErrorKind::ConfigInvalid`].

## URI construction

The URI format is `smb://server[:port]/share[/root]`. Supply credentials as
URI user information or configuration options:

```rust,no_run
use opendal_core::{Operator, OperatorRegistry, Result};
use opendal_service_smb::register_smb_service;

fn build_operator() -> Result<Operator> {
    register_smb_service(OperatorRegistry::get());
    Operator::from_uri((
        "smb://server.example.com/documents/reports",
        [
            ("user", r"DOMAIN\alice"),
            ("password", "password"),
        ],
    ))
}
```

The URI authority sets `endpoint`, the first path component sets `share`, and
the remaining path sets `root`. A `root` option is preserved when the URI
specifies only the share. Explicit options take precedence over credentials
embedded in the URI.
