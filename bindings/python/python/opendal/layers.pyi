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

from typing import final

@final
class CapabilityOverrideLayer(Layer):
    """A layer that overrides the full capability exposed by an operator."""

    def __new__(cls, /, overrides: str) -> CapabilityOverrideLayer:
        """
        Create a new CapabilityOverrideLayer from capability override entries.

        Parameters
        ----------
        overrides : str
            Comma-separated capability override entries.

        Returns
        -------
        CapabilityOverrideLayer
        """

@final
class ConcurrentLimitLayer(Layer):
    """
    A layer that limits the number of concurrent operations.

    Notes
    -----
    All operators wrapped by this layer will share a common semaphore. This
    allows you to reuse the same layer across multiple operators, ensuring
    that the total number of concurrent requests across the entire
    application does not exceed the limit.
    """

    def __new__(cls, /, limit: int) -> ConcurrentLimitLayer:
        """
        Create a new ConcurrentLimitLayer.

        Parameters
        ----------
        limit : int
            Maximum number of concurrent operations allowed.

        Returns
        -------
        ConcurrentLimitLayer
        """

class Layer:
    """
    Base class for all layers.

    A layer wraps an operator to intercept operations on the underlying
    storage, adding behavior such as retries, timeouts, concurrency limits,
    or content-type detection.

    This class is not meant to be instantiated directly. Use a concrete
    subclass such as `RetryLayer` or `TimeoutLayer` and apply it with
    `Operator.layer` or `AsyncOperator.layer`.
    """

@final
class MimeGuessLayer(Layer):
    """
    A layer that guesses MIME types for objects based on their paths or content.

    This layer uses the `mime_guess` crate
    (see https://crates.io/crates/mime_guess) to infer the
    ``Content-Type``.

    Notes
    -----
    This layer will not override a ``Content-Type`` that has already
    been set, either manually or by the backend service. It is only
    applied if no content type is present.

    A ``Content-Type`` is not guaranteed. If the file extension is
    uncommon or unknown, the content type will remain unset.
    """

    def __new__(cls, /) -> MimeGuessLayer:
        """
        Create a new MimeGuessLayer.

        Returns
        -------
        MimeGuessLayer
        """

@final
class RetryLayer(Layer):
    """
    A layer that retries operations that fail with temporary errors.

    Operations are retried if they fail with an error for which
    `Error.is_temporary` returns `True`. If all retries are exhausted,
    the error is marked as persistent and then returned.

    Notes
    -----
    After an operation on a `Reader` or `Writer` has failed through
    all retries, the object is in an undefined state. Reusing it
    can lead to exceptions.
    """

    def __new__(
        cls,
        /,
        max_times: int | None = None,
        factor: float | None = None,
        jitter: bool = False,
        max_delay: float | None = None,
        min_delay: float | None = None,
    ) -> RetryLayer:
        """
        Create a new RetryLayer.

        Parameters
        ----------
        max_times : Optional[int]
            Maximum number of retry attempts. Defaults to ``3``.
        factor : Optional[float]
            Backoff factor applied between retries. Must be a finite value
            ``>= 1.0``. Defaults to ``2.0``.
        jitter : bool
            Whether to apply jitter to the backoff. Defaults to ``False``.
        max_delay : Optional[float]
            Maximum delay (in seconds) between retries. Must be finite and
            non-negative. Defaults to ``60.0``.
        min_delay : Optional[float]
            Minimum delay (in seconds) between retries. Must be finite and
            non-negative. Defaults to ``1.0``.

        Returns
        -------
        RetryLayer

        Raises
        ------
        ConfigInvalid
            If ``factor``, ``max_delay``, or ``min_delay`` is out of range.
        """

@final
class TimeoutLayer(Layer):
    """
    A layer that adds timeouts to operations.

    Timeouts prevent slow or stalled work from hanging indefinitely, for
    example when a TCP connection stops emitting IO events. Control
    operations (such as `stat` and `delete`) and IO operations (such as
    `read` and `write`) are bounded by separate timeouts.

    Notes
    -----
    A small amount of overhead is added to IO operations to implement the
    timeout correctly.
    """

    def __new__(
        cls, /, timeout: float | None = None, io_timeout: float | None = None
    ) -> TimeoutLayer:
        """
        Create a new TimeoutLayer.

        Parameters
        ----------
        timeout : Optional[float]
            Timeout (in seconds) for control operations like ``stat`` and
            ``delete``. Must be a positive, finite number. A value of ``0``
            is rejected because it would make every operation time out
            immediately. Defaults to ``60.0``.
        io_timeout : Optional[float]
            Timeout (in seconds) for IO operations like ``read`` and
            ``write``. Must be a positive, finite number. A value of ``0``
            is rejected because it would make every operation time out
            immediately. Defaults to ``10.0``.

        Returns
        -------
        TimeoutLayer

        Raises
        ------
        ConfigInvalid
            If ``timeout`` or ``io_timeout`` is out of range.
        """
