#!/usr/bin/env bash
set -euo pipefail

# Task-only repeated execution of the original conditional-error fixture.
task_log=${1:?Pass a new evidence directory}
test ! -e "$task_log"
mkdir -p "$task_log"
task_repo=$(git rev-parse --show-toplevel)
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
test -z "$(git status --porcelain)"
git rev-parse HEAD HEAD^{tree} >"$task_log/source.head-tree"
sha256sum "$0" >"$task_log/driver.sha256"
java -version >"$task_log/java.version" 2>&1
rustc --version >"$task_log/rust.version"
export CARGO_BUILD_JOBS=4 CARGO_TERM_COLOR=never RUST_LOG=opendal=trace
unset CARGO_TARGET_DIR OPENDAL_TEST
cd "$task_repo/bindings/java"
./mvnw --version >"$task_log/maven.version" 2>&1
set +e
./mvnw -B '-Dtest=ReaderOptionsTest#testConditionalErrorsKeepTheirCode' test \
    >"$task_log/maven-test.log" 2>&1
task_test_exit=$?
set -e
printf '%s\n' "$task_test_exit" >"$task_log/maven-test.exit"
if test -f target/surefire-reports/TEST-org.apache.opendal.test.ReaderOptionsTest.xml; then
    cp target/surefire-reports/TEST-org.apache.opendal.test.ReaderOptionsTest.xml "$task_log/"
fi
cd "$task_repo"
git rev-parse HEAD HEAD^{tree} >"$task_log/post.head-tree"
git status --porcelain >"$task_log/post.status"
exit "$task_test_exit"
