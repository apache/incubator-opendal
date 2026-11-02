- Proposal Name: `s3_provider_presets`
- Start Date: 2026-07-24
- RFC PR: [apache/opendal#7948](https://github.com/apache/opendal/pull/7948)
- Tracking Issue: [apache/opendal#0000](https://github.com/apache/opendal/issues/0000)

# Summary

Add first-class `r2` and `minio` services to `opendal-service-s3`. Each service
has its own URI scheme, config type, and builder. Its config exposes only the
connection, location, and authentication fields supported by that provider.

The provider builder validates those fields, converts them into an internal
`S3Config`, and delegates requests to the existing S3 implementation. Existing
`s3` construction and behavior remain unchanged.

# Motivation

S3 wire compatibility does not imply AWS configuration compatibility. Today,
an R2 or MinIO user must construct `s3` and know which endpoint, region, and
AWS-oriented options are meaningful. The complete `S3Config` also advertises
settings such as AssumeRole, request payer, storage classes, and AWS encryption
options even when a provider does not support them.

Here, *preset* means predefined construction semantics, not a scheme alias or
AWS credential profile. It neither exposes `S3Config` wholesale nor stores
credentials.

# Guide-level explanation

Select Cloudflare R2 with an account ID:

```rust
let op = Operator::from_uri((
    "r2://data/root",
    [
        ("account_id", "example-account"),
        ("access_key_id", "example-access-key"),
        ("secret_access_key", "example-secret-key"),
    ],
))?;
```

The URI authority is the bucket and the path is the OpenDAL root. The preset
derives the documented
[`https://<ACCOUNT_ID>.r2.cloudflarestorage.com`](https://developers.cloudflare.com/r2/get-started/s3/)
endpoint and uses `auto` as the signing region. A jurisdiction may be supplied
with `account_id`:

```rust
let op = Operator::from_uri((
    "r2://data/root",
    [
        ("account_id", "example-account"),
        ("jurisdiction", "eu"),
    ],
))?;
```

An R2 user may supply `endpoint` instead of `account_id` for a proxy, gateway,
or test server. Supplying both is invalid, and `jurisdiction` is valid only
with `account_id`.

Select a MinIO deployment with its endpoint:

```rust
let op = Operator::from_uri((
    "minio://data/root",
    [
        ("endpoint", "http://127.0.0.1:9000"),
        ("access_key_id", "minioadmin"),
        ("secret_access_key", "minioadmin"),
    ],
))?;
```

MinIO has no universal endpoint, so `endpoint` is required. `region` is
optional and defaults to `auto`; deployments that require a configured region
can provide it explicitly.

Provider selection is explicit; OpenDAL does not infer it from a hostname.
Unsupported fields such as `role_arn` fail during construction. Users who need
the complete S3 configuration surface continue to use `s3`.

# Reference-level explanation

## Public API and registration

The `opendal-service-s3` crate exports `R2`, `R2Config`, `R2_SCHEME`, `Minio`,
`MinioConfig`, and `MINIO_SCHEME` alongside `S3`, `S3Config`, and `S3_SCHEME`.
Enabling `services-s3` registers all three schemes:

```rust,ignore
registry.register::<S3>(S3_SCHEME);
registry.register::<R2>(R2_SCHEME);
registry.register::<Minio>(MINIO_SCHEME);
```

Each provider implements the existing `Configurator` and `Builder` contracts.
`Operator::from_uri`, `Operator::via_iter`, and `Operator::from_config`
therefore work without changes to `OperatorRegistry`.

## Provider configuration

Provider config types are independent structs. They do not embed or flatten
`S3Config`.

`R2Config` exposes:

- `root`
- `bucket`
- `account_id`
- `jurisdiction`
- `endpoint`
- `access_key_id`
- `secret_access_key`
- `session_token`

`MinioConfig` exposes:

- `root`
- `bucket`
- `endpoint`
- `region`
- `access_key_id`
- `secret_access_key`
- `session_token`
- `skip_signature`

`bucket` is a required `String`, `skip_signature` is a `bool` that defaults to
`false`, and every other listed field is an `Option<String>`. Both configs are
non-exhaustive and use `#[serde(deny_unknown_fields)]`. Unknown or misspelled
options therefore fail rather than silently adopting S3 defaults. Custom
`Debug` implementations omit credentials and session tokens.

Direct credentials require both `access_key_id` and `secret_access_key`.
`session_token` requires that pair. Both
[R2](https://developers.cloudflare.com/r2/api/s3/temporary-credentials/) and
[MinIO](https://min.io/docs/minio/linux/developers/security-token-service.html)
support temporary credentials; the configs accept issued credentials but do
not expose credential-issuance settings. When direct credentials are absent,
the builders may load static credentials from the standard AWS environment
variables or shared credential files. They do not use SSO, web identity,
credential processes, ECS, EC2 metadata, or AssumeRole.
`MinioConfig::skip_signature` explicitly selects anonymous requests and cannot
be combined with direct credentials.

These allowlists are public contracts. Additions require documented provider
support and tests. S3-specific operation policy remains available through
`S3Config`, not provider configs.

## Service construction

Construction follows one path:

```text
provider config
-> provider validation and defaults
-> internal S3Config
-> shared S3 builder
-> S3 backend with provider scheme
```

`R2Config::from_uri` and `MinioConfig::from_uri` map the URI authority to
`bucket` and the path to `root`, then deserialize query or iterator options
into their own fields.

`R2Builder` requires exactly one of `account_id` and `endpoint`. With
`account_id`, it accepts an optional `eu` or `fedramp`
[jurisdiction](https://developers.cloudflare.com/r2/reference/data-location/)
and derives `https://<ACCOUNT_ID>.<JURISDICTION>.r2.cloudflarestorage.com`; it
omits the jurisdiction segment when no jurisdiction is set. It always sets the
internal signing region to `auto` and prevents AWS-specific metadata lookup
that is outside the R2 contract.

`MinioBuilder` requires `endpoint`, forwards an explicit `region`, or uses
`auto` when it is absent. It maps only its declared authentication fields.

Before delegation, each builder creates the restricted credential chain
described above and passes it to the shared builder. This prevents the default
AWS credential chain from reintroducing behavior omitted by the provider
contract.

Both builders create the shared S3 backend through a crate-private construction
path that accepts the selected scheme. The backend uses that scheme in
`ServiceInfo` and error context, so an R2 or MinIO operator identifies itself
correctly without an operation-forwarding wrapper.

The `S3Config` conversion remains private and does not widen provider
deserialization or public APIs.

## Error and validation contract

Invalid combinations return `ErrorKind::ConfigInvalid` before a request is
sent. Errors identify the provider and invalid field or field combination, but
never include credential values.

Provider validation includes:

- A non-empty bucket.
- The endpoint rules described above.
- Valid R2 jurisdiction values.
- Complete direct credential tuples.
- No credentials when MinIO anonymous mode is selected.

# Compatibility and migration

This proposal adds schemes and types without changing `s3`. OpenDAL never
reinterprets an existing S3 endpoint as a provider preset.

An application can migrate by changing `s3` to `r2` or `minio`, removing
AWS-only options, and supplying the provider's required fields. Applications
that depend on fields outside the provider contract keep using `s3` as an
explicit escape hatch. Older OpenDAL versions reject the new schemes as
unregistered instead of falling back to S3.

# Drawbacks

The dedicated configs duplicate a small number of S3 connection and
authentication fields. This duplication is intentional: it prevents shared
implementation details from defining provider APIs.

Provider schemes and config types also enlarge generated binding APIs.
Unit tests must cover field rejection, endpoint derivation, invalid credential
tuples, redacted debug output, and scheme identity. Existing R2 and MinIO
behavior fixtures must construct through the new schemes. MinIO deployments
can vary, so its initial contract remains conservative and requires an
endpoint.

# Rationale and alternatives

Registering `r2` or `minio` as aliases of `S3` would improve naming but would
retain the full AWS-oriented config surface and could not enforce
provider-specific requirements.

Embedding or flattening `S3Config` into provider configs would make every S3
field appear supported. A denylist would drift whenever `S3Config` grows. A
small allowlist makes provider support explicit.

Adding a provider discriminator to `S3Config` would accumulate provider
branches inside `S3Builder` and give typed construction a weaker contract.
Dedicated configs keep validation local while sharing the runtime.

Detecting providers from endpoints would fail for custom domains, gateways,
and compatible test servers. Explicit schemes keep construction deterministic.

Separate provider crates would add feature and release boundaries without a
separate protocol implementation. The provider types belong in
`opendal-service-s3` because their runtime is S3-compatible.

# Prior art

RFC-5444 introduced scheme-based `OperatorRegistry` construction and identified
R2 configuration presets as a future extension. This proposal makes those
presets first-class provider contracts over a shared S3 runtime.
