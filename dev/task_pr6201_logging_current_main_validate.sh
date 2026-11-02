#!/usr/bin/env bash
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
base_head="${BASE_HEAD:?original worktree HEAD required}"
log_dir="${LOG_DIR:?new log directory required}"
product_head=e2d122cf3e038d84c11c75c9f32d68fd65024b15
product_tree=d31c3c22384056f10779ef8b199bae029a237ade
original_head=c6b528c5277ea716104b59be651c61626c71908f
gtest_source=/home/xuanwo/Code/task-deps/opendal-pr6201-googletest-v1.15.2
native_deps=/home/xuanwo/reviews/toolchains/opendal-7999-deps
runner_path="$repo_root/bindings/c/tests/opendal_test_runner"

test "$base_head" = "$original_head"
test ! -e "$log_dir"
mkdir -p "$log_dir"
cd "$repo_root"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9

export PATH="/home/xuanwo/.local/share/uv/tools/cmake/bin:/home/xuanwo/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS=4
export CPATH="$native_deps/root/usr/include:$native_deps/root/usr/include/x86_64-linux-gnu${CPATH:+:$CPATH}"
export LIBRARY_PATH="$native_deps/lib${LIBRARY_PATH:+:$LIBRARY_PATH}"
export LD_LIBRARY_PATH="$repo_root/bindings/c/target/debug:$native_deps/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

cleanup_owned_runner() {
    if [ -f "$runner_path" ] && [ ! -L "$runner_path" ]; then
        sha256sum "$runner_path" >> "$log_dir/runner-cleanup.sha256"
        rm -f -- "$runner_path"
        return 0
    fi
    if [ -e "$runner_path" ] || [ -L "$runner_path" ]; then
        return 1
    fi
}

finish() {
    local result=$? restore_status cleanup_status
    trap - EXIT
    set +e
    git switch --detach "$base_head" > "$log_dir/restore.log" 2>&1
    restore_status=$?
    cleanup_owned_runner
    cleanup_status=$?
    printf '%s\n' "$restore_status" > "$log_dir/restore.status"
    printf '%s\n' "$cleanup_status" > "$log_dir/restored-runner-cleanup.status"
    git rev-parse HEAD HEAD^{tree} > "$log_dir/restored-head.txt"
    git status --porcelain --untracked-files=all > "$log_dir/restored-status.txt"
    if [ "$restore_status" -ne 0 ] || [ "$cleanup_status" -ne 0 ] || [ -s "$log_dir/restored-status.txt" ]; then
        result=2
    fi
    printf '%s\n' "$result" > "$log_dir/driver.exit"
    exit "$result"
}
run_stage() {
    local stage="$1"
    shift
    set +e
    "$@" > "$log_dir/$stage.log" 2>&1
    local stage_status=$?
    set -e
    printf '%s\n' "$stage_status" > "$log_dir/$stage.status"
    test "$stage_status" -eq 0
}

git rev-parse HEAD HEAD^{tree} > "$log_dir/carrier-head.txt"
test "$(git rev-parse HEAD^)" = "$product_head"
test "$(git rev-parse "$product_head^{tree}")" = "$product_tree"
sha256sum dev/task_pr6201_logging_current_main_validate.sh > "$log_dir/driver.sha256"
git status --porcelain --untracked-files=all > "$log_dir/pre-status.txt"
test ! -s "$log_dir/pre-status.txt"
test ! -e "$runner_path"
test ! -L "$runner_path"
trap finish EXIT

git switch --detach "$product_head" > "$log_dir/product-checkout.log" 2>&1
git rev-parse HEAD HEAD^{tree} > "$log_dir/source-head.txt"
cargo --version > "$log_dir/toolchain.txt"
rustc --version >> "$log_dir/toolchain.txt"
cmake --version >> "$log_dir/toolchain.txt"
g++ --version >> "$log_dir/toolchain.txt"

test "$(git -C "$gtest_source" remote get-url origin)" = https://github.com/google/googletest.git
test "$(git -C "$gtest_source" describe --tags --exact-match HEAD)" = v1.15.2
git -C "$gtest_source" rev-parse HEAD > "$log_dir/gtest-head.txt"
git -C "$gtest_source" status --porcelain --untracked-files=all > "$log_dir/gtest-status.txt"
test ! -s "$log_dir/gtest-status.txt"

run_stage diff-check git diff --check
run_stage fmt cargo fmt --manifest-path bindings/c/Cargo.toml -- --check
run_stage clippy cargo clippy --manifest-path bindings/c/Cargo.toml -- -D warnings
run_stage cmake-configure cmake -S bindings/c -B bindings/c/build \
    -DTEST_ENABLE_ASAN=ON \
    -DFEATURES=opendal/services-memory \
    -DFETCHCONTENT_SOURCE_DIR_GOOGLETEST="$gtest_source"
run_stage native-build cmake --build bindings/c/build --parallel 4
run_stage getting-started bindings/c/build/getting_started
run_stage gtest bindings/c/build/tests
rg -q '\[       OK \] OpendalLogging.NativeEventsAndRegistrationFailure' "$log_dir/gtest.log"

cd bindings/c/tests
run_stage make-clean make clean
run_stage make-build make -j4
run_stage memory-framework env OPENDAL_TEST=memory make behavior_test
run_stage list-suites make list-suites
cd "$repo_root"

sha256sum bindings/c/target/debug/libopendal_c.so > "$log_dir/native-library.sha256"
sha256sum bindings/c/Cargo.lock > "$log_dir/ignored-cargo-lock.sha256"
run_stage runner-cleanup cleanup_owned_runner
run_stage tracked-clean git diff --exit-code
git status --porcelain --untracked-files=all > "$log_dir/product-final-status.txt"
test ! -s "$log_dir/product-final-status.txt"
