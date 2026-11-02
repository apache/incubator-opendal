#!/usr/bin/env bash
set -Eeuo pipefail
task_service=${1:?service}
task_log=${2:?log directory}
task_product=${3:?product commit}
case "$task_service" in s3|git|smb) ;; *) exit 2 ;; esac
task_root=$(git rev-parse --show-toplevel)
cd "$task_root"
[[ $(git rev-parse HEAD^) == "$task_product" ]]
[[ -z $(git status --porcelain=v1) ]]
[[ $(git diff --name-only HEAD^ HEAD) == dev/task_service_msrv_check.sh ]]
mkdir -p "$task_log"
trap 'task_exit=$?; printf "%s\n" "$task_exit" > "$task_log/driver.exit"; git rev-parse HEAD HEAD^{tree} > "$task_log/post.head-tree"; git status --porcelain=v1 > "$task_log/post.status"; exit "$task_exit"' EXIT
git rev-parse HEAD HEAD^{tree} > "$task_log/pre.head-tree"
git status --porcelain=v1 > "$task_log/pre.status"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
export CARGO_BUILD_JOBS=4
rustup run 1.91.1 rustc -Vv > "$task_log/rustc-version.log"
cargo +1.91.1 --version > "$task_log/cargo-version.log"
set +e
cargo +1.91.1 check --locked --manifest-path core/Cargo.toml -p "opendal-service-$task_service" --all-targets > "$task_log/package.log" 2>&1
task_package_exit=$?
printf '%s\n' "$task_package_exit" > "$task_log/package.exit"
[[ $task_package_exit == 0 ]] || exit "$task_package_exit"
cargo +1.91.1 check --locked --manifest-path core/Cargo.toml --features "services-$task_service" --all-targets > "$task_log/facade.log" 2>&1
task_facade_exit=$?
printf '%s\n' "$task_facade_exit" > "$task_log/facade.exit"
exit "$task_facade_exit"
