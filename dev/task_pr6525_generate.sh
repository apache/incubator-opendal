#!/usr/bin/env bash
set -Eeuo pipefail

TASK_REPO=/home/xuanwo/Code/worktrees/od-review-s3-container-provider-20261102
TASK_PRODUCT=18ddc415d15118cca7faf4f708d2185beb3615e3
TASK_LOG_ROOT=/home/xuanwo/Code/logs/opendal-pr6525-generation-18ddc415-20261102
export PATH=/home/xuanwo/.cargo/bin:$PATH
export CARGO_BUILD_JOBS=4

mkdir -p /home/xuanwo/Code/logs
mkdir "$TASK_LOG_ROOT"
task_finish() {
    local rc=$?
    trap - EXIT
    printf '%s\n' "$rc" > "$TASK_LOG_ROOT/driver.status"
    (cd "$TASK_LOG_ROOT"; shopt -s nullglob; sha256sum ./*.log ./*.status ./*.txt > manifest.sha256.tmp; mv manifest.sha256.tmp manifest.sha256)
    exit "$rc"
}
trap task_finish EXIT
task_step() {
    local label=$1
    shift
    local rc=0
    "$@" > "$TASK_LOG_ROOT/$label.log" 2>&1 || rc=$?
    printf '%s\n' "$rc" > "$TASK_LOG_ROOT/$label.status"
    return "$rc"
}

cd "$TASK_REPO"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
git status --porcelain > "$TASK_LOG_ROOT/pre-clean.txt"
test ! -s "$TASK_LOG_ROOT/pre-clean.txt"
git switch --detach "$TASK_PRODUCT" > "$TASK_LOG_ROOT/checkout.log" 2>&1
git rev-parse HEAD > "$TASK_LOG_ROOT/product-head.txt"
git rev-parse HEAD^{tree} > "$TASK_LOG_ROOT/product-tree.txt"
test "$(git rev-parse HEAD)" = "$TASK_PRODUCT"
cargo --version > "$TASK_LOG_ROOT/cargo-version.txt"
rustc --version > "$TASK_LOG_ROOT/rustc-version.txt"

task_step generate-python cargo run --locked --manifest-path=dev/Cargo.toml -- generate -l python
task_step generate-java cargo run --locked --manifest-path=dev/Cargo.toml -- generate -l java
task_step generate-dotnet cargo run --locked --manifest-path=dev/Cargo.toml -- generate -l dotnet
task_step fmt-core cargo fmt --manifest-path=core/Cargo.toml --all -- --check
task_step fmt-python cargo fmt --manifest-path=bindings/python/Cargo.toml -- --check
task_step parse-python python3 -c 'import ast,pathlib; ast.parse(pathlib.Path("bindings/python/python/opendal/config.py").read_text())'
task_step diff-check git diff --check
git diff --name-only > "$TASK_LOG_ROOT/generated-paths.txt"
git diff --stat > "$TASK_LOG_ROOT/generated-stat.txt"
test "$(wc -l < "$TASK_LOG_ROOT/generated-paths.txt")" -eq 4
while IFS= read -r task_path; do
    case "$task_path" in
        bindings/python/src/services.rs|bindings/python/python/opendal/config.py|bindings/java/src/main/java/org/apache/opendal/ServiceConfig.java|bindings/dotnet/OpenDAL/ServiceConfig/S3ServiceConfig.cs) ;;
        *) exit 19 ;;
    esac
    sha256sum "$task_path" >> "$TASK_LOG_ROOT/generated-files.txt"
done < "$TASK_LOG_ROOT/generated-paths.txt"
git rev-parse HEAD > "$TASK_LOG_ROOT/post-head.txt"
test "$(git rev-parse HEAD)" = "$TASK_PRODUCT"
git status --porcelain > "$TASK_LOG_ROOT/post-status.txt"
