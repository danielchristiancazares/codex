"""Keep ownership of a synchronous request after its async caller cancels."""

import asyncio
import logging
import threading
from _thread import LockType
from collections.abc import Callable, Iterator
from concurrent.futures import Executor, Future
from contextlib import contextmanager
from contextvars import ContextVar, copy_context
from dataclasses import dataclass, field
from typing import TypeVar

from .models import JsonObject, JsonValue

ResultT = TypeVar("ResultT")
_LOGGER = logging.getLogger(__name__)
_LOCK_POLL_INTERVAL_S = 0.05


class _RequestCancelled(Exception):
    """The async caller withdrew this request before it could be written."""


@dataclass(slots=True)
class _RequestCancellation:
    cancelled: threading.Event = field(default_factory=threading.Event)
    accepted_turn: tuple[str, str] | None = None

    def check(self) -> None:
        if self.cancelled.is_set():
            raise _RequestCancelled()


_CURRENT_REQUEST: ContextVar[_RequestCancellation | None] = ContextVar(
    "codex_async_request", default=None
)


@contextmanager
def request_lock(lock: LockType) -> Iterator[None]:
    """Allow a withdrawn async request to leave existing lock queues promptly."""
    state = _CURRENT_REQUEST.get()
    if state is None:
        with lock:
            yield
        return

    while True:
        state.check()
        if lock.acquire(timeout=_LOCK_POLL_INTERVAL_S):
            break
    try:
        state.check()
        yield
    finally:
        lock.release()


def observe_response(method: str, params: JsonObject | None, result: JsonValue) -> None:
    """Keep accepted turn identity even if response-model validation later fails."""
    state = _CURRENT_REQUEST.get()
    if state is None or method != "turn/start" or params is None:
        return
    thread_id = params.get("threadId")
    turn = result.get("turn") if isinstance(result, dict) else None
    turn_id = turn.get("id") if isinstance(turn, dict) else None
    if isinstance(thread_id, str) and isinstance(turn_id, str):
        state.accepted_turn = thread_id, turn_id


async def call_cancellable(
    request: Callable[[], ResultT],
    cancel_turn: Callable[[str, str], None],
    *,
    executor: Executor | None = None,
    discard_result: Callable[[ResultT], None] | None = None,
) -> ResultT:
    state = _RequestCancellation()
    completed: Future[ResultT] = Future()

    def run_request() -> ResultT:
        token = _CURRENT_REQUEST.set(state)
        try:
            state.check()
            result = request()
        except BaseException as exc:
            completed.set_exception(exc)
            raise
        else:
            completed.set_result(result)
            return result
        finally:
            _CURRENT_REQUEST.reset(token)

    def clean_up(finished: Future[ResultT]) -> None:
        try:
            result = finished.result()
            if discard_result is not None:
                discard_result(result)
        except _RequestCancelled:
            return
        except BaseException:
            _LOGGER.exception("Cancelled SDK request failed before its result could be delivered")

        if state.accepted_turn is None:
            return
        thread_id, turn_id = state.accepted_turn

        def interrupt() -> None:
            token = _CURRENT_REQUEST.set(None)
            try:
                cancel_turn(thread_id, turn_id)
            except Exception:
                _LOGGER.exception(
                    "Failed to interrupt cancelled SDK turn %s in thread %s", turn_id, thread_id
                )
            finally:
                _CURRENT_REQUEST.reset(token)

        threading.Thread(target=interrupt, name="codex-turn-start-cleanup", daemon=True).start()

    def observe_task(task: asyncio.Future[ResultT]) -> None:
        if not task.cancelled():
            task.exception()

    task = asyncio.get_running_loop().run_in_executor(executor, copy_context().run, run_request)
    try:
        return await asyncio.shield(task)
    except asyncio.CancelledError:
        state.cancelled.set()
        # The completion observer, like goal-start cleanup, outlives the cancelled await.
        completed.add_done_callback(clean_up)
        task.add_done_callback(observe_task)
        raise
