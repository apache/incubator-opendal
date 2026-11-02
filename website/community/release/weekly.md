---
title: Weekly source releases
sidebar_position: 4
---

# Weekly source releases

The weekly workflow is scheduled each Thursday at 00:00 UTC. It prepares a release
branch from the Thursday 00:00 UTC main cutoff and calls `release-compose.yml` to
build, sign and upload the candidate to ATR. Manual dispatch runs the same flow
from the selected branch commit at the time of dispatch.
Preparation creates no version PR and requires no manual merge.

The release manager then follows the [release procedure](release.md) to verify
the candidate and start the vote with automatic resolution and publication
enabled in ATR. Hourly synchronization then finishes publication and opens the
version-sync PR after ATR records a passing result.

Starting preparation on Thursday leaves time for a Monday release. The 72-hour
vote period starts when the RM opens the vote, not when preparation starts.
Scheduled-run delays, a later vote start or insufficient votes can delay publication;
Monday is a target, not a forced publication deadline.

## Preparation

Scheduled runs select the latest recorded main push at or before the cutoff.
Push-run timestamps record arrival on main; commit dates do not. Keep that run
history until preparation finishes. The first week requires a recorded push before
the cutoff. Manual dispatch starts a fresh attempt without requiring earlier push
history. It uses the dispatch commit even if the branch advances while the run
is queued.

Weekly preparation reads the merged PRs between the last published source commit
and the fixed cutoff. It validates their optional breaking change declarations,
then calls `update-version --patch --breaking <package>` for each affected package.
Unmarked PRs require no release metadata and retain the normal patch policy.

Each package receives at least a patch increment from the latest published final
GitHub Release. Declared breaking changes require an incompatible increment:
minor for `0.x` packages, major for stable `1.x` and later packages (`0.0.x` follows
Cargo's patch compatibility boundary). Multiple breaking PRs increment a package
only once. Higher configured versions and new package versions are preserved.
Public dependency compatibility changes also raise affected integration versions;
a core breaking change does not automatically require an incompatible binding
version. Existing manifest and dependency update code applies the resulting plan.

The candidate commits `.release/plan.json`, containing the baseline, source SHA,
PR declarations and version decisions. Discussions and final release notes read
this snapshot, not live PR descriptions. Resuming downstream jobs keeps the
candidate unchanged; preparing another RC can collect corrected PR declarations.
The next weekly range starts at the published plan's source SHA, independently of
whether the version-sync PR has merged. For older releases without a plan, it
starts at the merge base of the final tag and cutoff. Draft and prerelease GitHub
Releases do not advance the baseline. Missing PR associations or malformed breaking
declarations stop preparation with the affected commit or PR identified.

## Declaring breaking changes

Compatible PRs leave the optional **Breaking changes** section empty. Breaking PRs
add the `breaking-changes` label and fill the section in the PR template:

```markdown
# Breaking changes

Affected packages: core, bindings/java

Migration:
- Replace `old_api()` with `new_api()`.
```

Use comma-separated package names from `dev/src/release/package.rs`. Include
bindings whose public behavior changes even when their source files do not.
Explain the migration or the available alternatives for removed functionality.
The **Breaking change declaration** CI check provides feedback on the label,
package names and migration text on PR changes and label/description edits. It
does not add a required merge check. Weekly preparation validates declarations
before choosing versions. Compatible PRs need not list packages or declare `none`.

Reviewers identify API, behavior, default-value and runtime requirement breaks.
This contract does not detect unmarked breaking changes automatically. Before the
first weekly run with this contract, review unreleased breaking PRs and add their
missing labels and migration declarations; older published PRs need no backfill.

## Candidate refs

Each attempt commits the version, dependency and changelog updates on
`releases/<version>-rc.N`, for example `releases/0.59.3-rc.1`. It atomically pushes
that branch and its lightweight `v0.59.3-rc.1` tag at the same commit, then passes
the SHA to compose. Compose signs the source archives; the RC tag is unsigned.
Preparation chooses the first unused positive RC number from the fetched tags,
including manual candidates. Retaining RC tags prevents number reuse after branch
cleanup. A conflicting push fails without replacing a tag. Correct preparation
failures before starting another run.

Preparing a new candidate does not cancel an existing vote or replace approved
artifacts. The RM resolves any existing vote or pending publication before starting
another vote. Abandoned branches and ATR drafts can be cleaned up separately.

## Configuration

Before the first candidate using this lifecycle:

- Reuse `GPG_SECRET_KEY` and `SOURCE_SIGNING_FINGERPRINT` for source archives and
  signed final tags. Source signing accepts schedule and manual dispatch events.
- Register `.github/workflows/weekly_release.yml` as an ATR compose caller, along
  with `.github/workflows/release-compose.yml` for direct compose runs.
- Register `.github/workflows/release_lifecycle.yml` and
  `.github/workflows/release_publish.yml` as ATR **finish** callers. Hourly runs
  call the publication workflow directly, preserving the caller identity for
  OIDC; they do not dispatch publication as `github-actions[bot]`.
- The schedule owner and manual dispatcher must have ASF-linked GitHub accounts
  with the required OpenDAL permissions. An RM should maintain the cron expression,
  which determines the scheduled actor.
- Existing `NEXUS_STAGE_DEPLOYER_USER` and `NEXUS_STAGE_DEPLOYER_PW` must allow
  promotion of OpenDAL's closed staging repositories, as well as staging uploads.
- Allow GitHub Actions to create pull requests and write the release refs and
  Discussions. Existing package publisher credentials and release environment
  rules continue to apply.

These are external repository/ATR/Nexus settings; merging workflow code does not
configure them. Never publish a release solely to test credentials.

## Builds and preparation notice

After pushing the RC tag, the workflow explicitly dispatches the existing tag
builds, binding checks and documentation workflow against that tag. GitHub does
not trigger downstream push workflows for tags created with `GITHUB_TOKEN`.
These dispatches retain the RC behavior: Java stages to Nexus, Python uses
TestPyPI, NodeJS performs a publish dry run, Ruby and .NET retain artifacts, and
Rust does not publish. Documentation retains RC staging without deploying to
nightlies. Disabled workflows are skipped and recorded in the run summary.
A retry skips workflows already dispatched for the candidate commit;
rerun failed downstream jobs from their own runs.

After ATR upload and dispatch succeed, `Release candidate: <RC>` becomes the
shared release entry point in General Discussions. It includes the exact commit,
ATR revision, checks, downloads, build links, RM actions, CLI commands and the
final announcement draft, package version decisions and breaking-change migration
instructions. Upload and dispatch do not assert that builds or ATR
checks passed. The RM independently verifies the candidate before opening voting.

## Vote and community notification

Use the [official ATR CLI](https://github.com/apache/tooling-releases-client/blob/main/RELEASE-PROCESS.md)
or ATR's browser controls. The candidate Discussion substitutes the real RC and
revision into these commands:

```bash
atr check status opendal 0.59.3-rc.1 00001
uv run --python 3.13 --with "apache-trusted-releases @ git+https://github.com/apache/tooling-releases-client" python - <<'PYTHON'
from atrclient import api
from atrclient.models.api import VoteStartArgs

task = api.vote_start(VoteStartArgs(
    project="opendal",
    version="0.59.3-rc.1",
    revision="00001",
    email_to="dev@opendal.apache.org",
    automatic_resolve_when_finished=True,
    automatic_publish_when_resolved=True,
    notify_when_finished=True,
)).task
print(task.model_dump_json(indent=2))
assert task.task_args["automatic_resolve_when_finished"] is True
assert task.task_args["automatic_publish_when_resolved"] is True
PYTHON
```

This runs the official ATR Python client with the same authentication configuration
as `atr`, and checks both automation flags in the returned task. The dedicated
`atr vote start --auto-publish` command only enables publication, not resolution;
the generic `atr api post` command sends strings instead of the required booleans.
If a flag check fails, inspect the task in ATR; do not repeat the start command.
To read the task again, use `atr api get /task/get/TASK_ID`, replacing `TASK_ID`
with the returned task ID. Do not repeat the start command on an active vote.

At the scheduled vote end, ATR checks its Trusted Vote ballots and automatically
resolves a passing vote, then publishes the approved source files. If the vote
does not pass at that check, ATR leaves it unresolved for manual follow-up. Later
ballots do not guarantee another automatic attempt. End notifications are enabled
so the release manager can follow up. GitHub and email discussion replies do not
count as ATR Trusted Vote ballots; follow ATR's instructions to cast a ballot.

For manual recovery, inspect the tally and resolve according to the result:

```bash
atr vote tabulate opendal 0.59.3-rc.1
atr vote resolve opendal 0.59.3-rc.1 passed
```

Use ATR's browser controls to cancel a vote or inspect the tally if the CLI is
unavailable. Agents use the same CLI after the RM authorizes the voting action;
credentials belong in the CLI's hidden prompt. The CLI is an interactive RM tool;
CI reads ATR's JSON API and uses its trusted-publisher announcement endpoint.

`release_lifecycle.yml` runs hourly at minute 17 (GitHub can delay scheduled runs).
It keeps the candidate Discussion's opening post unchanged and appends status
changes as comments, including one reminder per ATR vote round. Unchanged status
does not generate another comment. Follow the latest replies and ATR for current
progress. Failure notices link to the workflow run lists so hourly retries do not
change the notice solely because the run ID changed. Scheduled publication runs
appear under `release_lifecycle.yml`; manual runs appear under `release_publish.yml`.
GitHub subscribers receive comments through their notification
settings. The official vote remains in ATR and the dev mailing-list thread; the
Discussion does not create another ballot. To synchronize sooner:

```bash
gh workflow run release_lifecycle.yml --repo apache/opendal --ref main
```

GitHub synchronization never starts or resolves a vote; ATR owns those actions.
A failed or cancelled vote cannot trigger publication. Existing votes retain
their original settings: changing the command for future candidates does not
enable automatic resolution on a vote that has already started.

## Recovering an existing candidate

Inspect the weekly run's revision and jobs before assuming automatic dispatch is
available. Workflow changes must be present in the revision used by the run;
merging a fix does not backfill older candidates. List runs for the existing tag
without filtering to `push`, since automatic dispatch produces
`workflow_dispatch` events:

```bash
gh run list --repo apache/opendal --branch "v${release_candidate_version}" \
  --limit 100 --json name,event,headSha,status,conclusion,databaseId,url
```

Check each run's head SHA against the candidate commit. Rerun failed downstream
jobs from their existing runs. If a workflow has no run, dispatch it against the
same RC tag. For example, to recover a missing Python release build:

```bash
gh workflow run release_python.yml --repo apache/opendal \
  --ref "v${release_candidate_version}"
```

This RC path publishes to TestPyPI. Read the selected workflow at the RC tag
before dispatching: it must support `workflow_dispatch` there, not only on main.
Use the workflow list and inputs in the weekly `builds` job as the maintained
reference for other missing builds. In particular, NodeJS uses
`nodejs-publish=false` and `nodejs-publish-dry-run=false` (the RC tag enables its
dry run), and Docs uses the RC tag as `release_version` with
`deploy-nightlies=false`. An old tag without a required
input or dispatch trigger needs a separate recovery decision; do not move it.

A successful dispatch only confirms acceptance. Find the resulting run and follow
it to completion. A candidate notice does not replace checking all required
builds, registry staging and ATR checks.

For a compose permission failure, correct ATR's allowed caller or actor
configuration and rerun failed jobs on the original run when its completed
outputs remain usable. Reuse existing signed artifacts. Rerunning preparation or
starting a new weekly run creates a new candidate rather than recovering the old
one.

## After the vote passes

For new `releases/<version>-rc.N` candidates, the hourly workflow calls
`release_publish.yml` once ATR reports a resolved passing vote. It:

1. Verifies the RC branch and tag identify the same commit. Creates the retained
   `releases/<version>` branch and signed `v<version>` tag at that commit. Existing
   final refs must agree; they are never moved. ATR's OIDC `commit_hash` is not
   used because the upload workflow starts on main before generating the RC.
2. Creates the GitHub Release and asks ATR to send the announcement to
   `announce@apache.org`. ATR checks source publication and download propagation.
   Posts the reviewed announcement in GitHub Announcements, which the repository
   mirrors to dev@opendal.apache.org.
3. Opens a draft version-sync PR from the latest main. `update-version --baseline
   v<version> --sync` takes the higher of main and released versions per package,
   preserves new packages and development changes, and regenerates dependencies,
   lockfiles and the changelog entry. Review and merge complete the sync; the
   workflow does not merge the release branch or auto-merge the PR. Because
   `GITHUB_TOKEN` does not trigger PR CI, a maintainer closes and reopens the
   reviewed PR (or pushes an update) to start checks.
4. Independently follows optional language distributions: dispatches existing
   Rust, Python, Node.js, Ruby, .NET, Dart and Docs workflows at the final tag,
   skipping disabled workflows, and promotes the approved Java Nexus staging
   repository. Go belongs to its separate repository. Pending or failed package
   jobs do not block the GitHub Release, announcements or version-sync PR.
5. Reports optional distribution progress in the candidate Discussion. Retains
   RC branches while distribution needs follow-up so hourly discovery can resume
   it. Once complete, removes same-version RC branches; each deletion requires
   its RC tag to retain the commit. The approved branch is deleted last so partial
   cleanup remains discoverable. Final branch, final tag and RC tags stay.

Each hourly run checks external completion records and resumes unfinished work.
Optional package failures remain visible in their own Actions runs and in the
candidate notice; the source release is already published. Rerun failed package
jobs, then wait for the next hourly pass or resume explicitly:

```bash
gh workflow run release_publish.yml --repo apache/opendal --ref main \
  -f rc=0.59.3-rc.1
```

If the version-sync PR was closed without merging, reopen it before resuming.
If pushing its branch succeeded but PR creation failed, the retry reuses that
branch. A successful publication run can still mean it is waiting for packages;
inspect the candidate Discussion and downstream results for completion. Package
workflow success is the automatic gate; RMs can also verify registry propagation
using each package's version, as described in the release procedure.

This lifecycle discovers only the new branch namespace. Existing
`release-candidates/weekly-*` candidates, including 0.59.2, retain
the [manual official-release procedure](release.md#official-release). Do not rename
or migrate them to opt into automation. Source archives remain the approved ATR
revision throughout; the manual SVN procedure applies only to SVN-staged sources.
