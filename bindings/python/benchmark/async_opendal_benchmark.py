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

import asyncio
import timeit

from pydantic_settings import BaseSettings

import opendal


class Config(BaseSettings):
    aws_endpoint: str
    aws_region: str
    aws_s3_bucket: str


SETTINGS = Config()

TEST_CASE = [
    {"name": "4kb", "data": b"0" * 4096},
    {"name": "8kb", "data": b"0" * 8192},
    {"name": "16kb", "data": b"0" * 16384},
    {"name": "32kb", "data": b"0" * 32768},
    {"name": "256kb", "data": b"0" * 262144},
    {"name": "512kb", "data": b"0" * 524288},
    {"name": "16mb", "data": b"0" * 16777216},
]


async def opendal_write():
    op = opendal.AsyncOperator(
        "s3",
        bucket=SETTINGS.aws_s3_bucket,
        region=SETTINGS.aws_region,
        endpoint=SETTINGS.aws_endpoint,
    )
    tasks = [
        op.write(f"/benchmark/opendal_write/{case['name']}", case["data"])
        for case in TEST_CASE
    ]
    _ = await asyncio.gather(*tasks)


async def opendal_read():
    op = opendal.AsyncOperator(
        "s3",
        bucket=SETTINGS.aws_s3_bucket,
        region=SETTINGS.aws_region,
        endpoint=SETTINGS.aws_endpoint,
    )
    tasks = [op.read(f"/benchmark/opendal_write/{case['name']}") for case in TEST_CASE]
    _ = await asyncio.gather(*tasks)


def read_run():
    asyncio.run(opendal_read())


def write_run():
    asyncio.run(opendal_write())


def opendal_benchmark():
    print(f"OpenDAL S3 Client async write: {timeit.timeit(write_run, number=3)}")
    print(f"OpenDAL S3 Client async read: {timeit.timeit(read_run, number=3)}")


if __name__ == "__main__":
    opendal_benchmark()
