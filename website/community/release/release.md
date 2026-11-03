---
title: Create a release
sidebar_position: 3
---

This document mainly introduces how the release manager
releases a new version of Apache OpenDAL™ in accordance with the Apache requirements.

See [Reproducible source archives](./reproducible-source.md) for rebuilding and
comparing the source packages before signing.

For candidates prepared on `releases/<version>-rc.N`, follow the
[weekly release lifecycle](weekly.md): start the vote with ATR automatic resolution
and publication enabled, then monitor publication and review the version-sync PR.
The manual steps
below apply to candidates outside that lifecycle, including older weekly branches.

## Introduction

`Source Release` is the key point which Apache values, and is also necessary for an ASF release.

Please remember that publishing software has legal consequences.

This guide complements the foundation-wide policies and guides:

- [Release Policy](https://www.apache.org/legal/release-policy.html)
- [Release Distribution Policy](https://infra.apache.org/release-distribution)
- [Release Creation Process](https://infra.apache.org/release-publishing.html)

## Release terminology

The release process uses the following terms:

- `release_version`: the final version proposed for release, like `0.46.0`.
- `release_candidate_version`: the exact candidate used during staging and voting, formed from `${release_version}-${rc_version}`, like `0.46.0-rc.1`.
- `opendal_last_version`: the most recent final version, used as the starting point for the release comparison.
- `rc_version`: the suffix that identifies the release candidate and voting round, like `rc.1`.
- `maven_artifact_number`: the number of the Maven staging repository, like `1010`. Find it by searching for "opendal" in the [Apache Nexus staging repositories](https://repository.apache.org/#stagingRepositories). GitHub Actions creates the staged Maven artifacts when the release candidate tag is pushed.

## Preparation

:::caution

This section is the requirements for individuals who are new to the role of release manager.

:::

Refer to [Setup GPG Key](reference/setup_gpg.md) to make sure the GPG key has been set up.

## Start discussion about the next release

Start a discussion at [OpenDAL Discussion General](https://github.com/apache/opendal/discussions/categories/general):

Title:

```
[DISCUSS] Release Apache OpenDAL ${release_version}
```

Content:

```
Hello, Apache OpenDAL community,

I would like to start a discussion about releasing Apache OpenDAL ${release_version}.

Changes since Apache OpenDAL ${opendal_last_version}:

https://github.com/apache/opendal/compare/v${opendal_last_version}...main

Please share any comments on this release plan. After the discussion, we will
update the repository versions and begin preparing the release candidate.

Thanks,
${name}
```

## Start a tracking issue about the next release

Start a [tracking issue on GitHub](https://github.com/apache/opendal/issues/new?template=3-new-release.md) for the upcoming release to track all tasks that need to be completed.

## Release List

Weekly releases calculate versions from the optional breaking change declarations
in merged PRs; see [Declaring breaking changes](weekly.md#declaring-breaking-changes).
Compatible PRs need no additional metadata. The candidate Discussion includes the
frozen version plan and migration instructions.

For a manual release, update the version list in `dev/src/release/package.rs`.

This file is the source of truth for the split source release layout. Each entry in this list produces an independent source archive named `apache-opendal-{package}-{version}-src.tar.gz`.

For example:

- If there is any breaking change, please bump the `minor` version instead of the `patch` version.
- If this package is not ready for release, please skip it from the release list.
- Packages that have moved to separate repositories must not be included here.

## GitHub Side

### Bump version in project

Run `just update-version` to bump the version in the project.

Before updating any files, this command compares public compatibility dependencies
against the latest reachable final release tag. It rejects an integration target
that uses a patch bump after `opendal` or another configured public dependency
crosses a Cargo-incompatible version boundary. Fetch the repository tags before
running the command.

### Update docs

- Update `CHANGELOG.md`, refer to [Generate Release Note](reference/generate_release_note.md) for more information.
- Update `core/core/src/docs/upgrade.md` if there are breaking changes in `core`
- Make sure every released bindings' `upgrade.md` has been updated.
    - java: `bindings/java/upgrade.md`
    - node.js: `bindings/nodejs/upgrade.md`
    - python: `bindings/python/upgrade.md`

### Generate dependencies list

Download and setup `cargo-deny`. You can refer to [cargo-deny](https://embarkstudios.github.io/cargo-deny/cli/index.html).

Running `python3 ./scripts/dependencies.py generate` to update the dependency list of every package.

### Bootstrap Rust crate names

During release preparation, wait until the intended Rust crate-name set is on
`apache/opendal` `main`. The release manager chooses the exact reservation time,
normally about three days before the planned release. At that time, check out
the current `main` commit and run:

```shell
.agents/skills/opendal-release/scripts/bootstrap-rust-crates.sh
```

The helper dispatches the manual `Bootstrap Rust Crates` workflow without
inputs. The workflow scans the Rust publish plan from the `main` commit that
GitHub records as the run's `headSha`. The helper resolves that exact run,
checks the `headSha`, and waits for it to finish. A PMC member must approve the
protected `rust-bootstrap` environment on every run.

The protected job authenticates ownership and audits every existing planned
crate before it writes anything. Each established crate must already have
`apache/opendal`, `release_rust.yml`, and the `release` environment as its only
Trusted Publisher, with `trustpub_only` enabled. The workflow never modifies an
established crate. For each missing name, it publishes a dependency-free
`0.0.0` package, configures that Trusted Publisher, and enables
`trustpub_only`. Reruns reconcile every placeholder. The protected authenticated
audit runs even when there are no bootstrap candidates.

The one-time migration of existing crates is a separate administrative task.
Configure the same `apache/opendal`, `release_rust.yml`, and `release` publisher
identity and enable `trustpub_only` for every existing crate. This workflow
audits but never modifies established crates, and it fails if the migration is
incomplete. Complete that migration before using the OIDC-only Rust release
workflow.

The `0.0.0` package is a namespace reservation, not an ASF software release or
release artifact. crates.io exposes the reservation publicly, and publishing it
is irreversible. The release manager decides when the names are stable enough
to reserve. Run the helper again if a later `main` commit adds another planned
crate. Do not create the RC tag until every name in the current publish plan has
passed the workflow.

ASF Infrastructure provisions the `rust-bootstrap` GitHub environment from
`.asf.yaml` with PMC required reviewers, self-review disabled, and a `main`-only
deployment policy. A PMC release manager must add a
`CARGO_REGISTRY_BOOTSTRAP_TOKEN` environment secret. The crates.io token must
have only the `publish-new` and `trusted-publishing` endpoint scopes and
OpenDAL-specific crate scopes. The normal Rust release workflow must not have
access to this token.

### Push release candidate tag

After bump version PR gets merged, push the release candidate tag:

- Create a tag at `main` branch on the `Bump Version` / `Patch up version` commit: `git tag -s "v0.46.0-rc.1"`, please correctly check out the corresponding commit instead of directly tagging on the main branch.
- Push tags to GitHub: `git push --tags`.


:::note

Pushing an RC tag to GitHub will trigger the tag-based release workflows. In the current flow, this includes the Java staging artifacts on https://repository.apache.org and dry-run checks for other released packages.

:::

### Check the GitHub action status

After pushing the tag, check the GitHub action status to make sure the RC workflows finished successfully.

- Rust packages: [Release Rust Packages](https://github.com/apache/opendal/actions/workflows/release_rust.yml)
- Python: [Release Python Binding](https://github.com/apache/opendal/actions/workflows/release_python.yml)
- Java: [Bindings Java CI](https://github.com/apache/opendal/actions/workflows/ci_bindings_java.yml) and [Bindings Java Release](https://github.com/apache/opendal/actions/workflows/release_java.yml)
- Node.js: [Bindings Node.js CI](https://github.com/apache/opendal/actions/workflows/ci_bindings_nodejs.yml) and [Release NodeJS Binding](https://github.com/apache/opendal/actions/workflows/release_nodejs.yml)
- Docs: [Docs](https://github.com/apache/opendal/actions/workflows/docs.yml)

In the most cases, it would be great to rerun the failed workflow directly when you find some failures. But if a new code patch is needed to fix the failure, you should create a new release candidate tag, increase the rc number and push it to GitHub.

### Check Rust crates.io readiness

Before the official release tag is pushed, check the Rust package publish plan:

```shell
python3 .github/scripts/release_rust/plan.py
```

The plan must include every Rust package that is referenced by released crates.
For example, `opendal-testkit` is referenced by the `opendal` crate's `tests`
feature, so it must be publishable and appear before `opendal` in the plan.

The Rust release workflow uses `.github/scripts/release_rust/publish.py` instead
of calling `cargo publish` directly. The helper temporarily removes repo-local
`dev-dependencies` while packaging crates, because crates.io resolves
`dev-dependencies` even when `cargo publish --no-verify` is used. Without this
step, same-version dev dependencies and dev-only cycles can block the release
even though they are not needed by downstream users.

The normal workflow publishes only through Trusted Publishing. The helper
fetches and revokes a fresh OIDC-derived crates.io token for every publish
attempt, including attempts retried after a rate limit. New crate names must
already exist from the reservation workflow.

## ASF Side

If any step in the ASF Release process fails and requires code changes,
we will abandon that version and prepare for the next one.
Our release page will only display ASF releases instead of GitHub Releases.

Additionally, we should also drop the staging Maven artifacts on https://repository.apache.org.

### Create an ASF Release

After the RC tag has been pushed and the required workflows are green, create the ASF source release artifacts.

- Checkout to released tag. (e.g. `git checkout v0.46.0-rc.1`, tag is created in the previous step)
- Use the release script to create a new release: `just release`
  - This script will generate the release candidate artifacts under `dist` for every package listed in `dev/src/release/package.rs`, including:
    - `apache-opendal-{package}-{version}-src.tar.gz`
    - `apache-opendal-{package}-{version}-src.tar.gz.asc`
    - `apache-opendal-{package}-{version}-src.tar.gz.sha512`
  - Artifact names use each package's own version. The RC version is only used for the SVN directory name, such as `0.55.0-rc.1/`.
  - Each archive contains `LICENSE`, `NOTICE`, the package directory itself, and any repo-local dependencies needed to build that package from source.
  - This repository no longer produces a monolithic `apache-opendal-${release_version}-src.tar.gz` artifact or any `apache-opendal-bin-*` artifacts.

This script will create a new release under `dist`.

For example:

```shell
dist
├── apache-opendal-bindings-c-${c_version}-src.tar.gz
├── apache-opendal-bindings-c-${c_version}-src.tar.gz.asc
├── apache-opendal-bindings-c-${c_version}-src.tar.gz.sha512
├── apache-opendal-bindings-cpp-${cpp_version}-src.tar.gz
├── apache-opendal-bindings-cpp-${cpp_version}-src.tar.gz.asc
├── apache-opendal-bindings-cpp-${cpp_version}-src.tar.gz.sha512
├── apache-opendal-bindings-java-${java_version}-src.tar.gz
├── apache-opendal-bindings-java-${java_version}-src.tar.gz.asc
├── apache-opendal-bindings-java-${java_version}-src.tar.gz.sha512
├── apache-opendal-bindings-nodejs-${nodejs_version}-src.tar.gz
├── apache-opendal-bindings-nodejs-${nodejs_version}-src.tar.gz.asc
├── apache-opendal-bindings-nodejs-${nodejs_version}-src.tar.gz.sha512
├── apache-opendal-bindings-python-${python_version}-src.tar.gz
├── apache-opendal-bindings-python-${python_version}-src.tar.gz.asc
├── apache-opendal-bindings-python-${python_version}-src.tar.gz.sha512
├── apache-opendal-core-${core_version}-src.tar.gz
├── apache-opendal-core-${core_version}-src.tar.gz.asc
├── apache-opendal-core-${core_version}-src.tar.gz.sha512
├── apache-opendal-integrations-dav-server-${dav_server_version}-src.tar.gz
├── apache-opendal-integrations-dav-server-${dav_server_version}-src.tar.gz.asc
├── apache-opendal-integrations-dav-server-${dav_server_version}-src.tar.gz.sha512
├── apache-opendal-integrations-object_store-${object_store_version}-src.tar.gz
├── apache-opendal-integrations-object_store-${object_store_version}-src.tar.gz.asc
├── apache-opendal-integrations-object_store-${object_store_version}-src.tar.gz.sha512
├── apache-opendal-integrations-parquet-${parquet_version}-src.tar.gz
├── apache-opendal-integrations-parquet-${parquet_version}-src.tar.gz.asc
├── apache-opendal-integrations-parquet-${parquet_version}-src.tar.gz.sha512
├── apache-opendal-integrations-unftp-sbe-${unftp_sbe_version}-src.tar.gz
├── apache-opendal-integrations-unftp-sbe-${unftp_sbe_version}-src.tar.gz.asc
└── apache-opendal-integrations-unftp-sbe-${unftp_sbe_version}-src.tar.gz.sha512
```

### Upload artifacts to the SVN dist repo

:::info

SVN is required for this step.

:::

The svn repository of the dev branch is: <https://dist.apache.org/repos/dist/dev/opendal>

First, checkout OpenDAL to local directory:

```shell
# As this step will copy all the versions, it will take some time. If the network is broken, please use svn cleanup to delete the lock before re-execute it.
svn co https://dist.apache.org/repos/dist/dev/opendal opendal-dist-dev
```

Then, upload the artifacts:

> The `${release_candidate_version}` here should be like `0.46.0-rc.1`.

```shell
cd opendal-dist-dev
# create a directory named by version
mkdir ${release_candidate_version}
# copy source code and signature package to the versioned directory
cp ${repo_dir}/dist/* ${release_candidate_version}/
# check svn status
svn status
# add to svn
svn add ${release_candidate_version}
# check svn status
svn status
# commit to SVN remote server
svn commit -m "Prepare for ${release_candidate_version}"
```

Visit <https://dist.apache.org/repos/dist/dev/opendal/> to make sure the artifacts are uploaded correctly.

### Close the Nexus staging repo

To verify the Maven staging artifacts in the next step, close the Nexus staging repo as below.

1. Open https://repository.apache.org/#stagingRepositories with your Apache ID login.
2. Find the artifact `orgapacheopendal-${maven_artifact_number}`, click the "Close" button.

The `close` operation means that the artifacts are ready for voting.

:::caution

If the vote failed, click "Drop" to drop the staging Maven artifacts.

:::

### Rescue

If you publish incorrect or unexpected artifacts, such as invalid signatures or
checksums, abandon the current release candidate `${release_candidate_version}`,
increment `rc_version`, and prepare a new candidate. Delete the incorrect
artifacts from the SVN dist repository and drop the Maven staging repository at
https://repository.apache.org.

## Voting

A release candidate must receive at least three binding `+1` votes from OpenDAL
PMC members and more binding `+1` than `-1` votes.

Start a VOTE at [OpenDAL Discussion General](https://github.com/apache/opendal/discussions/categories/general):

Title:

```
[VOTE] Release Apache OpenDAL ${release_candidate_version}
```

Content:

````
Hello, Apache OpenDAL community,

This is a call for a vote on Apache OpenDAL release candidate
v${release_candidate_version}, proposed for release as Apache OpenDAL ${release_version}.

Release candidate source packages:

https://dist.apache.org/repos/dist/dev/opendal/${release_candidate_version}/

Keys used to verify the signatures:

https://downloads.apache.org/opendal/KEYS

Release candidate Git tag:

https://github.com/apache/opendal/releases/tag/v${release_candidate_version}

Maven staging repository:

https://repository.apache.org/content/repositories/orgapacheopendal-${maven_artifact_number}/

Python packages on TestPyPI:

https://test.pypi.org/project/opendal/

Staged website:

https://opendal-v${release_candidate_version | replace('.', '-')}.staged.apache.org/

Please download, verify, and test the release candidate. When voting, state
which checks you performed.

The vote will remain open for at least 72 hours and until the release candidate
receives at least three binding `+1` votes, with more binding `+1` than `-1`
votes.

```markdown
- [ ] +1 Approve.
- [ ] +0 No opinion.
- [ ] -1 Do not approve. Please explain why.

Verification checklist:

- [ ] Download links work.
- [ ] Checksums and signatures are valid.
- [ ] LICENSE and NOTICE files are present.
- [ ] Source packages contain no unexpected binary files.
- [ ] Source files include ASF license headers.
- [ ] Source builds successfully.
```

Use `verify.py` to help verify the release candidate:

```shell
svn checkout https://dist.apache.org/repos/dist/dev/opendal/${release_candidate_version}/ opendal-dist-${release_candidate_version}
cd opendal-dist-${release_candidate_version}
curl --silent --show-error --location https://github.com/apache/opendal/raw/v${release_candidate_version}/scripts/verify.py --output verify.py
python verify.py
```

For more information about Apache OpenDAL, visit https://opendal.apache.org/.

Thanks,
${name}
````

Example: <https://github.com/apache/opendal/discussions/5211>

The vote should remain open for **at least 72 hours**, except in the following cases:

1. Security issues
2. Urgent bug fixes that affect many users
3. Other emergencies approved by the PMC

The release manager must explain the emergency and shortened voting period in the vote message.

> The 72-hour minimum gives community members in different time zones an opportunity to verify the release candidate and vote.

After the release candidate receives at least three binding `+1` votes from [OpenDAL PMC members](https://people.apache.org/phonebook.html?project=opendal), with more binding `+1` than `-1` votes, announce the result:

Title:

```
[RESULT][VOTE] Release Apache OpenDAL ${release_candidate_version}
```

Content:

```
Hello, Apache OpenDAL community,

The vote on Apache OpenDAL release candidate v${release_candidate_version} has passed.
The candidate is approved for release as Apache OpenDAL ${release_version}.

Binding +1 votes:

- xxx
- yyy
- zzz

Non-binding +1 votes:

- aaa

+0 votes:

- None

-1 votes:

- None

Vote thread: ${vote_thread_url}

Thanks,
${name}
```

Identify voters by their real name, public profile name, or Apache ID. Avoid
nicknames that make votes difficult to verify. Confirm that every binding vote
comes from an OpenDAL PMC member.

Example: <https://lists.apache.org/thread/xk5myl10mztcfotn59oo59s4ckvojds6>

## Official Release

For the new weekly lifecycle, a passing ATR resolution automatically starts final
publication. See [After the vote passes](weekly.md#after-the-vote-passes) for final
refs, package jobs, Nexus promotion, announcements, main synchronization and RC
branch cleanup. Do not also execute the manual publication steps below for that
candidate.

For the manual path, the release manager initiates this phase after the vote
passes. Use the approved RC commit even if `main` has advanced.

For a candidate staged in ATR, complete source publication through ATR using the
approved revision. The SVN instructions below apply to candidates staged in
`dist/dev`; preserve the approved archives in either path.

### Push the release git tag

```shell
# Checkout the tags that passed VOTE
git checkout v${release_candidate_version}
# Tag with the final release version
git tag -s v${release_version}
# Push tags to GitHub to trigger releases
git push origin v${release_version}
```

Push the final tag with the release manager's credentials. This triggers the
existing tag workflows: Rust, Python, Node.js, Ruby and .NET take their formal
publication paths; Java stages to Nexus and requires the promotion below. Dart
builds artifacts. Check the resulting runs and package repositories before
claiming publication succeeded.

Tags pushed with `GITHUB_TOKEN` need explicit final-tag workflow dispatch.
`release_publish.yml` handles this for new weekly candidates; RC dispatch inputs
are not final-publication inputs. Creating the GitHub Release later is separate
from this trigger.

### Publish artifacts to SVN RELEASE branch

```shell
svn mv https://dist.apache.org/repos/dist/dev/opendal/${release_candidate_version} https://dist.apache.org/repos/dist/release/opendal/${release_version} -m "Release ${release_version}"
```

### Release Maven artifacts

1. Open https://repository.apache.org/#stagingRepositories.
2. Find the artifact `orgapacheopendal-${maven_artifact_number}`, click the "Release" button.

It will take some time to sync the Maven artifacts to the Maven Central.

:::caution

If the vote failed, click "Drop" to drop the staging Maven artifacts.

:::

### Check the language binding artifacts

We need to check the language binding artifacts in the language package repo to make sure they are released successfully.

- Rust: <https://crates.io/crates/opendal>
- Python: <https://pypi.org/project/opendal/>
- Java: <https://repository.apache.org/#nexus-search;quick~opendal>
- Node.js: <https://www.npmjs.com/package/opendal>

For Rust crates, check both the top-level `opendal` crate and split crates that
were added or changed in the release, such as `opendal-service-*`,
`opendal-layer-*`, and integration crates.

For Java binding, if we cannot find the latest version of artifacts in the repo,
we need to check the `orgapacheopendal-${maven_artifact_number}` artifact status in staging repo.

For non-Java bindings, if we cannot find the latest version of artifacts in the repo,
we need to check the GitHub action status.

### Create a GitHub Release

- Click [here](https://github.com/apache/opendal/releases/new) to create a new release.
- Pick the git tag of this release version from the dropdown menu.
- Use the existing final tag at the approved RC commit, even if main has advanced.
- Generate the release note by clicking the `Generate release notes` button.
- Add the release note from every component's `upgrade.md` if there are breaking changes before the content generated by GitHub. Check them carefully.
- Publish the release.

### Send the announcement

Post an announcement in [OpenDAL Discussion Announcements](https://github.com/apache/opendal/discussions/categories/announcements) and send the same content to `announce@apache.org`.

> Follow the [Committer Email](https://infra.apache.org/committer-email.html) guide to configure SMTP before sending to the announcement mailing list.

Summarize notable user-facing changes instead of copying the raw breaking-change list.

Title:

```
[ANNOUNCE] Release Apache OpenDAL ${release_version}
```

Content:

```
Hi all,

The Apache OpenDAL community is pleased to announce the release of
Apache OpenDAL ${release_version}.

Apache OpenDAL provides a unified data access layer for applications,
libraries, and data systems.

Notable changes in this release include:

1. xxxxx
2. yyyyyy
3. zzzzzz

Changelog: https://github.com/apache/opendal/releases/tag/v${release_version}

Download: https://opendal.apache.org/download

Website: https://opendal.apache.org/

Issue tracker: https://github.com/apache/opendal/issues

Mailing list: dev@opendal.apache.org

Thanks,
${name}
on behalf of the Apache OpenDAL community
```

Example: <https://lists.apache.org/thread/oy77n55brvk72tnlb2bjzfs9nz3cfd0s>

## Post release

After the official release out, you may perform a few post-actions.

### Remove the old releases

Remove the old releases if any. You only need the latest release there, and older releases are available through the Apache archive.

To clean up old releases, run:

```shell
# 1. Get the list of releases
svn ls https://dist.apache.org/repos/dist/release/opendal
# 2. Delete each release (except for the last one)
svn del -m "Archiving OpenDAL release X.Y.Z" https://dist.apache.org/repos/dist/release/opendal/X.Y.Z
```
