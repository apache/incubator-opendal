# Release Troubleshooting

Repository paths and shell commands are relative to the repository root.

## Common Failure Patterns

### An RC tag exists but release workflows did not start

A tag pushed with `GITHUB_TOKEN` does not trigger downstream push workflows.
Inspect the tag SHA, weekly run revision and `builds` job before diagnosing an
individual binding. `Release Python Binding` is dispatched through
`release_python.yml`; it is not implied by source compose success.

Use the recovery procedure in `website/community/release/weekly.md`. Query runs
without restricting the event to `push`, then dispatch only missing workflows at
the existing RC tag. Reuse an existing failed run for retries. Read the workflow
at that tag: adding `workflow_dispatch` on main does not add it to an old tag.
An unsupported old-tag workflow requires a separate recovery decision, not moving
the RC tag. A new weekly run prepares a new candidate and is not a backfill.

### Required CI is green except unrelated workflows

Use the agreed gate. For 0.56.0, the blocking gate was Rust / Java / Python / NodeJS. Dotnet RC NuGet publishing was outside that gate and was fixed separately.

### Python release looks stuck

The wheel matrix can take a long time, especially macOS Intel and Windows. Poll actual job status and wait for conclusion before rerunning.

### TestPyPI reports duplicate files

`uv publish` can fail with `400 File already exists` when an RC attempt already uploaded the same wheel to TestPyPI. Record the filename, verify whether the existing TestPyPI artifacts are usable, and get an explicit release-manager waiver before voting. Do not treat duplicate files as equivalent to a successful fresh upload.

### PyPI top-level JSON is stale

Do not rely only on `https://pypi.org/pypi/opendal/json` immediately after publishing. Check `https://pypi.org/pypi/opendal/${python_version}/json` and the simple index for the exact version.

### Java CI fails on crates.io index flakiness

Failures such as `no matching package named futures-channel found` from the crates.io sparse index are registry flakes. Rerun failed jobs before opening a code-fix PR.

### Docs deploy fails after docs build succeeds

If the build jobs pass but the tagged website deploy fails with SSH key or rsync errors such as `error in libcrypto` or `Permission denied (publickey)`, separate it from documentation correctness. It can be a release follow-up when the release manager explicitly agrees; otherwise fix the deploy secret and create a new RC if the agreed gate requires `Docs` success.

### Dotnet tries to publish on RC tags

RC tags should build/validate but not publish prerelease packages to NuGet. The durable fix is a prerelease guard in `.github/workflows/release_dotnet.yml`, as done in PR `#7433`.

### Rust publish fails on dev dependencies

Do not patch workflow YAML blindly. Use and test `.github/scripts/release_rust/publish.py`, which strips repo-local `dev-dependencies` during packaging and restores manifests afterward.

### `opendal-testkit` is missing from crates.io plan

Include `core/testkit` / `opendal-testkit` when `opendal` references it through the `tests` feature. Make it publishable and publish before top-level `opendal`.

### SVN authentication fails

Confirm whether the files are staged locally. If yes, the blocker is credentials. Do not claim upload until `svn commit` returns a revision. macOS Keychain credentials may be visible to `svn auth` but unavailable to the Codex process. If `--force-interactive` hangs, stop retrying it and switch to a shallow working copy plus explicit credential environment variables.

### Rust publish takes a long time

The final Rust publish can run for a long time and still be healthy because split crates publish in plan order. Inspect `.github/scripts/release_rust/plan.py`, watch the workflow logs, and verify crates progressively instead of assuming the job is stuck.

### Maven URL returns 404

Inspect the response body. If it says the staging repo exists but is `open` or `not exposed`, close the repo before voting. Do not treat Java workflow success as Maven vote-readiness.

### Vote has comments but may not have passed

Count binding votes explicitly and verify the 72-hour rule. Do not move artifacts to `dist/release` or release Maven artifacts until the result is formally posted.

### ATR vote passed but automatic publication is incomplete

Automatic discovery covers only `releases/<version>-rc.N`. Inspect
`release_lifecycle.yml`, `release_publish.yml`, the candidate Discussion and
actual downstream runs. The hourly scheduler can be delayed. Reuse the same
candidate and rerun failed package jobs rather than starting new runs.

For ATR finish OIDC failures, check both the allowed caller workflow and ASF-linked
actor. Register the hourly caller and manual publish workflow; do not dispatch
ATR publication as `github-actions[bot]`. For Nexus promotion permission errors,
verify the existing staging credentials have release permission. A GitHub Release
can exist before ATR confirms announcement; it alone does not prove completion.

For sync-PR creation failures, enable workflow PR creation and resume; the pushed
sync branch is reused. Reopen a closed, unmerged sync PR. Cleanup deletes the
approved RC branch last and preserves all RC tags; do not remove that recovery
entry point manually while publication is incomplete.
