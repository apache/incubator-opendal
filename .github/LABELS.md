# Issue and PR labels

The Label workflow classifies the issue or PR that triggered it using Cloudflare
Workers AI (`@cf/qwen/qwen3-30b-a3b-fp8`). It runs when an issue or PR is opened,
edited or reopened, and when a PR receives new commits. It processes open items
only. There is no scheduled scan or historical backfill.

## Assignment policy

| Labels | Issues | PRs | Assignment |
| --- | --- | --- | --- |
| `bug`, `enhancement`, `documentation`, `research`, `release` | Yes | No | AI chooses one type; it may abstain when none applies. |
| `releases-note/feat`, `releases-note/fix`, `releases-note/refactor`, `releases-note/docs`, `releases-note/ci`, `releases-note/build`, `releases-note/chore` | No | Yes | AI chooses one release-note category. |
| `core`, `website`, `services/*`, `bindings/*`, `integrations/*` | Yes | Yes | AI selects directly affected components from the repository catalog. |
| `breaking-changes` | No | Yes | AI may add it for an explicit incompatible API or behavior change. |
| `size:*` | No | Yes | The script calculates changed lines. |

The category follows the substantive purpose. For example, a docstring-only
fix belongs to `releases-note/docs`, a routine dependency upgrade to
`releases-note/build`, and a package compilation regression to
`releases-note/fix`. The title prefix is evidence rather than an override.
Performance optimizations without new public functionality belong to
`releases-note/refactor`.

`research` covers technical investigations and unresolved design questions.
`release` covers release tracking issues. A usage question that does not fit a
type can receive component labels without a type.

Existing type/category labels are preserved, including maintainer corrections.
Component labels and `breaking-changes` are additive; the workflow does not
remove them when subsequent edits change the scope. Maintainers can correct
these labels. Size labels are recalculated on PR events and replace prior size
labels.

Maintainers control `lgtm`, `run-with-secrets`, `release-blocker`,
`good first issue`, `good first vibe`, and `help wanted`. The model cannot select
them. Language labels, `dependencies`, and `github_actions` remain under the
existing Dependabot/manual workflow and are preserved.

## Component catalog

`scripts/label.py` derives services from the generated service inventory in
`website/data/services.json`, and bindings/integrations from their repository
directories. Layers and HTTP transports use `core`.

Existing naming conventions are retained: service names use underscores in
labels, and the `hf` service uses `services/huggingface`. New component labels
are created when catalog inputs change on `main`. Synchronization only creates
missing labels; it does not rename, delete or overwrite existing labels.
The classifier never creates labels from model-generated names.

## Size labels

The size is additions plus deletions from GitHub's PR file list:

| Label | Changed lines |
| --- | --- |
| `size:XS` | 0–9 |
| `size:S` | 10–29 |
| `size:M` | 30–99 |
| `size:L` | 100–499 |
| `size:XL` | 500–999 |
| `size:XXL` | 1,000 or more |

The calculation excludes `*.lock`, `pnpm-lock.yaml`, `package-lock.json`,
`DEPENDENCIES.*`, `services.json`, and `generated.js`. Other generated files
still count; binary changes have the line counts reported by GitHub.

## Setup and operation

Configure these repository Actions settings before enabling classification:

- Variable `CLOUDFLARE_ACCOUNT_ID`: the account that provides Workers AI.
- Secret `CLOUDFLARE_API_TOKEN`: a token scoped to that account with Workers AI
  inference permission. Follow the Cloudflare
  [REST API setup](https://developers.cloudflare.com/workers-ai/get-started/rest-api/).

GitHub's workflow token receives `issues: write` and `pull-requests: write` for
label assignment, and `contents: read` for the trusted checkout. Fork PRs use
`pull_request_target`; the workflow always checks out the default branch and
never executes the PR's code. Titles, bodies and diff excerpts are sent to the
model as data. Model output is validated locally against separate issue/PR
categories and the component catalog before any label write.

The request contains at most 10,000 body characters and 6,000 characters of diff
excerpts, plus the file list. Classification is approximate; large diffs are not
fully analyzed. The workflow checks the current title, body and PR head again
before applying its result and skips a result if the item changed during
inference. These reads are not an atomic lock against concurrent edits.

Requests retry HTTP 429 and transient server errors up to three attempts.
Missing credentials, quota exhaustion, truncated output and invalid labels
fail the job without applying a classification. After fixing the cause, rerun
the failed job. Logs contain proposed label changes and model token/Neuron
usage. They do not publish model-generated explanations as comments.

Run local contract tests with:

```shell
python3 -m unittest discover -s scripts -p test_label.py
```

With `GH_TOKEN` and `GITHUB_REPOSITORY` configured, synchronize definitions with
`python3 scripts/label.py sync`. To inspect one saved Actions event, also set
`GITHUB_EVENT_PATH` and the Cloudflare credentials, then run
`python3 scripts/label.py event`. This defaults to a dry run; `--apply` writes
the selected labels. Neither command enumerates historical issues or PRs.
