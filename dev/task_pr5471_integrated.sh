#!/usr/bin/env bash
set -Eeuo pipefail
export PATH=/home/xuanwo/.cargo/bin:$PATH
export CARGO_BUILD_JOBS=4
TASK_REPO=/home/xuanwo/Code/worktrees/od-review-5471-20261101
TASK_PRODUCT=82a610812adc63d3da411d0e989c2ce688a520af
TASK_LOG=/home/xuanwo/Code/logs/opendal-pr5471-main042bc-20261102
mkdir "$TASK_LOG"
trap 'task_rc=$?; printf "%s\n" "$task_rc" > "$TASK_LOG/driver.status"' EXIT
step() {
    local name=$1
    shift
    local rc=0
    "$@" > "$TASK_LOG/$name.log" 2>&1 || rc=$?
    printf '%s\n' "$rc" > "$TASK_LOG/$name.status"
    return "$rc"
}
cd "$TASK_REPO"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
git status --porcelain > "$TASK_LOG/pre-clean.txt"
test ! -s "$TASK_LOG/pre-clean.txt"
git switch --detach "$TASK_PRODUCT" > "$TASK_LOG/checkout.log" 2>&1
git rev-parse HEAD > "$TASK_LOG/head.txt"
git rev-parse HEAD^{tree} > "$TASK_LOG/tree.txt"
test "$(git rev-parse HEAD)" = "$TASK_PRODUCT"
rustc --version > "$TASK_LOG/rustc.txt"
cargo --version > "$TASK_LOG/cargo.txt"
cd core
step tests cargo test --locked -p opendal-service-memcached
step clippy cargo clippy --locked -p opendal-service-memcached --all-targets -- -D warnings
step fmt cargo fmt --all -- --check
cd ..
step diff-check git diff --check
git status --porcelain > "$TASK_LOG/post-clean.txt"
test ! -s "$TASK_LOG/post-clean.txt"
git rev-parse HEAD > "$TASK_LOG/post-head.txt"
test "$(git rev-parse HEAD)" = "$TASK_PRODUCT"
