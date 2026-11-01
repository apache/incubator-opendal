---
title: Getting started
sidebar_label: Getting started
description: Run your first OpenDAL program in Python, then point it at a real storage backend.
---

# Getting started

## Your first program

This program builds an operator, writes a file, reads it back, inspects its
metadata, and deletes it. It runs against the in-memory service with no
credentials, so you can run it right after installing.

Install from PyPI:

```shell
pip install opendal
```

Or install from Anaconda main:

```shell
conda install opendal
```

Or install from conda-forge:

```shell
conda install conda-forge::opendal
```

```python file=bindings/python/examples/getting_started.py region=quickstart
```

Note that `write` takes `bytes` and `read` returns `bytes`.

## Point it at a real backend

Only the service changes; the operations stay identical. The scheme is the first
argument, and configuration is passed as keyword arguments:

```python
import opendal

op = opendal.Operator("s3", bucket="my-bucket", region="us-east-1")

op.write("hello.txt", b"Hello from S3!")
print(op.read("hello.txt"))
```

The wheel bundles every service, so there is nothing extra to install. The next
page, [Connecting to your storage](./03-connecting.md), covers configuration and
credentials in depth; [Services](/services) lists every backend and its keys.

## Async usage {#blocking-vs-async}

OpenDAL has a first-class async API. Use [`AsyncOperator`] with the same verbs,
awaiting each call:

```python
import asyncio
import opendal

async def main():
    op = opendal.AsyncOperator("s3", bucket="my-bucket", region="us-east-1")
    await op.write("hello.txt", b"Hello, World!")
    data = await op.read("hello.txt")
    print(data.decode())

asyncio.run(main())
```

Both operators expose the same operations. Convert between them with
`op.to_async_operator()` and `op.to_operator()` when you need the other form.

[`AsyncOperator`]: https://opendal.apache.org/docs/python/api/async_operator/#opendal.AsyncOperator
