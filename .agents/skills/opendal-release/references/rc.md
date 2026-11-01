# Release Candidate

Repository paths and shell commands are relative to the repository root.

## Weekly ATR Preparation

Read `website/community/release/weekly.md` and the selected revision of
`.github/workflows/weekly_release.yml` before operating this path. Scheduled
preparation runs on Thursday at 00:00 UTC and uses that cutoff; manual dispatch
pins the selected commit.
The workflow prepares package versions, pushes `releases/<version>-rc.N` and its lightweight
RC tag with the first unused `rc.N` number for its version, and calls
`release-compose.yml` to build, sign and upload source archives. Signing accepts
both schedule and manual dispatch events. Disabled downstream workflows are
skipped and listed in the run summary.
Do not add a manual version PR or rebuild an already staged candidate by default.

Check that the dispatch implementation is on the revision actually running.
Merging a workflow fix does not change earlier runs or backfill their downstream
jobs. The weekly `builds` job dispatches the existing workflows automatically;
no human needs to trigger each build in a normal run. The `notify` job waits for
compose and dispatch, then posts a preparation notice. It does not wait for all
downstream results. `release_lifecycle.yml` separately observes ATR hourly and
calls `release_publish.yml` after a resolved passing vote. The notice includes the
verified revision's CLI commands and the announcement draft for RM review.

For ATR OIDC, register `.github/workflows/weekly_release.yml` as an allowed compose
caller alongside `.github/workflows/release-compose.yml`. Inspect the actual ATR
permission error and initiating actor before changing artifacts or source code.
After correcting external configuration, rerun failed jobs on the original run
when its candidate and completed outputs remain usable. Rerunning all jobs can
run preparation again and create another candidate.

## Manual RC Tagging

Before creating an RC tag:

- Resolve the remote that points to `apache/opendal`; do not assume it is named `origin`.
- Confirm the bump PR or required fix PR is merged.
- Confirm the tag target commit exactly.
- Confirm the Rust crate bootstrap workflow succeeded and no later change added a crate to the publish plan.
- Confirm no existing tag uses the intended RC version.

Tag and push:

```bash
apache_remote="$(git remote -v | awk '$2 ~ /github.com[:\/]apache\/opendal(\.git)?$/ && $3 == "(fetch)" { print $1; exit }')"
test -n "${apache_remote}" || {
  echo "cannot find a git remote for apache/opendal" >&2
  exit 1
}

git fetch "${apache_remote}" main --tags
git tag -s "v${release_version}" "${main_sha}" -m "v${release_version}"
git tag -v "v${release_version}"
git push "${apache_remote}" "v${release_version}"
```

If a new commit lands after an RC tag and before the release is final, do not move the tag. Continue with the existing RC unless the release manager explicitly wants that commit included or the existing RC artifacts, release gates, or vote fail. In those cases, create the next RC tag at the intended commit.

## Required CI Gate

After preparing the RC, inspect runs for its tag and verify their head SHA. Include both `push` and `workflow_dispatch` events; source compose success alone does not establish downstream success.

Default required gate:

- `Release Rust Packages`
- `Release Java Binding`
- `Bindings Java CI`
- `Release Python Binding`
- `Bindings Go CI`
- `Release NodeJS Binding`
- `Bindings NodeJS CI`
- `Docs`

If the release manager explicitly narrows or expands the gate, follow that instruction and state the gate in status updates.

Useful commands:

```bash
gh run list --repo apache/opendal --branch "v${release_version}" --limit 100 \
  --json name,status,conclusion,databaseId,url

gh run view "${run_id}" --repo apache/opendal --json status,conclusion,jobs

gh run view "${run_id}" --repo apache/opendal --log-failed
```

Rules:

- Rerun transient workflow failures when logs indicate network, GitHub, package registry, or runner flakiness.
- If code or workflow changes are needed, land a PR on `main`, then create the next RC.
- Python wheel matrix can be slow. Poll job status before treating long runtime as a real failure.
- The Go binding has no `release_go.yml` in this repository. Its RC gate in `apache/opendal` is `Bindings Go CI`; publishing is tag-driven and handled separately.
- Dotnet/NuGet RC publish failures are nonblocking only if Dotnet is outside the agreed gate. RC tags must not publish prerelease packages to NuGet.
- If `Docs` fails, inspect the failed jobs. A docs build failure is release-relevant; a tagged website/nightlies deploy failure caused only by rsync SSH key or secret issues can be recorded as follow-up if the release manager explicitly narrows the gate.

## Build ASF Source Artifacts

For weekly ATR candidates, reuse the signed artifacts from the successful compose
run and verify their candidate SHA, signatures, hashes and ATR revision. Source
composition and downstream builds run independently; both must satisfy the agreed
pre-vote gate. Do not rerun `just release` merely to obtain local copies.

For the manual source-build path, build after the required RC workflows are green.

```bash
git checkout "v${release_version}"
rm -rf dist
just release
find dist -maxdepth 1 -type f | sort
```

Verify:

- Artifacts exist for every package listed in `dev/src/release/package.rs`.
- Each package group has `.tar.gz`, `.tar.gz.asc`, and `.tar.gz.sha512`.
- Artifact filenames use package-specific versions, not necessarily `opendal_version`. For example, Java, Python, Node.js, C/C++, and integrations can all differ from each other and from core.
- There is no obsolete monolithic `apache-opendal-${opendal_version}-src.tar.gz` assumption.

## Upload To ASF SVN `dist/dev`

Use the user's normal SVN configuration. Do not use an isolated SVN config unless the release manager explicitly asks for it.

```bash
svn co https://dist.apache.org/repos/dist/dev/opendal /tmp/opendal-dist-dev-${release_version}
mkdir /tmp/opendal-dist-dev-${release_version}/${release_version}
cp dist/* /tmp/opendal-dist-dev-${release_version}/${release_version}/
svn add /tmp/opendal-dist-dev-${release_version}/${release_version}
svn status /tmp/opendal-dist-dev-${release_version}
svn commit /tmp/opendal-dist-dev-${release_version} -m "Prepare for ${release_version}"
svn ls https://dist.apache.org/repos/dist/dev/opendal/${release_version}/
```

Stop rules:

- If `svn commit` fails with `E215004 Authentication failed`, treat it as an auth blocker, not an artifact blocker.
- If `svn commit --force-interactive` hangs, do not keep retrying forced interactivity from Codex. It may not be able to access macOS Keychain prompts.
- Do not say "uploaded" until SVN returns a committed revision and `svn ls` confirms the remote RC directory.

## Close Java Nexus Staging

Find the staging repo id from the Java release workflow logs or Nexus UI. It has the form `orgapacheopendal-<number>`.

The Java workflow can deploy staging artifacts without closing them. A successful Java release workflow does not mean Maven is vote-ready.

Before vote, the staging repo must be closed and publicly exposed:

```bash
curl -sS -L -o /tmp/opendal-maven-index.html -w '%{http_code} %{url_effective}\n' \
  https://repository.apache.org/content/repositories/orgapacheopendal-${maven_artifact_number}/
```

Stop rules:

- Any `404` from the staging URL means the Maven artifacts are not vote-ready. Inspect the response body to distinguish an open or not-exposed staging repo from a wrong repo id, dropped repo, or missing repo.
- `404` with text like `staging: open` or `not exposed` means the repo exists but is not closed. Close it before voting.
- `Close` is pre-vote. `Release` is post-vote. Do not click or automate `Release` before the vote passes.
- If the vote fails, drop the staging repo.

Issue `apache/opendal#7435` tracks automating the RC pre-vote close step.
