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

from datetime import datetime
from typing import Final, final

@final
class Entry:
    """
    Entry.

    An entry representing a path and its associated metadata.

    Notes
    -----
    If this entry is a directory, ``path`` **must** end with ``/``.
    Otherwise, ``path`` **must not** end with ``/``.
    """

    @property
    def metadata(self, /) -> Metadata:
        """The metadata of this entry."""
    @property
    def name(self, /) -> str:
        """The name of entry, representing the last segment of the path."""
    @property
    def path(self, /) -> str:
        """The path of entry relative to the operator's root."""

@final
class EntryMode:
    """
    EntryMode.

    The mode of an entry, indicating if it is a file or a directory.
    """

    Dir: Final[EntryMode]
    """
    The entry is a directory and can be listed.
    """
    File: Final[EntryMode]
    """
    The entry is a file and has data to read.
    """
    Unknown: Final[EntryMode]
    """
    The mode of the entry is unknown.
    """
    def __eq__(self, value: object, /) -> bool: ...
    def __hash__(self, /) -> int: ...
    def __int__(self, /) -> int: ...
    def __ne__(self, value: object, /) -> bool: ...
    def is_dir(self, /) -> bool:
        """
        Check if the entry mode is `Dir`.

        Returns
        -------
        bool
            True if the entry is a directory.
        """
    def is_file(self, /) -> bool:
        """
        Check if the entry mode is `File`.

        Returns
        -------
        bool
            True if the entry is a file.
        """

@final
class Metadata:
    """
    The metadata of an ``Entry``.

    The metadata is always tied to a specific context and is not a global
    state. For example, two versions of the same path might have different
    content lengths.

    Notes
    -----
    In systems that support versioning, such as AWS S3, the metadata may
    represent a specific version of a file. Use :attr:`version` to get
    the version of a file if it is available.
    """

    @property
    def content_disposition(self, /) -> str | None:
        """The content disposition of this entry."""
    @property
    def content_encoding(self, /) -> str | None:
        """The content encoding of this entry."""
    @property
    def content_length(self, /) -> int:
        """The content length of this entry."""
    @property
    def content_md5(self, /) -> str | None:
        """The content MD5 of this entry."""
    @property
    def content_type(self, /) -> str | None:
        """The content type of this entry."""
    @property
    def etag(self, /) -> str | None:
        """The ETag of this entry."""
    @property
    def is_dir(self, /) -> bool:
        """Whether this entry is a directory."""
    @property
    def is_file(self, /) -> bool:
        """Whether this entry is a file."""
    @property
    def last_modified(self, /) -> datetime | None:
        """The last modified timestamp of this entry."""
    @property
    def mode(self, /) -> EntryMode:
        """The mode of this entry."""
    @property
    def user_metadata(self, /) -> dict[str, str] | None:
        """The user-defined metadata of this entry."""
    @property
    def version(self, /) -> str | None:
        """The version of this entry."""

@final
class PresignedRequest:
    """
    A presigned request.

    This contains the information required to make a request to the
    underlying service, including the URL, method, and headers.
    """

    @property
    def headers(self, /) -> dict[str, str]:
        """
        The HTTP headers of this request.

        Returns
        -------
        dict
            The HTTP headers of this request.
        """
    @property
    def method(self, /) -> str:
        """The HTTP method of this request."""
    @property
    def url(self, /) -> str:
        """The URL of this request."""
