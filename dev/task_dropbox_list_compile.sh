#!/usr/bin/env bash
set -euo pipefail

task_log=${1:?Pass a new evidence directory}
test ! -e "$task_log"
mkdir -p "$task_log"
task_repo=$(git rev-parse --show-toplevel)
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
test -z "$(git status --porcelain)"
git rev-parse HEAD HEAD^{tree} >"$task_log/source.head-tree"
export CARGO_BUILD_JOBS=4 CARGO_TERM_COLOR=never
unset CARGO_TARGET_DIR OPENDAL_TEST
cd "$task_repo/core"
rustc --version >"$task_log/rust.version"
set +e
cargo test --locked --no-run --test task_dropbox_list_contract --features tests,services-dropbox >"$task_log/compile.log" 2>&1
task_compile_exit=$?
printf '%s\n' "$task_compile_exit" >"$task_log/compile.exit"
cargo fmt --all -- --check >"$task_log/format.log" 2>&1
task_format_exit=$?
printf '%s\n' "$task_format_exit" >"$task_log/format.exit"
set -e
cd "$task_repo"
git rev-parse HEAD HEAD^{tree} >"$task_log/post.head-tree"
git status --porcelain >"$task_log/post.status"
cmp "$task_log/source.head-tree" "$task_log/post.head-tree"
test ! -s "$task_log/post.status"
test "$task_compile_exit" = 0 && test "$task_format_exit" = 0
