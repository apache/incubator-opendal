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

task_log=${1:?Pass a new evidence directory}
test ! -e "$task_log"
mkdir -p "$task_log"
task_repo=$(git rev-parse --show-toplevel)
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9

git status --porcelain >"$task_log/source.status"
test ! -s "$task_log/source.status"
git rev-parse HEAD HEAD^{tree} >"$task_log/source.head-tree"

export CARGO_BUILD_JOBS=4
export CARGO_TERM_COLOR=never
export RUST_LOG=off
unset CARGO_TARGET_DIR OPENDAL_TEST

cd "$task_repo/core"
rustc --version >"$task_log/rust.version"

set +e
cargo test --locked --no-run -p opendal --test task_s3_container_provider \
    --features tests,services-s3 >"$task_log/compile.log" 2>&1
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
test "$task_compile_exit" = 0
test "$task_format_exit" = 0
