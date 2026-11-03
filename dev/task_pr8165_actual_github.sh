#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS=4
task_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
task_base="${BASE_HEAD:?previous worktree head required}"
task_product=78a561237509451ab3a74851d3e8df0fad0f3a2c
task_log=/home/xuanwo/Code/logs/opendal-pr8165-original-78a56123-typed-github-20261103
[[ ! -e "$task_log" ]]
mkdir -p "$task_log"
cd "$task_repo_root"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
finish() {
    task_exit=$?
    trap - EXIT
    set +e
    unset OPENDAL_GITHUB_TOKEN
    git switch --detach "$task_base" > "$task_log/restore.log" 2>&1
    printf '%s\n' "$?" > "$task_log/restore.exit"
    git rev-parse HEAD HEAD^{tree} > "$task_log/restored-head.txt"
    git status --porcelain --untracked-files=all > "$task_log/restored-status.txt"
    printf '%s\n' "$task_exit" > "$task_log/driver.exit"
    exit "$task_exit"
}
[[ $(git rev-parse HEAD^) == "$task_product" ]]
[[ -z $(git status --porcelain --untracked-files=all) ]]
git rev-parse HEAD HEAD^{tree} > "$task_log/carrier-head.txt"
trap finish EXIT
[[ $(gh api user --jq .login) == Xuanwo ]]
[[ $(gh api repos/Xuanwo/opendal-pr-8165-fixture-20261103 --jq .id) == 1443743524 ]]
[[ $(gh api repos/Xuanwo/opendal-pr-8165-fixture-20261103 --jq .private) == true ]]
[[ $(gh api repos/Xuanwo/opendal-pr-8165-fixture-20261103 --jq .default_branch) == main ]]
git diff --exit-code "$task_product" HEAD -- core/services/github core/core core/tests/behavior/async_delete.rs > "$task_log/provider-source-identity.log"
git rev-parse "$task_product" "$task_product^{tree}" > "$task_log/provider-product.txt"
git rev-parse HEAD HEAD^{tree} > "$task_log/executed-head.txt"
export OPENDAL_TEST=github
export OPENDAL_GITHUB_OWNER=Xuanwo
export OPENDAL_GITHUB_REPO=opendal-pr-8165-fixture-20261103
export OPENDAL_GITHUB_ROOT=/pr8165-original-78a56123/
export OPENDAL_GITHUB_TOKEN
OPENDAL_GITHUB_TOKEN="$(gh auth token)"
[[ -n "$OPENDAL_GITHUB_TOKEN" ]]
export RUST_LOG=warn
cd core
cargo --version > "$task_log/toolchain.txt"
rustc --version >> "$task_log/toolchain.txt"
# GitHub requires Contents API writes and deletes to run sequentially.
cargo test --locked --features tests,services-github --test behavior -- test_delete_with_if_match_ --test-threads=1 --nocapture > "$task_log/canonical.log" 2>&1
rg -q 'test result: ok\. 3 passed; 0 failed' "$task_log/canonical.log"
cd "$task_repo_root"
git diff --exit-code > "$task_log/source-clean.log"
