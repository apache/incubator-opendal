#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS=4
task_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
task_base="${BASE_HEAD:?previous worktree head required}"
task_product=ad7316acdd0931a6a9f1edda000d81741ec82969
task_log=/home/xuanwo/Code/logs/opendal-pr8165-integrated-ad7316ac-20261103
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
    git rev-parse HEAD HEAD^{tree} > "$task_log/final-product-head.txt"
    git status --porcelain --untracked-files=all > "$task_log/final-product-status.txt"
    git switch --detach "$task_base" > "$task_log/restore.log" 2>&1
    printf '%s\n' "$?" > "$task_log/restore.exit"
    git rev-parse HEAD HEAD^{tree} > "$task_log/restored-head.txt"
    git status --porcelain --untracked-files=all > "$task_log/restored-status.txt"
    printf '%s\n' "$task_exit" > "$task_log/driver.exit"
    exit "$task_exit"
}
run_stage() {
    local task_name="$1"
    shift
    set +e
    "$@" > "$task_log/$task_name.log" 2>&1
    local task_rc=$?
    set -e
    printf '%s\n' "$task_rc" > "$task_log/$task_name.exit"
    return "$task_rc"
}
[[ $(git rev-parse HEAD^) == "$task_product" ]]
[[ -z $(git status --porcelain --untracked-files=all) ]]
git rev-parse HEAD HEAD^{tree} > "$task_log/carrier-head.txt"
trap finish EXIT
git switch --detach "$task_product" > "$task_log/product-checkout.log" 2>&1
git rev-parse HEAD HEAD^{tree} > "$task_log/product-head.txt"
[[ -z $(git status --porcelain --untracked-files=all) ]]
[[ -z ${CARGO_TARGET_DIR+x} ]]
[[ $(gh api user --jq .login) == Xuanwo ]]
[[ $(gh api repos/Xuanwo/opendal-pr-8165-fixture-20261103 --jq .id) == 1443743524 ]]
[[ $(gh api repos/Xuanwo/opendal-pr-8165-fixture-20261103 --jq .private) == true ]]
[[ $(gh api repos/Xuanwo/opendal-pr-8165-fixture-20261103 --jq .default_branch) == main ]]
cd core
cargo --version > "$task_log/toolchain.txt"
rustc --version >> "$task_log/toolchain.txt"
run_stage package-unit cargo test --locked -p opendal-service-github --lib
run_stage package-doc cargo test --locked -p opendal-service-github --doc
run_stage package-clippy cargo clippy --locked -p opendal-service-github --all-targets -- -D warnings
run_stage facade-clippy cargo clippy --locked --all-targets --features tests,services-github -- -D warnings
run_stage fmt cargo fmt --all -- --check
export OPENDAL_TEST=github
export OPENDAL_GITHUB_OWNER=Xuanwo
export OPENDAL_GITHUB_REPO=opendal-pr-8165-fixture-20261103
export OPENDAL_GITHUB_ROOT=/pr8165-integrated-ad7316ac/
export OPENDAL_GITHUB_TOKEN
OPENDAL_GITHUB_TOKEN="$(gh auth token)"
[[ -n "$OPENDAL_GITHUB_TOKEN" ]]
export RUST_LOG=warn
# The real fixture uses the normal main entrypoint and sequential Contents API writes.
run_stage canonical cargo test --locked --features tests,services-github --test behavior -- test_delete_with_if_match_ --test-threads=1 --nocapture
rg -q 'test result: ok\. 3 passed; 0 failed' "$task_log/canonical.log"
unset OPENDAL_GITHUB_TOKEN
gh api 'repos/Xuanwo/opendal-pr-8165-fixture-20261103/git/trees/main?recursive=1' > "$task_log/provider-tree-after.json"
cd "$task_repo_root"
run_stage source-clean git diff --exit-code
[[ -z $(git status --porcelain --untracked-files=all) ]]
