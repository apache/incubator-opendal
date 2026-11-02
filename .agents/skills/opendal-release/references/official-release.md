# Official Release and Follow-Up

Repository paths and shell commands are relative to the repository root.

## Automatic publication for new weekly candidates

Read `website/community/release/weekly.md` and `scripts/release_lifecycle.py` for
`releases/<version>-rc.N` candidates. Verify ATR reports a resolved passing vote;
never infer it from comments or elapsed time. Hourly synchronization calls the
publication workflow with the scheduler's identity. ATR finish permissions must
allow both the hourly caller and manual publication workflow, with an ASF-linked
actor. Existing Nexus credentials must allow promotion.

Final branch and signed tag use the RC branch SHA, cross-checked against its tag.
Ignore ATR `commit_hash` as the tag target: OIDC can record the main workflow SHA.
The workflow creates the GitHub Release, completes GitHub/ATR announcements and
opens a draft main version-sync PR before following optional package distribution.
Package or Nexus failures do not block the source release. It retains RC branches
for hourly distribution retries, then removes them once follow-up completes,
while retaining their tags and the final branch. Review and merge
that PR through normal repository checks; versions must never regress. A PR
created with `GITHUB_TOKEN` does not start CI: after review, close and reopen it
with the maintainer's credentials (or push an update) to trigger required checks.

Inspect the candidate Discussion and actual downstream results. A completed
publication run can mean it deferred while packages are still running. Recover
failed package jobs using their existing runs. Resume the lifecycle after fixing
configuration; do not dispatch another candidate or duplicate emails and Nexus
promotion. The same announcement draft is shown before voting and reused for
publication. A closed, unmerged sync PR blocks cleanup; reopen it to resume.

```bash
gh workflow run release_publish.yml --repo apache/opendal --ref main \
  -f rc="${release_version}"
```

## Manual official release

Use this path only for candidates outside the new branch namespace. After the
vote passes, the RM creates the final tag at the approved commit. A tag pushed
with the RM's credentials triggers existing final-tag workflows. A
`GITHUB_TOKEN` tag push needs explicit downstream dispatch. Java stages artifacts
on tag runs; promote the approved RC staging repository instead of replacing it
with a newly staged final-tag build.

For sources staged in ATR, follow the live ATR voting/publication path for the
approved candidate revision. The SVN commands below apply to SVN-staged sources;
do not rebuild approved archives to switch between the two paths.

After the vote passes:

1. Push final release tag:

```bash
apache_remote="$(git remote -v | awk '$2 ~ /github.com[:\/]apache\/opendal(\.git)?$/ && $3 == "(fetch)" { print $1; exit }')"
test -n "${apache_remote}" || {
  echo "cannot find a git remote for apache/opendal" >&2
  exit 1
}

git checkout "v${release_version}"
git tag -s "v${opendal_version}" -m "v${opendal_version}"
git tag -v "v${opendal_version}"
git push "${apache_remote}" "v${opendal_version}"
```

2. If Go is in scope, push the final Go binding tag and verify Go services as
   described in [Go Binding Release Readiness](go.md).

3. Move SVN artifacts from `dist/dev` to `dist/release`:

```bash
svn mv https://dist.apache.org/repos/dist/dev/opendal/${release_version} \
  https://dist.apache.org/repos/dist/release/opendal/${opendal_version} \
  -m "Release ${opendal_version}"
```

If remote `svn mv` or `svn mv --force-interactive` hangs on macOS Keychain
authentication, use a shallow working copy and commit a local move instead:

```bash
svn checkout --depth empty https://dist.apache.org/repos/dist /tmp/opendal-dist-root-${opendal_version}
cd /tmp/opendal-dist-root-${opendal_version}
svn update --set-depth empty dev release
svn update --set-depth empty dev/opendal release/opendal
svn update "dev/opendal/${release_version}"
svn move "dev/opendal/${release_version}" "release/opendal/${opendal_version}"
svn status
svn commit -m "Release ${opendal_version}"
svn ls https://dist.apache.org/repos/dist/release/opendal/${opendal_version}/
if svn ls https://dist.apache.org/repos/dist/dev/opendal/${release_version}/; then
  echo "dev RC directory still exists" >&2
  exit 1
fi
```

If the release manager provides SVN credentials, pass them through environment
variables and avoid caching or echoing them:

```bash
svn commit -m "Release ${opendal_version}" \
  --username "$SVN_USER" --password "$SVN_PASS" \
  --non-interactive --no-auth-cache
```

4. Release Maven artifacts in Nexus. The UI path is acceptable:
   - Open https://repository.apache.org/#stagingRepositories.
   - Find `orgapacheopendal-${maven_artifact_number}`.
   - Click `Release`.

The REST API path is also acceptable and easier to audit:

```bash
curl -fsSL -u "$NEXUS_USER:$NEXUS_PASS" \
  "https://repository.apache.org/service/local/staging/repository/orgapacheopendal-${maven_artifact_number}"

printf '{"data":{"stagedRepositoryIds":["orgapacheopendal-%s"],"description":"Release Apache OpenDAL %s"}}\n' \
  "$maven_artifact_number" "$opendal_version" \
  >/tmp/opendal-nexus-release.json

curl -fsSL -u "$NEXUS_USER:$NEXUS_PASS" \
  -H 'Content-Type: application/json' \
  -X POST \
  -d @/tmp/opendal-nexus-release.json \
  https://repository.apache.org/service/local/staging/bulk/promote \
  -w '\nHTTP %{http_code}\n'
```

After releasing, query the same staging repository API and confirm it reports
`released` before declaring Maven complete. Then verify Maven Central propagation
for the package-specific Java version.

5. Verify package repositories:
   - Rust: crates.io for every crate in `.github/scripts/release_rust/plan.py`, not only `opendal`.
   - Python: version-specific PyPI JSON or the simple index for the package-specific Python version.
   - Java: Maven Central or Nexus search for the package-specific Java version.
   - Node.js: npm for the package-specific Node.js version.
   - Go: `GOPROXY=direct go list -m -versions` for the binding module and service modules when Go is in scope.

For Rust, verify both top-level `opendal` and any split crates added or changed in the release. For bindings and integrations, verify the package-specific version from `dev/src/release/package.rs`, not `opendal_version`.

Use version-specific PyPI checks. The top-level PyPI JSON endpoint can lag or
temporarily report the previous version even after the version-specific endpoint
and simple index show the new release.

6. Create GitHub Release for `v${opendal_version}`:
   - Select the existing final tag at the approved RC commit.
   - Use `v${opendal_version}` as the release title.
   - Generate release notes.
   - Check the previous final tag and compare the listed PRs with that release
     range. Unlabeled PRs belong in the catch-all category; they must not vanish
     from the notes. Existing releases need an explicit notes update after a
     generation fix.
   - Prepend upgrade notes only for components with breaking changes.

7. Send announcement:
   - GitHub Discussions `Announcements`.
   - `announce@apache.org` from the committer email setup.
   - Use notable changes, not a raw breaking-change dump.
   - Verify the GitHub discussion URL and mail client send result before reporting completion.

## Post Release

After official release:

- Verify `https://dist.apache.org/repos/dist/release/opendal/${opendal_version}/`.
- Verify old release cleanup requirements.
- Verify website download page references the ASF release.
- Verify package repository propagation after enough sync time.
- Close or update the release tracking issue.
- Record failures and permanent fixes as PRs/issues, not just notes.

Old release cleanup:

```bash
svn ls https://dist.apache.org/repos/dist/release/opendal
svn del -m "Archiving OpenDAL release X.Y.Z" \
  https://dist.apache.org/repos/dist/release/opendal/X.Y.Z
```
