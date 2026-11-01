# Configuration

- `root`: Set the working directory of the service.
- `bucket`: Set the Google Cloud Storage bucket name.
- `endpoint`: Set the gRPC endpoint. The default is `https://storage.googleapis.com`.
- `scope`: Set the OAuth 2.0 scope. The default is
  `https://www.googleapis.com/auth/devstorage.read_write`.
- `credential`: Set a base64-encoded service account credential.
- `credential_path`: Set the path to a service account credential file.
- `token`: Set an OAuth 2.0 access token.
- `skip_signature`: Send requests without authentication.
- `disable_vm_metadata`: Disable the GCE metadata credential provider.
- `disable_config_load`: Disable environment and well-known credential loading.

The URI form is `gcs-grpc://bucket/root`.

The endpoint must use `http` or `https`. Use an `http` endpoint only with a
trusted local emulator and `skip_signature`, because authenticated gRPC metadata
would otherwise be sent without TLS.
