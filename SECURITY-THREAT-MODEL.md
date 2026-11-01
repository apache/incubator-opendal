<!--
  Licensed to the Apache Software Foundation (ASF) under one
  or more contributor license agreements.  See the NOTICE file
  distributed with this work for additional information
  regarding copyright ownership.  The ASF licenses this file
  to you under the Apache License, Version 2.0 (the
  "License"); you may not use this file except in compliance
  with the License.  You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing,
  software distributed under the License is distributed on an
  "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
  KIND, either express or implied.  See the License for the
  specific language governing permissions and limitations
  under the License.
-->

# Apache OpenDAL Security Threat Model

## 1. Status

This document defines the security boundary for Apache OpenDAL. It is intended
for maintainers, security reporters, downstream users, and automated security
scanners that need to decide whether a report describes an OpenDAL
vulnerability or a responsibility of the embedding application, storage backend,
or deployment.

The canonical disclosure process remains
[`website/community/security.md`](website/community/security.md). Reports that
may affect OpenDAL security should be sent to `private@opendal.apache.org`
before public disclosure.

## 2. Purpose

OpenDAL is an in-process storage access library. It gives a host application a
uniform `Operator` API over many storage backends. It is not a daemon, gateway,
identity provider, authorization service, sandbox, or multi-tenant broker.

The purpose of this document is to help maintainers triage security reports by
answering four questions:

1. What is OpenDAL's security boundary?
2. What security responsibilities do applications that use OpenDAL have?
3. What security responsibilities do cloud and network storage providers have?
4. Which of the following categories apply to a security report: OpenDAL vulnerabilities,
hardening requests, user misuse, or out-of-scope deployment issues?

The most important premises are:

> OpenDAL trusts storage services for fulfilling storage capabilities defined per documentation.
> OpenDAL also trusts users to provide secure configuration and use OpenDAL as users' storage layer.

If a user intentionally uses OpenDAL to access an untrusted S3-compatible endpoint,
WebDAV server, FTP server, database, local path, or other backend, the user
is responsible for the consequences of configured service. OpenDAL does not
authenticate remote object contents, prove remote metadata correctness, or
protect the user from a malicious storage service that the user chose.

That premise does not remove OpenDAL's responsibility for OpenDAL library
boundary. OpenDAL must still preserve OpenDAL API contracts, isolate credentials, avoid leaking secrets in debug or observability output, and avoid
memory-safety violations in code reachable through OpenDAL public API.

## 3. System Model

An OpenDAL deployment has these participants:

| Participant | Role in this model |
| --- | --- |
| Host application | Trusted caller that creates `Builder`s, supplies config, applies `Layer`s, and decides which end-user input may reach OpenDAL. |
| OpenDAL | In-process library that normalizes public API inputs, builds service requests, signs requests when configured, parses service responses, and returns data or errors. |
| Storage backend | A trusted service that the host application use. e.g., An OpenDAL uses connects to S3, or a database uses OpenDAL communicates with Google Cloud. OpenDAL trusts a trusted service for backend-side authentication, object bytes, metadata, consistency, and durability. |
| End user of the host application | Out of OpenDAL's direct model. The host application must authenticate, authorize, and sanitize end-user requests before calling OpenDAL. |
| Network attacker | Out of scope unless the report shows OpenDAL weakened the configured transport. TLS and proxy policy are delegated to the HTTP client and host environment. |

OpenDAL doesn't use principal model. A `Operator` takes control of a backend service by user-provided credentials, root and other configurations. Every operation via an
`Operator` instance shares the same OpenDAL-level authority.
A typical interaction will be:

```
Trusted service <-> Backend <-> Operator <-> Host Application <-> End user
                 |                        |
                 |------------------------|
                    OpenDAL code surface
```

## 4. Security Boundary

OpenDAL's security boundary is the public library boundary plus the internal
state that OpenDAL owns behind that boundary.

The following properties are in scope for OpenDAL security.

### 4.1 Public API contract

OpenDAL must handle inputs accepted by the public API without memory-safety
violations, data races, panics across FFI boundaries, or inconsistent internal
state.

Examples:

- `Operator` path normalization must be deterministic and consistent with the
  documented path rules in `core/core/src/raw/path.rs`.
- `Operator::from_uri(...)` and `Operator::via_iter(...)` must build the service
  and root implied by trusted caller config.
- Public operation options such as ranges, conditional headers, metadata, and
  presign expiry must be encoded according to the selected service contract.

OpenDAL does not decide whether a host application's end user is allowed to use
a path. The host application owns that policy. However, once the host
application passes a path into OpenDAL, OpenDAL must not accidentally map it to
OpenDAL does not care whether a host application's end user has permissions for a path access. A host application decides what and how to interact with data and path with a configured service via an `Operator` instance. In another word, if 
a host application passes a path into OpenDAL, OpenDAL must not map it to
a different backend location than that backend's API contract describes. e.g. when passing `/path` to an `Fs` operator, OpenDAL should only access `/path`.

### 4.2 Local resource boundary

Remote storage services are trusted by configuration. Local resources touched
directly by OpenDAL are different because OpenDAL itself constructs host
filesystem paths, cache directories, temporary files, and local database paths.

For local services and layers, OpenDAL is responsible for the filesystem effects
created by its own path construction.

Examples:

- `services::Fs` documents `root` as the root under which operations happen.
  Reports showing that normalized OpenDAL paths can escape that configured root
  through OpenDAL's path construction are in scope.
- `services::Foyer` and `FoyerLayer` can use on-disk cache storage. Reports about
  OpenDAL writing cache data outside the configured cache location, leaking
  credentials into cache metadata, or corrupting unrelated local files are in
  scope.
- `atomic_write_dir` and temporary-write behavior are in scope when OpenDAL
  creates or renames files itself.

Symlink behavior for local filesystems should be documented explicitly by the
service. A report about symlink traversal should not be closed solely because
OpenDAL is a library; it should be triaged against the service's documented
local filesystem contract.

### 4.3 Credential isolation

OpenDAL must not leak credentials between independent operators, services, or layers within the same process.

Sharing OpenDAL operators across processes (e.g., via fork) while reusing configurations and credentials remains users' responsibility.

Examples:

- A request from `Operator` B must not be signed with credentials from
  `Operator` A.
- An OAuth token, temporary credential, signer context, or credential-provider
  cache must not be shared across operators unless the sharing is explicitly
  configured by the caller.
- `disable_config_load`, `disable_ec2_metadata`, and `disable_vm_metadata` must
  prevent the ambient credential sources they document.

### 4.4 Credential redaction and observability

OpenDAL must avoid exposing credentials through debug output, error chains, log
lines, tracing spans, and metrics labels.

Expected behavior:

- Builder `Debug` implementations redact credential fields.
- `LoggingLayer` may emit paths because paths are operation context, but it must
  not emit access keys, secret keys, bearer tokens, session tokens, private key
  material, or signer state.
- Metrics labels include service-level dimensions such as scheme, namespace,
  root, operation, error, status code, and service operation. They should not
  include credentials.
- Tracing and other observability layers follow the same credential-redaction
  rule.

Paths, namespace names, and roots can be sensitive in some deployments. OpenDAL
does not treat them as credentials by default; callers that need stronger
privacy must choose their observability layers and sinks accordingly.

### 4.5 Request construction and signing

OpenDAL is responsible for constructing requests that match the configured
service, endpoint, root, path, and operation.

Examples:

- `skip_signature = true` must only skip request signing. It must not silently
  change HTTPS to HTTP, disable endpoint validation, or weaken unrelated
  transport behavior.
- `allow_anonymous` is a deprecated compatibility alias only for the services
  that still expose it.
- A service must not send credentials to an endpoint other than the endpoint
  selected by the trusted caller config.

If the caller configures a malicious endpoint, sending requests to that endpoint
is caller responsibility. If OpenDAL ignores the caller's configured endpoint or
mixes it with another operator's credentials, that is in scope.

### 4.6 Response handling robustness

OpenDAL trusts the configured backend for semantic correctness: object bytes,
ETags, metadata, listing order, backend authentication and authorization decisions, and durability claims
belong to the backend.

OpenDAL is still responsible for handling backend responses without violating
its own library invariants.

Examples:

- XML and JSON parsers used by services must not cause memory-safety violations
  or uncontrolled panics reachable through the public API.
- Error parsing must not leak credentials.
- HTTP response body accounting must follow the implemented contract. For
  non-`HEAD` responses without `Content-Encoding`, OpenDAL records
  `Content-Length` and checks the consumed body length.

A malicious or compromised trusted backend returning false data is not by
itself an OpenDAL vulnerability. A parser bug in OpenDAL that turns a malformed
response into memory unsafety, credential disclosure, or process compromise may
be an OpenDAL vulnerability.

### 4.7 FFI and language bindings

Language bindings and FFI shims are in scope when they expose OpenDAL through a
documented API.

Examples:

- C ABI functions must uphold ownership and lifetime rules documented for that
  ABI.
- Binding-level crashes or use-after-free bugs reachable from safe binding APIs
  are in scope.
- Bare `unsafe` blocks are not findings by themselves; the finding must show a
  reachable violation of the documented API contract.

Each binding has its own version. A security property confirmed for Rust core
version `X` is not automatically confirmed for every binding release named `X`.

## 5. Out Of Scope

The following are not OpenDAL vulnerabilities by default.

### 5.1 Backend-side authorization and configuration

Storage-provider policy is owned by the backend and operator.

Examples:

- Public S3 bucket policy.
- HDFS permissions.
- POSIX file mode bits outside OpenDAL's own path construction.
- OAuth scopes granted to Google Drive, Dropbox, OneDrive, or similar services.
- Database user privileges.

### 5.2 Malicious storage services selected by the caller

The caller is responsible for choosing the storage service and endpoint.

Examples:

- A caller configures an attacker-controlled S3-compatible endpoint and the
  endpoint receives signed requests.
- A WebDAV, FTP, SFTP, HTTP, database, or SaaS endpoint lies about object
  contents, metadata, ETags, timestamps, or consistency.
- A backend returns data that fails an application-level integrity check.

OpenDAL does not provide cryptographic authentication of backend bytes.
Applications that need end-to-end integrity must add it above OpenDAL.

### 5.3 Host application authentication and authorization

OpenDAL does not authenticate end users, authorize per-user access, or decide
whether end-user input is allowed to become an OpenDAL path, endpoint, metadata
value, or body.

Examples:

- A web service forwards `../../secret` from an HTTP route into `Operator::read`
  without its own authorization or sanitization.
- A host application lets a tenant choose arbitrary `Builder::endpoint` values.
- A host application gives multiple tenants the same `Operator` over the same
  backend root.

These may be serious vulnerabilities in the host application, but they are not
OpenDAL vulnerabilities unless OpenDAL violates one of the boundaries in
section 4.

### 5.4 Backend correctness and data integrity

OpenDAL does not guarantee that a trusted backend is correct.

Examples:

- ETag is provider-defined and is not a MAC.
- Listing order and consistency are backend properties.
- `Content-Type`, `Content-Length`, timestamps, and user metadata are backend
  responses. OpenDAL may parse or check them for protocol handling, but does not
  turn them into cryptographic truth.

### 5.5 Resource exhaustion by default

OpenDAL provides streaming APIs and optional layers, but it is not a default
DoS shield.

Examples:

- A backend returns a very large object.
- A backend returns many listing pages.
- A caller uses `Operator::read` or `Operator::list` APIs that materialize data
  in memory instead of streaming APIs.
- The caller does not apply `TimeoutLayer`, `ConcurrentLimitLayer`, or
  `ThrottleLayer`.

Resource-exhaustion reports are OpenDAL vulnerabilities only when they show that
OpenDAL violates a documented bound or consumes resources independently of the
operation requested by the trusted caller and backend.

### 5.6 Transport policy selected outside OpenDAL

TLS certificate validation, CA bundles, proxies, and custom HTTP clients are
properties of the HTTP client and host environment. A report is in scope only
if an OpenDAL option silently weakens transport contrary to its documented
meaning.

### 5.7 Deployment shape of integrations

Integrations such as `dav-server` and `unftp-sbe` can expose an `Operator` over
a network listener. Authentication and authorization for that listener belong
to the host process and deployment.

Code-level bugs in the integration are in scope for that integration. A
deployment that exposes it without the intended authn/authz layer is not an
OpenDAL core vulnerability.

### 5.8 Supply chain, release, and project infrastructure

Dependency freshness, GitHub Actions hardening, release signing, branch
protection, and ASF infrastructure policy are important, but they are outside
this OpenDAL library threat boundary unless a separate project policy says
otherwise.

## 6. Component Scope

| Component family | Examples | Boundary notes |
| --- | --- | --- |
| Core API | `Operator`, `blocking::Operator`, `raw::Access`, operation options | Public API contract, path normalization, request construction, response handling, and memory safety are in scope. |
| Remote object stores | S3, GCS, Azure Blob, OSS, COS, OBS, B2, TOS, Swift, AzDLS, AzFile, LakeFS, Upyun, Vercel Blob | Backend contents and authz are trusted. OpenDAL credential handling, request signing, endpoint use, and parser robustness are in scope. |
| Remote protocols | HTTP, WebDAV, FTP, SFTP, WebHDFS, HDFS-family services | Remote service is trusted. Protocol implementation, credential handling, and parser robustness are in scope. |
| SaaS services | Google Drive, Dropbox, OneDrive, GitHub, Hugging Face, Aliyun Drive, pCloud, Koofr, Seafile, Yandex Disk | OAuth scope and remote correctness are backend/operator concerns. Token handling and redaction are in scope. |
| Database and KV services | MySQL, PostgreSQL, SQLite, Redis, MongoDB, SurrealDB, D1, Cloudflare KV, GridFS, etcd, Memcached, TiKV, FoundationDB, RocksDB, Redb, Persy, Sled, Cacache, GHAC, Vercel Artifacts | Backend authz and data correctness are trusted. Local file effects from embedded databases are in scope when OpenDAL config constructs paths. |
| Local filesystem services | `fs`, `compfs`, `monoiofs`, `opfs` | OpenDAL path construction and documented root behavior are in scope. Host-level permissions and caller authz are out of scope. |
| Cache services and layers | `foyer`, `FoyerLayer`, immutable-index layer | Cache directories, credential redaction, and local file effects are in scope when configured through OpenDAL. Cache hit correctness follows the layer contract. |
| Observability layers | logging, tracing, metrics, OpenTelemetry, Prometheus, dtrace, fastmetrics | Credential leakage is in scope. Paths and roots may be emitted as operational context unless the layer documents stronger privacy. |
| Control layers | retry, timeout, throttle, concurrent-limit, route, capability-check, chaos | Layer behavior must match its documented contract and must not cross-contaminate credentials or operator state. |
| Language bindings | C, C++, D, Dart, .NET, Go, Haskell, Java, Lua, Node.js, OCaml, PHP, Python, Ruby, Swift, Zig | Binding API safety, FFI ownership, and contract parity are in scope for the binding. |
| Examples, benches, fixtures, edge tests | `examples/`, `core/benches/`, `fixtures/`, `core/edge/` | Not production API. Bugs here are usually not OpenDAL vulnerabilities unless copied into shipped library behavior. |

## 7. Triage Dispositions

| Disposition | Use when |
| --- | --- |
| `VALID` | The report shows a violation of an in-scope boundary in section 4, reachable through documented OpenDAL APIs or shipped bindings. |
| `VALID-HARDENING` | There is no clear security-boundary violation, but OpenDAL's API makes dangerous misuse common enough that maintainers choose to harden behavior or documentation. |
| `OUT-OF-SCOPE: trusted-backend` | The report requires the configured storage backend to be malicious, compromised, or semantically wrong, and OpenDAL does not violate its own parser, credential, or memory-safety boundary. |
| `OUT-OF-SCOPE: caller-authz` | The report depends on the host application forwarding unauthorized end-user input into OpenDAL. |
| `OUT-OF-SCOPE: caller-config` | The report depends on a trusted caller choosing a malicious endpoint, credential, root, layer, or HTTP client. |
| `OUT-OF-SCOPE: deployment` | The report depends on exposing an integration or application without the deployment's intended authn/authz controls. |
| `OUT-OF-SCOPE: infrastructure` | The report is about release, CI, dependency, or ASF infrastructure policy rather than the OpenDAL library boundary. |
| `BY-DESIGN: property-not-provided` | The report asks OpenDAL to provide a property explicitly not provided here, such as backend-byte authentication or default resource caps. |
| `MODEL-GAP` | The report cannot be classified by this document. Treat this as evidence that the model needs revision. |

## 8. Examples

### 8.1 In scope

- `services::Fs` maps a normalized OpenDAL path outside the configured `root`
  through OpenDAL path construction.
- `Operator` B signs a request with credentials configured for `Operator` A.
- A builder `Debug` implementation prints a secret access key.
- A service-specific XML parser can trigger memory unsafety through a normal
  `Operator::list` call.
- A binding safe API can trigger use-after-free in the C ABI.
- `disable_config_load = true` still reads credentials from the ambient
  environment source it promised to disable.
- `skip_signature = true` silently downgrades HTTPS to HTTP.

### 8.2 Out of scope by default

- The caller configures an attacker-controlled S3-compatible endpoint and that
  endpoint receives signed requests.
- A storage service returns false object bytes or a misleading ETag.
- A host application forwards an untrusted tenant path to OpenDAL without its
  own authorization checks.
- A bucket policy allows public reads.
- A caller exposes `dav-server-opendalfs` without an authn layer.
- A caller reads a huge object with an API that materializes the whole object in
  memory.

### 8.3 Security and documentation improvement

- A common misuse is easy enough that many users repeat it.
- A service's local filesystem behavior is surprising but not clearly contrary
  to the documented contract.
- A layer emits non-credential operational context that can be sensitive in some
  deployments.

## 9. Maintainer Decisions

The following decisions are part of this model.

| Topic | Decision |
| --- | --- |
| Remote backends | OpenDAL trusts the remote backend selected by caller configuration. Malicious or incorrect remote backend behavior is out of scope unless OpenDAL violates its own parser, credential, memory-safety, or API boundary while handling the response. |
| `services::Fs` lexical root | OpenDAL must not map normalized operation paths outside the configured `root` through its own lexical path construction. |
| `services::Fs` symlinks | `services::Fs` follows normal host filesystem semantics. It is not a sandbox against symlinks that already exist under the configured `root`, unless a service-specific document later promises stronger confinement. |
| Canonical location | This file, `SECURITY-THREAT-MODEL.md`, is the repository-level threat model. The project security page may link to it, but the disclosure process remains there. |
| Version binding | A report should be triaged against the threat model version present in the affected release tag or maintenance branch. |
| Component drift | When services, layers, bindings, or local resource surfaces change, this file should be updated in the same pull request or a linked follow-up. |
| Observability privacy | Credentials are secrets and must be redacted. Paths, roots, and namespaces are operational context, not credentials by default, but deployments that treat them as sensitive should configure observability accordingly. |

## 10. Revision Triggers

We will revisit this threat model when any of the changes:

- A service or layer starts storing credentials outside process memory.
- A service or layer adds a new local filesystem write surface.
- A credential-loading option changes default behavior.
- A request-signing option changes semantics.
- A new binding or FFI surface is added.
- A new integration exposes an `Operator` through a protocol listener.
- A vulnerability report is classified as `MODEL-GAP`.
- The project decides to treat a previously caller-owned property as an
  OpenDAL-provided property.

## 11. Relationship To Existing Security Documentation

`website/community/security.md` currently defines the disclosure process:

- report concerns to `private@opendal.apache.org`;
- include the project name and reproduction details;
- report privately before public disclosure.

This threat model is additive. It does not replace the disclosure process. The
security page should link here as the canonical reference for "what counts as an
OpenDAL vulnerability".
