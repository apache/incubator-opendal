# Rust Release Readiness

Repository paths and shell commands are relative to the repository root.

## Rust Release Readiness

Before official release, and preferably before vote, inspect the Rust publish plan:

```bash
python3 .github/scripts/release_rust/plan.py
```

Release safety requirements from the 0.56.0 cycle:

- `core/testkit` / `opendal-testkit` must be in the Rust publish plan when top-level `opendal` references it through the `tests` feature.
- Publish helpers must use `cargo publish --package <name>` rather than relying on workspace defaults.
- Repo-local `dev-dependencies` can break packaging even with `cargo publish --no-verify`; use `.github/scripts/release_rust/publish.py` and keep its tests green.
- The bootstrap workflow must succeed before RC tagging, and it must be rerun if the publish plan later gains a crate.
- The normal release workflow must use Trusted Publishing only. It fetches and revokes a new OIDC-derived crates.io token for every publish attempt, including retries.

Diagnose release helper CI failures from the failed jobs. Rebase only when the failure requires a newer base, and update code or dependencies only when the evidence calls for it. Run targeted helper tests for the affected behavior; reuse passing results while the relevant code, dependencies, and environment remain unchanged.
