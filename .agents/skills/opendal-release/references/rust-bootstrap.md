# Rust Crate Bootstrap

Repository paths and shell commands are relative to the repository root.

## Bootstrap Rust Crates

During release preparation, wait until the intended Rust crate-name set is
present on `apache/opendal` `main`. The release manager chooses the exact
reservation time, normally about three days before the planned release. When
the release manager decides to reserve the names, dispatch the Rust crate
bootstrap workflow:

```bash
.agents/skills/opendal-release/scripts/bootstrap-rust-crates.sh
```

This command performs the transition to `crates-bootstrapped`. Run it without
asking for a second confirmation after a PMC release manager explicitly chooses
the reservation time. Do not run it for release status checks or dry-run
planning. A `0.0.0` package is an externally visible and irreversible namespace
reservation, not an ASF software release or release artifact.

The helper:

- Requires a clean checkout at the current `apache/opendal` `main`.
- Verifies that the `rust-bootstrap` environment has a required-reviewer protection rule.
- Dispatches `bootstrap_rust_crates.yml` without inputs.
- Resolves the exact run, checks its `headSha`, and waits for completion.
- Blocks while the `rust-bootstrap` environment awaits PMC approval on every run.
- Verifies publicly that every crate in the checked publish plan exists and has `trustpub_only` enabled after the authenticated workflow audit succeeds.

The workflow always scans the publish plan from its `main` commit. Before any
write, the protected job uses the bootstrap token to authenticate ownership and
audit the exact Trusted Publisher configuration of every existing planned
crate. It never modifies an established crate. For missing names, it publishes
a dependency-free `0.0.0` namespace reservation, creates the single allowed
Trusted Publisher for `apache/opendal`, `release_rust.yml`, and the `release`
environment, and enables `trustpub_only`. Reruns reconcile every placeholder.
The protected authenticated audit runs even when discovery finds no bootstrap
candidates.

The one-time migration of existing, established crates to Trusted Publishing is
an independent administrative prerequisite. Configure the same
`apache/opendal`, `release_rust.yml`, and `release` publisher identity and
enable `trustpub_only` for every existing crate. This workflow audits but never
changes established crates, and it fails if that migration is incomplete.
Complete the migration before using the OIDC-only release workflow.

ASF Infrastructure provisions the `rust-bootstrap` GitHub environment from
`.asf.yaml` with PMC required reviewers, self-review disabled, and a `main`-only
deployment policy. A PMC release manager must add the
`CARGO_REGISTRY_BOOTSTRAP_TOKEN` environment secret. The token must have only
`publish-new` and `trusted-publishing` endpoint scopes, with crate scopes
restricted to OpenDAL package names. Never expose this token to the normal
release workflow.

If the Rust publish plan gains another crate after a successful reservation
run, run the helper again at the release manager's chosen time.
