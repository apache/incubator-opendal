# OpenDAL Python Bindings Benchmark

This benchmark compares OpenDAL with the AWS SDK for Python (Boto3). The Boto3
benchmark runs its synchronous client in a bounded thread pool to issue the
same concurrent workload without an alternative I/O runtime.

To run the benchmark, please make sure the following env have been set correctly.

- AWS_REGION: the region of the s3 service
- AWS_ENDPOINT: the endpoint of the s3 service
- AWS_ACCESS_KEY_ID: the access key of the s3 service
- AWS_SECRET_ACCESS_KEY: the secret key of the s3 service
- AWS_S3_BUCKET: the bucket name of the s3 service

To run the benchmark:

```shell
maturin develop -r -E=benchmark

export AWS_ENDPOINT=http://127.0.0.1:9000
export AWS_REGION=us-east-1
export AWS_ACCESS_KEY_ID=minioadmin
export AWS_SECRET_ACCESS_KEY=minioadmin
export AWS_S3_BUCKET=opendal

uv run async_opendal_benchmark.py
uv run async_origin_s3_benchmark_with_threads.py
```

## Blocking Operator GIL benchmark

`blocking_gil_benchmark.py` uses a local HTTP subprocess with a configurable
response delay, so it does not require service credentials. It reports the
median and p95 latency for one blocking read, two sequential reads, and two
reads started from independent Python threads.

From `bindings/python`, build the binding from the revision to measure, then
run:

```shell
uv run maturin develop --release
uv run benchmark/blocking_gil_benchmark.py \
  --delay-ms 50 --iterations 30 --warmups 3
```

Run the same command from an unchanged baseline checkout to compare
single-thread overhead and concurrency speedup with identical local I/O.

## File Open and Read over HTTP

The repository's nginx fixture provides a reproducible benchmark for opening a
Python file object and reading its first byte:

```shell
docker compose -f fixtures/http/docker-compose-nginx.yml up -d --wait

cd bindings/python
uv run maturin develop
uv run python benchmark/file_open_read_benchmark.py
```

The benchmark reports the per-operation latency for synchronous and asynchronous
file APIs. The nginx access log can be used to verify the corresponding HTTP
request count.
