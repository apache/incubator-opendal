#!/usr/bin/env bash
set -Eeuo pipefail
task_log=${1:?log directory}
task_product=${2:?product commit}
cd "$(git rev-parse --show-toplevel)"
[[ $(git rev-parse HEAD^) == "$task_product" ]]
[[ -z $(git status --porcelain=v1) ]]
mkdir -p "$task_log"
trap 'task_exit=$?; printf "%s\n" "$task_exit" > "$task_log/driver.exit"; git rev-parse HEAD HEAD^{tree} > "$task_log/post.head-tree"; git status --porcelain=v1 > "$task_log/post.status"; exit "$task_exit"' EXIT
git rev-parse HEAD HEAD^{tree} HEAD^ HEAD^^{tree} > "$task_log/pre.head-tree"
git status --porcelain=v1 > "$task_log/pre.status"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
export CARGO_BUILD_JOBS=4
rustc -Vv > "$task_log/rustc-version.log"
cargo --version > "$task_log/cargo-version.log"
java -version > "$task_log/java-version.log" 2>&1
mvn --version > "$task_log/maven-version.log" 2>&1
run_check() {
    local task_name=$1
    shift
    set +e
    "$@" > "$task_log/$task_name.log" 2>&1
    local task_result=$?
    set -e
    printf '%s\n' "$task_result" > "$task_log/$task_name.exit"
    [[ $task_result == 0 ]] || exit "$task_result"
}
run_check generate-java cargo run --locked --manifest-path dev/Cargo.toml -- generate -l java
run_check generate-python cargo run --locked --manifest-path dev/Cargo.toml -- generate -l python
run_check generate-dotnet cargo run --locked --manifest-path dev/Cargo.toml -- generate -l dotnet
run_check generate-docs cargo run --locked --manifest-path dev/Cargo.toml -- generate -l docs
run_check generated-diff git diff --exit-code
run_check spotless mvn --batch-mode -f bindings/java/pom.xml spotless:check
run_check dev-test cargo test --locked --manifest-path dev/Cargo.toml
run_check dev-clippy cargo clippy --locked --manifest-path dev/Cargo.toml --all-targets -- -D warnings
run_check dev-fmt cargo fmt --manifest-path dev/Cargo.toml --all -- --check
run_check final-diff git diff --exit-code
git status --porcelain=v1 > "$task_log/final.status"
run_check source-clean test ! -s "$task_log/final.status"
