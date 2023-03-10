# Copyright 2022 Datafuse Labs
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
from typing import Iterable, Optional

class Error(Exception): ...

class Operator:
    def __init__(self, scheme: str, **kwargs): ...
    def read(self, path: str) -> bytes: ...
    def open_reader(self, path: str) -> Reader: ...
    def write(self, path: str, bs: bytes): ...
    def stat(self, path: str) -> Metadata: ...
    def create_dir(self, path: str): ...
    def delete(self, path: str): ...
    def list(self, path: str) -> Iterable[Entry]: ...
    def scan(self, path: str) -> Iterable[Entry]: ...

class AsyncOperator:
    def __init__(self, scheme: str, **kwargs): ...
    async def read(self, path: str) -> bytes: ...
    async def write(self, path: str, bs: bytes): ...
    async def stat(self, path: str) -> Metadata: ...
    async def create_dir(self, path: str): ...
    async def delete(self, path: str): ...

class Reader:
    def read(self, size: Optional[int] = None) -> bytes: ...
    def seek(self, offset: int, whence: int = 0) -> int: ...
    def tell(self) -> int: ...
    def __enter__(self) -> Reader: ...
    def __exit__(self, exc_type, exc_value, traceback) -> None: ...

class Entry:
    @property
    def path(self) -> str: ...

class Metadata:
    @property
    def content_disposition(self) -> Optional[str]: ...
    @property
    def content_length(self) -> int: ...
    @property
    def content_md5(self) -> Optional[str]: ...
    @property
    def content_type(self) -> Optional[str]: ...
    @property
    def etag(self) -> Optional[str]: ...
    @property
    def mode(self) -> EntryMode: ...

class EntryMode:
    def is_file(self) -> bool: ...
    def is_dir(self) -> bool: ...
