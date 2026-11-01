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

import argparse
import asyncio
import statistics
import time

import opendal


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Benchmark Python file open followed by a one-byte HTTP read."
    )
    parser.add_argument("--endpoint", default="http://127.0.0.1:8080")
    parser.add_argument("--path", default="normal_file.txt")
    parser.add_argument("--iterations", type=int, default=100)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=10)
    return parser.parse_args()


def sync_once(op: opendal.Operator, path: str) -> None:
    with op.open(path, "rb") as file:
        file.read(1)


async def async_once(op: opendal.AsyncOperator, path: str) -> None:
    async with await op.open(path, "rb") as file:
        await file.read(1)


def benchmark_sync(args: argparse.Namespace) -> list[float]:
    op = opendal.Operator("http", endpoint=args.endpoint, root="/")
    for _ in range(args.warmups):
        sync_once(op, args.path)

    results = []
    for _ in range(args.rounds):
        started = time.perf_counter()
        for _ in range(args.iterations):
            sync_once(op, args.path)
        elapsed = time.perf_counter() - started
        results.append(elapsed * 1000 / args.iterations)
    return results


async def benchmark_async(args: argparse.Namespace) -> list[float]:
    op = opendal.AsyncOperator("http", endpoint=args.endpoint, root="/")
    for _ in range(args.warmups):
        await async_once(op, args.path)

    results = []
    for _ in range(args.rounds):
        started = time.perf_counter()
        for _ in range(args.iterations):
            await async_once(op, args.path)
        elapsed = time.perf_counter() - started
        results.append(elapsed * 1000 / args.iterations)
    return results


def print_result(name: str, values: list[float]) -> None:
    formatted = ", ".join(f"{value:.3f}" for value in values)
    print(f"{name}_ms_per_operation=[{formatted}]")
    print(f"{name}_median_ms={statistics.median(values):.3f}")


def main() -> None:
    args = parse_args()
    print(f"endpoint={args.endpoint}")
    print(f"path={args.path}")
    print(f"rounds={args.rounds}")
    print(f"iterations_per_round={args.iterations}")
    print_result("sync_open_read", benchmark_sync(args))
    print_result("async_open_read", asyncio.run(benchmark_async(args)))


if __name__ == "__main__":
    main()
