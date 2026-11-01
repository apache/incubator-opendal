# Release Preparation

Repository paths and shell commands are relative to the repository root.

## Start A Release

1. Verify context:
   - Current repo is `apache/opendal`.
   - Current default branch and latest `main` SHA are known.
   - Existing release discussions, tracking issue, PRs, tags, and votes are identified.
   - Existing RC tags are listed with their target commits.

2. Consult the sources relevant to the preparation work. Reuse unchanged sources already read in this checkout:
   - `website/community/release/release.md`
   - `dev/src/release/package.rs`
   - `.github/workflows/release_*.yml`
   - `.github/workflows/bootstrap_rust_crates.yml`
   - `.github/scripts/release_rust/bootstrap.py`
   - `.github/scripts/release_rust/plan.py`
   - `.github/scripts/release_rust/publish.py`

3. Determine versions:
   - `opendal_version`: final release version, for example `0.56.0`.
   - `release_version`: RC version, for example `0.56.0-rc.4`.
   - Package-specific versions from `dev/src/release/package.rs`.
   - Existing RC numbers. Use the next RC number only when the previous RC is intentionally abandoned because artifacts or release gates failed, the vote failed, or the release manager explicitly wants a new commit included.

## Weekly Version Planning

Weekly preparation collects merged PRs since the last published source cutoff.
Only breaking PRs need the `breaking-changes` label, affected package names and
migration instructions in the optional PR-template section. Follow the format in
`website/community/release/weekly.md`; do not add release metadata to compatible
PRs or introduce per-PR files.

The existing version updater applies scoped incompatible increments and public
dependency propagation while preserving higher configured versions. Review the
candidate's `.release/plan.json` and Discussion for version and migration evidence.
Missing associations or malformed declarations fail with a commit or PR reference;
fix the declaration before preparing another candidate. Do not edit an existing
RC's plan or retag it. Before enabling the first run, check unreleased breaking PRs
for missing declarations; published history does not need migration.

## Bump And Release Notes

When preparing a bump PR:

- Run `just update-version` only after confirming the desired package versions.
- Update `CHANGELOG.md`.
- Update `core/core/src/docs/upgrade.md` only for core breaking changes.
- Update binding upgrade docs only for released bindings that have breaking changes:
  - `bindings/java/upgrade.md`
  - `bindings/nodejs/upgrade.md`
  - `bindings/python/upgrade.md`
- Do not add upgrade sections for bindings without breaking changes.
- Respect that every binding can have a different version.
- Regenerate dependency lists with `python3 ./scripts/dependencies.py generate` when the release docs require it.

Before opening the PR, check whether PR templates exist and use them. Keep the PR body self-contained and reviewer-facing.
