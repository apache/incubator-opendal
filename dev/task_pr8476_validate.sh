#!/usr/bin/env bash
set -uo pipefail

task_carrier=${1:?pass the Git carrier commit}
task_logs=${2:?pass the log directory}
task_base=edac5a0f9d6748c46dc28eabd645066cdaaa5d96
task_head=fae76b7a7bf5b0268609af2a92f3cfd76b1ebb03
task_probe=core/tests/request_signing_retry_probe.rs
task_root=$(git rev-parse --show-toplevel)
task_start=$(git rev-parse HEAD)
cd "$task_root" || exit 1
test -z "$(git status --porcelain)" || exit 1
mkdir -p "$task_logs" || exit 1
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9 || exit 1
export CARGO_BUILD_JOBS=4
unset CARGO_TARGET_DIR CARGO_BUILD_BUILD_DIR

task_restore() {
    cd "$task_root" || return
    rm -f -- "$task_probe"
    git switch --detach "$task_start" >"$task_logs/restore.log" 2>&1
    git status --porcelain >"$task_logs/post-status.log"
    git rev-parse HEAD >"$task_logs/post-head.log"
}
trap task_restore EXIT

task_run() {
    local label=$1
    shift
    "$@" >"$task_logs/$label.log" 2>&1
    local result=$?
    printf '%s\n' "$result" >"$task_logs/$label.exit"
    return "$result"
}

git switch --detach "$task_base" >"$task_logs/base-checkout.log" 2>&1 || exit 1
git rev-parse HEAD >"$task_logs/base-head.log"
git show "$task_carrier:$task_probe" >"$task_probe" || exit 1
cd core || exit 1
task_run baseline cargo test --locked -p opendal --features services-gcs --test request_signing_retry_probe temporary_token_failure_recovers -- --exact --nocapture
test "$?" -eq 101 || exit 1
rg -Fq 'temporary token failure must retry' "$task_logs/baseline.log" || exit 1
rg -Fq 'test result: FAILED. 0 passed; 1 failed;' "$task_logs/baseline.log" || exit 1
cd "$task_root" || exit 1
rm -- "$task_probe"

git switch --detach "$task_head" >"$task_logs/head-checkout.log" 2>&1 || exit 1
git rev-parse HEAD >"$task_logs/fixed-head.log"
git rev-parse 'HEAD^{tree}' >"$task_logs/fixed-tree.log"
git show "$task_carrier:$task_probe" >"$task_probe" || exit 1
cd core || exit 1
task_run fixed cargo test --locked -p opendal --features services-gcs --test request_signing_retry_probe -- --nocapture || exit 1
rg -Fq 'test result: ok. 3 passed; 0 failed;' "$task_logs/fixed.log" || exit 1
cd "$task_root" || exit 1
rm -- "$task_probe"
cd core || exit 1
task_run clippy-with-reqsign cargo clippy --locked -p opendal-core --no-default-features --features reqsign --lib -- -D warnings || exit 1
task_run clippy-without-reqsign cargo clippy --locked -p opendal-core --no-default-features --lib -- -D warnings || exit 1
task_run fmt cargo fmt --all -- --check || exit 1
cd "$task_root" || exit 1
git status --porcelain >"$task_logs/fixed-status.log"
test ! -s "$task_logs/fixed-status.log" || exit 1
printf '0\n' >"$task_logs/driver.exit"
