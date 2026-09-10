from __future__ import annotations

import asyncio
import random
import time
from typing import Awaitable, Callable, TypeVar

from .errors import is_retryable_error

T = TypeVar("T")


def retry_on_overload(
    op: Callable[[], T],
    *,
    max_attempts: int = 3,
    initial_delay_s: float = 0.25,
    max_delay_s: float = 2.0,
    jitter_ratio: float = 0.2,
) -> T:
    """Retry helper for transient server-overload errors."""

    if max_attempts < 1:
        raise ValueError("max_attempts must be >= 1")

    delay = initial_delay_s
    attempt = 0
    while True:
        attempt += 1
        try:
            return op()
        except Exception as exc:
            if attempt >= max_attempts:
                raise
            if not is_retryable_error(exc):
                raise

            sleep_for = _retry_delay(delay, max_delay_s, jitter_ratio)
            if sleep_for > 0:
                time.sleep(sleep_for)
            delay = min(max_delay_s, delay * 2)


async def _retry_on_overload_async(
    op: Callable[[], Awaitable[T]],
    *,
    max_attempts: int = 3,
    initial_delay_s: float = 0.25,
    max_delay_s: float = 2.0,
    jitter_ratio: float = 0.2,
) -> T:
    if max_attempts < 1:
        raise ValueError("max_attempts must be >= 1")

    delay = initial_delay_s
    attempt = 0
    while True:
        attempt += 1
        try:
            return await op()
        except Exception as exc:
            if attempt >= max_attempts or not is_retryable_error(exc):
                raise
            sleep_for = _retry_delay(delay, max_delay_s, jitter_ratio)
            if sleep_for > 0:
                await asyncio.sleep(sleep_for)
            delay = min(max_delay_s, delay * 2)


def _retry_delay(delay: float, max_delay: float, jitter_ratio: float) -> float:
    jitter = delay * jitter_ratio
    return min(max_delay, delay) + random.uniform(-jitter, jitter)
