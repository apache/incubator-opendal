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
import json
import multiprocessing
import statistics
import threading
import time
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from multiprocessing.connection import Connection


class _DelayedHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        time.sleep(self.server.response_delay_seconds)
        body = b"benchmark response"
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format_: str, *args: object) -> None:
        pass


def _serve_delayed_http(port_sender: Connection, delay_seconds: float) -> None:
    server = ThreadingHTTPServer(("127.0.0.1", 0), _DelayedHandler)
    server.response_delay_seconds = delay_seconds
    port_sender.send(server.server_address[1])
    port_sender.close()
    server.serve_forever(poll_interval=0.01)


def _measure(operation: Callable[[], object], iterations: int) -> list[float]:
    samples = []
    for _ in range(iterations):
        started_at = time.perf_counter()
        operation()
        samples.append((time.perf_counter() - started_at) * 1_000)
    return samples


def _summarize(samples: list[float]) -> dict[str, float]:
    sorted_samples = sorted(samples)
    p95_index = max(0, (len(sorted_samples) * 95 + 99) // 100 - 1)
    return {
        "median_ms": round(statistics.median(sorted_samples), 3),
        "p95_ms": round(sorted_samples[p95_index], 3),
    }


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Benchmark blocking Operator GIL release with local delayed HTTP I/O."
        )
    )
    parser.add_argument("--delay-ms", type=float, default=50.0)
    parser.add_argument("--iterations", type=int, default=30)
    parser.add_argument("--warmups", type=int, default=3)
    return parser.parse_args()


def main() -> None:
    import opendal

    args = _parse_args()
    if args.delay_ms < 0:
        message = "--delay-ms must be non-negative"
        raise ValueError(message)
    if args.iterations < 1:
        message = "--iterations must be positive"
        raise ValueError(message)
    if args.warmups < 0:
        message = "--warmups must be non-negative"
        raise ValueError(message)

    context = multiprocessing.get_context("spawn")
    port_receiver, port_sender = context.Pipe(duplex=False)
    server_process = context.Process(
        target=_serve_delayed_http,
        args=(port_sender, args.delay_ms / 1_000),
    )
    server_process.start()
    port_sender.close()

    try:
        if not port_receiver.poll(10):
            message = "delayed HTTP benchmark server did not start"
            raise RuntimeError(message)
        port = port_receiver.recv()
        operator = opendal.Operator("http", endpoint=f"http://127.0.0.1:{port}")

        for _ in range(args.warmups):
            operator.read("warmup")

        single_read = _measure(lambda: operator.read("single"), args.iterations)
        sequential_pair = _measure(
            lambda: (operator.read("first"), operator.read("second")),
            args.iterations,
        )

        with ThreadPoolExecutor(max_workers=2) as executor:

            def concurrent_pair() -> list[bytes]:
                start = threading.Barrier(3)

                def read(path: str) -> bytes:
                    start.wait()
                    return operator.read(path)

                futures = [
                    executor.submit(read, "first"),
                    executor.submit(read, "second"),
                ]
                start.wait()
                return [future.result() for future in futures]

            concurrent_pair_samples = _measure(concurrent_pair, args.iterations)

        sequential_median = statistics.median(sequential_pair)
        concurrent_median = statistics.median(concurrent_pair_samples)
        result = {
            "delay_ms": args.delay_ms,
            "iterations": args.iterations,
            "single_read": _summarize(single_read),
            "sequential_two_reads": _summarize(sequential_pair),
            "concurrent_two_reads": _summarize(concurrent_pair_samples),
            "concurrency_speedup": round(sequential_median / concurrent_median, 3),
        }
        print(json.dumps(result, indent=2, sort_keys=True))
    finally:
        server_process.terminate()
        server_process.join(10)
        port_receiver.close()


if __name__ == "__main__":
    main()
