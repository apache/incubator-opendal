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
java -version >"$task_log/java.version" 2>&1
rustc --version >"$task_log/rust.version"
export CARGO_BUILD_JOBS=4 CARGO_TERM_COLOR=never RUST_LOG=opendal=trace
unset CARGO_TARGET_DIR OPENDAL_TEST
cd "$task_repo/bindings/java"
./mvnw --version >"$task_log/maven.version" 2>&1
task_result=0
for task_mode in false true; do
    mkdir "$task_log/$task_mode"
    set +e
    ./mvnw -B '-Dtest=ReaderOptionsTest#testConditionalErrorsKeepTheirCode' \
        "-Dopendal.task.completeRequest=$task_mode" test >"$task_log/$task_mode/maven-test.log" 2>&1
    task_exit=$?
    set -e
    printf '%s\n' "$task_exit" >"$task_log/$task_mode/maven-test.exit"
    cp target/surefire-reports/TEST-org.apache.opendal.test.ReaderOptionsTest.xml "$task_log/$task_mode/"
    if test "$task_exit" != 0; then task_result=1; fi
done
cd "$task_repo"
git rev-parse HEAD HEAD^{tree} >"$task_log/post.head-tree"
git status --porcelain >"$task_log/post.status"
cmp "$task_log/source.head-tree" "$task_log/post.head-tree"
test ! -s "$task_log/post.status"
exit "$task_result"
