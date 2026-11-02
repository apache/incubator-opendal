# Release Rust Plan

This directory contains the planning logic for the Rust crates.io release workflow.

## Why this script exists

After the repository split, Rust crates are no longer represented by a short hand-maintained list.
The release workflow needs to:

- discover all publishable Rust crates under `core/` and `integrations/`
- exclude crates with `publish = false`
- publish them in dependency order so local path dependencies are already available on crates.io
- package crates without repo-local dev-dependencies so same-version dev-dependency cycles don't block publishing

Keeping this logic in a standalone script makes it testable and keeps the workflow YAML readable.

## Planned crate roots

The planner scans:

- `core/Cargo.toml`
- `core/core/Cargo.toml`
- `core/testkit/Cargo.toml`
- `core/layers/*/Cargo.toml`
- `core/services/*/Cargo.toml`
- `integrations/*/Cargo.toml`

It reads local `dependencies`, `build-dependencies`, and target-specific variants of those tables
to build a dependency graph, then emits a deterministic topological order.

## Usage

Print the publish order as JSON:

```bash
python3 .github/scripts/release_rust/plan.py
```

Write the same JSON to GitHub Actions output as `packages=<json>`:

```bash
python3 .github/scripts/release_rust/plan.py --github-output
```

Publish the planned crates:

```bash
PACKAGES="$(python3 .github/scripts/release_rust/plan.py)" \
  python3 .github/scripts/release_rust/publish.py
```

Live publishing requires a GitHub Actions job with `id-token: write`. Dry runs
do not request a Trusted Publishing token.

`publish.py` wraps `cargo publish` with the release-specific behavior we need:

- retries crates.io rate limits by using the server-provided retry time
- fetches and revokes a fresh GitHub OIDC token for every publish attempt
- uses `cargo publish --package <name>` so workspace packages publish the intended crate
- temporarily removes repo-local `dev-dependencies` from the package manifest before publishing
- restores every touched manifest and lockfile after each package

## Bootstrap new crate names

`bootstrap.py` reserves new crate names and audits the complete Rust publish
plan. Its `discover` command reports missing names and existing `0.0.0`
placeholders without using credentials. Its `apply` command uses the protected
bootstrap token to:

- authenticate ownership and audit the exact Trusted Publisher of every
  existing planned crate before any write
- publish a dependency-free `0.0.0` namespace reservation for every missing name
- configure `apache/opendal`, `release_rust.yml`, and the `release` environment
  as a placeholder's only Trusted Publisher
- enable `trustpub_only` and reconcile partially completed placeholders
- perform a final authenticated audit of every planned crate

The `verify` command reads public crates.io metadata and confirms that all
planned crates exist and require Trusted Publishing. The authenticated `apply`
audit is authoritative for ownership and the exact publisher configuration.

Migration of existing crates is independent. `bootstrap.py apply` refuses to
modify an established crate. The `rust-bootstrap` protected job always runs,
including when the plan contains no missing names or placeholders.

Version `0.0.0` only reserves a crates.io namespace. It is not an ASF software
release or release artifact.

## Tests

Run the unit tests with:

```bash
python3 -m unittest discover -s .github/scripts/release_rust -p "test_*.py"
```
