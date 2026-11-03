#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS=4
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
product=0e1ad8c89d8795f28b9346edeccf2122554ec9ff
log_dir=/home/xuanwo/Code/logs/opendal-pr8350-generation-0e1ad8c8-20261103
[[ ! -e "$log_dir" ]]
mkdir -p "$log_dir"
cd "$repo_root"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
trap 'task_exit=$?; printf "%s\n" "$task_exit" > "$log_dir/driver.exit"' EXIT
[[ $(git rev-parse HEAD^) == "$product" ]]
[[ -z $(git status --porcelain --untracked-files=all) ]]
git rev-parse HEAD HEAD^{tree} > "$log_dir/carrier-head.txt"
git switch --detach "$product" > "$log_dir/checkout.log" 2>&1
git rev-parse HEAD HEAD^{tree} > "$log_dir/product-head.txt"
cargo --version > "$log_dir/toolchain.txt"
run_stage() {
    local name="$1"; shift
    set +e
    "$@" > "$log_dir/$name.log" 2>&1
    local code=$?
    set -e
    printf "%s\n" "$code" > "$log_dir/$name.exit"
    [[ "$code" -eq 0 ]]
}
run_stage dev-fmt cargo fmt --manifest-path dev/Cargo.toml --all -- --check
run_stage python cargo run --manifest-path=dev/Cargo.toml -- generate -l python
run_stage java cargo run --manifest-path=dev/Cargo.toml -- generate -l java
run_stage dotnet cargo run --manifest-path=dev/Cargo.toml -- generate -l dotnet
run_stage docs cargo run --manifest-path=dev/Cargo.toml -- generate -l docs
run_stage python-fmt cargo fmt --manifest-path bindings/python/Cargo.toml --all
run_stage dotnet-fmt cargo fmt --manifest-path bindings/dotnet/Cargo.toml --all
run_stage generated-clean git diff --exit-code
git status --porcelain --untracked-files=all > "$log_dir/final-status.txt"
[[ ! -s "$log_dir/final-status.txt" ]]
