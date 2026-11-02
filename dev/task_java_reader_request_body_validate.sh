#!/usr/bin/env bash
set -euo pipefail

task_log=${1:?Pass a new evidence directory}
test ! -e "$task_log"
mkdir -p "$task_log"
task_repo=$(git rev-parse --show-toplevel)
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
test -z "$(git status --porcelain)"
test "$(git rev-parse HEAD^)" = 8ff810a83ee7e6dc455b23fe93523ab49d284ba3
git rev-parse HEAD HEAD^{tree} >"$task_log/source.head-tree"
git rev-parse HEAD^ HEAD^^ >"$task_log/product-and-base.head"
sha256sum "$0" >"$task_log/driver.sha256"
java -version >"$task_log/java.version" 2>&1
rustc --version >"$task_log/rust.version"
export CARGO_BUILD_JOBS=4 CARGO_TERM_COLOR=never RUST_LOG=opendal=trace
unset CARGO_TARGET_DIR OPENDAL_TEST
cd "$task_repo/bindings/java"
./mvnw --version >"$task_log/maven.version" 2>&1
set +e
./mvnw -B test >"$task_log/java-tests.log" 2>&1
task_test_exit=$?
printf '%s\n' "$task_test_exit" >"$task_log/java-tests.exit"
./mvnw -B spotless:check >"$task_log/spotless.log" 2>&1
task_format_exit=$?
printf '%s\n' "$task_format_exit" >"$task_log/spotless.exit"
set -e
mkdir "$task_log/surefire-reports"
cp target/surefire-reports/TEST-*.xml "$task_log/surefire-reports/"
sha256sum target/classes/native/linux-x86_64/libopendal_java.so >"$task_log/native.sha256"
cd "$task_repo"
git diff --check >"$task_log/diff-check.log"
git rev-parse HEAD HEAD^{tree} >"$task_log/post.head-tree"
git status --porcelain >"$task_log/post.status"
cmp "$task_log/source.head-tree" "$task_log/post.head-tree"
test ! -s "$task_log/post.status"
test "$task_test_exit" = 0 && test "$task_format_exit" = 0
