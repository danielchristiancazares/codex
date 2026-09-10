import asyncio
import io
import json
import logging
import queue
import threading
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from types import SimpleNamespace

import pytest

from openai_codex import AsyncCodex, AsyncThread
from openai_codex._message_router import ResponseQueueItem
from openai_codex.async_client import AsyncCodexClient
from openai_codex.client import CodexClient
from openai_codex.errors import InvalidParamsError, ServerBusyError, TransportClosedError
from openai_codex.generated.v2_all import ModelListResponse, TurnStartResponse
from openai_codex.models import JsonObject, JsonValue

WAIT_TIMEOUT = 5.0
TURN_RESULT: JsonObject = {"turn": {"id": "accepted-turn", "items": [], "status": "inProgress"}}


class RecordingStdin(io.StringIO):
    """Capture writes below the real request, serialization, and transport locks."""

    def __init__(self, client: CodexClient, monkeypatch: pytest.MonkeyPatch) -> None:
        super().__init__()
        self.client = client
        self.requests: list[JsonObject] = []
        self.incoming: queue.Queue[JsonObject] = queue.Queue()
        self.on_request: Callable[[JsonObject], None] | None = None
        monkeypatch.setattr(client, "_proc", SimpleNamespace(stdin=self))

    def write(self, text: str) -> int:
        message = json.loads(text)
        assert isinstance(message, dict)
        self.requests.append(message)
        self.incoming.put(message)
        if self.on_request is not None:
            self.on_request(message)
        return len(text)

    async def next_request(self) -> JsonObject:
        return await asyncio.to_thread(self.incoming.get, True, WAIT_TIMEOUT)

    def respond(self, request: JsonObject, result: JsonValue) -> None:
        self.client._router.route_response({"id": request["id"], "result": result})

    def overload(self, request: JsonObject) -> None:
        self.client._router.route_response(
            {
                "id": request["id"],
                "error": {
                    "code": -32001,
                    "message": "Server overloaded",
                    "data": {"codex_error_info": "server_overloaded"},
                },
            }
        )

    def messages(self) -> list[JsonObject]:
        return [
            {"method": request["method"], "params": request["params"]} for request in self.requests
        ]


async def wait_for_event(event: threading.Event) -> None:
    assert await asyncio.to_thread(event.wait, WAIT_TIMEOUT), "worker did not reach its barrier"


@pytest.mark.parametrize("entry_point", ["public_turn", "stream_text"])
def test_cancelled_turn_waiting_for_same_thread_lock_is_not_written(
    monkeypatch: pytest.MonkeyPatch,
    entry_point: str,
) -> None:
    async def scenario() -> list[JsonObject]:
        codex = AsyncCodex()
        codex._initialized = True
        thread = AsyncThread(codex, "thread-1")
        client = codex._client._sync
        transport = RecordingStdin(client, monkeypatch)
        first = asyncio.create_task(thread.turn("first prompt"))
        first_request = await transport.next_request()
        entered = threading.Event()
        finished = threading.Event()
        original_lock = client._thread_start_lock

        @contextmanager
        def observed_lock(thread_id: str) -> Iterator[None]:
            entered.set()
            try:
                with original_lock(thread_id):
                    yield
            finally:
                finished.set()

        monkeypatch.setattr(client, "_thread_start_lock", observed_lock)

        def respond(request: JsonObject) -> None:
            result: JsonObject = (
                {"data": [], "nextCursor": None}
                if request["method"] == "model/list"
                else TURN_RESULT
            )
            transport.respond(request, result)
            if request["method"] == "turn/start":
                client._router.route_notification(
                    client._coerce_notification(
                        "item/agentMessage/delta",
                        {
                            "threadId": "thread-1",
                            "turnId": "accepted-turn",
                            "itemId": "item-1",
                            "delta": "unexpected output",
                        },
                    )
                )

        transport.on_request = respond
        operation = (
            thread.turn("withdrawn prompt")
            if entry_point == "public_turn"
            else anext(codex._client.stream_text("thread-1", "withdrawn prompt"))
        )
        cancelled = asyncio.create_task(operation)
        try:
            await wait_for_event(entered)
            cancelled.cancel()
            with pytest.raises(asyncio.CancelledError):
                await cancelled
            await wait_for_event(finished)
            models = await asyncio.wait_for(codex._client.model_list(), WAIT_TIMEOUT)
            assert models == ModelListResponse(data=[], next_cursor=None)
        finally:
            transport.respond(first_request, TURN_RESULT)
            await asyncio.wait_for(first, WAIT_TIMEOUT)
            await wait_for_event(finished)
        assert client._thread_start_locks == {}
        assert client._router._response_waiters == {}
        return transport.messages()

    assert asyncio.run(scenario()) == [
        {
            "method": "turn/start",
            "params": {
                "threadId": "thread-1",
                "input": [{"type": "text", "text": "first prompt"}],
            },
        },
        {"method": "model/list", "params": {"includeHidden": False}},
    ]


def test_cancelled_request_waiting_for_transport_lock_releases_response_waiter(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def scenario() -> list[JsonObject]:
        client = AsyncCodexClient()
        transport = RecordingStdin(client._sync, monkeypatch)
        registered = threading.Event()
        discarded = threading.Event()
        router = client._sync._router
        original_create = router.create_response_waiter
        original_discard = router.discard_response_waiter

        def create(request_id: str) -> queue.Queue[ResponseQueueItem]:
            waiter = original_create(request_id)
            registered.set()
            return waiter

        def discard(request_id: str) -> None:
            original_discard(request_id)
            discarded.set()

        monkeypatch.setattr(router, "create_response_waiter", create)
        monkeypatch.setattr(router, "discard_response_waiter", discard)
        try:
            with client._sync._lock:
                task = asyncio.create_task(client.turn_start("thread-1", "withdrawn"))
                await wait_for_event(registered)
                task.cancel()
                with pytest.raises(asyncio.CancelledError):
                    await task
                await wait_for_event(discarded)
        finally:
            router.fail_all(TransportClosedError("test transport closed"))
        assert router._response_waiters == {}
        return transport.messages()

    assert asyncio.run(scenario()) == []


def test_cancelled_overload_backoff_does_not_send_another_request(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import openai_codex.retry as retry_module

    requests: list[JsonObject] = []
    entered = threading.Event()
    release = threading.Event()

    def sync_sleep(_delay: float) -> None:
        entered.set()
        assert release.wait(WAIT_TIMEOUT), "legacy retry worker was not released"

    async def async_sleep(_delay: float) -> None:
        entered.set()
        await asyncio.Future()

    monkeypatch.setattr(retry_module.time, "sleep", sync_sleep)
    monkeypatch.setattr(asyncio, "sleep", async_sleep)

    async def scenario() -> None:
        client = AsyncCodexClient()
        transport = RecordingStdin(client._sync, monkeypatch)

        def respond(request: JsonObject) -> None:
            requests.append(request)
            if len(requests) == 1:
                transport.overload(request)
            else:
                transport.respond(request, TURN_RESULT)

        transport.on_request = respond
        task = asyncio.create_task(
            client.request_with_retry_on_overload(
                "turn/start",
                {"threadId": "thread-1", "input": [{"type": "text", "text": "prompt"}]},
                response_model=TurnStartResponse,
            )
        )
        try:
            await wait_for_event(entered)
            task.cancel()
            with pytest.raises(asyncio.CancelledError):
                await task
        finally:
            release.set()

    asyncio.run(scenario())
    assert [request["method"] for request in requests] == ["turn/start"]


@pytest.mark.parametrize(
    "entry_point",
    [
        "public_turn",
        "raw_request",
        "retry_request",
        "invalid_response",
        "stream_text",
        "mutated_request",
    ],
)
@pytest.mark.parametrize("accepted_before_cancel", [False, True])
def test_cancelled_start_interrupts_exact_accepted_turn_and_releases_routes(
    monkeypatch: pytest.MonkeyPatch,
    entry_point: str,
    accepted_before_cancel: bool,
) -> None:
    async def scenario() -> list[JsonObject]:
        codex = AsyncCodex()
        codex._initialized = True
        client = codex._client
        transport = RecordingStdin(client._sync, monkeypatch)
        registered = threading.Event()
        release = threading.Event()
        cleaned_up = threading.Event()
        original_register = client._sync.register_turn_notifications
        original_unregister = client._sync.unregister_turn_notifications
        original_request = client._sync._request_raw

        def register(turn_id: str) -> None:
            original_register(turn_id)
            registered.set()

        def unregister(turn_id: str) -> None:
            original_unregister(turn_id)
            cleaned_up.set()

        def request(method: str, params: JsonObject | None = None) -> JsonValue:
            result = original_request(method, params)
            if method == "turn/start" and accepted_before_cancel:
                registered.set()
                assert release.wait(WAIT_TIMEOUT), "accepted request worker was not released"
            return result

        monkeypatch.setattr(client._sync, "register_turn_notifications", register)
        monkeypatch.setattr(client._sync, "unregister_turn_notifications", unregister)
        monkeypatch.setattr(client._sync, "_request_raw", request)
        params: JsonObject = {
            "threadId": "thread-1",
            "input": [{"type": "text", "text": "prompt"}],
        }
        if entry_point == "public_turn":
            operation = AsyncThread(codex, "thread-1").turn("prompt")
        elif entry_point == "stream_text":
            operation = anext(client.stream_text("thread-1", "prompt"))
        elif entry_point in {"raw_request", "mutated_request"}:
            operation = client.request("turn/start", params, response_model=TurnStartResponse)
        elif entry_point == "invalid_response":
            operation = client.request("turn/start", params, response_model=ModelListResponse)
        else:
            operation = client.request_with_retry_on_overload(
                "turn/start", params, response_model=TurnStartResponse
            )
        task = asyncio.create_task(operation)
        start = await transport.next_request()
        if entry_point == "mutated_request":
            params["threadId"] = "different-thread"
        client._sync._router.route_notification(
            client._sync._coerce_notification(
                "item/agentMessage/delta",
                {
                    "threadId": "thread-1",
                    "turnId": "accepted-turn",
                    "itemId": "item-1",
                    "delta": "early output",
                },
            )
        )
        try:
            if accepted_before_cancel:
                transport.respond(start, TURN_RESULT)
                await wait_for_event(registered)
            task.cancel()
            with pytest.raises(asyncio.CancelledError):
                await task
            if not accepted_before_cancel:
                transport.respond(start, TURN_RESULT)
            release.set()
            interrupt = await transport.next_request()
            assert interrupt["method"] == "turn/interrupt"
            assert interrupt["params"] == {
                "threadId": "thread-1",
                "turnId": "accepted-turn",
            }
            transport.respond(interrupt, {})
            await wait_for_event(cleaned_up)
            assert {
                "waiters": client._sync._router._response_waiters,
                "turns": client._sync._router._turn_notifications,
                "pending": client._sync._router._pending_turn_notifications,
                "start_locks": client._sync._thread_start_locks,
            } == {"waiters": {}, "turns": {}, "pending": {}, "start_locks": {}}
            return transport.messages()
        finally:
            release.set()
            client._sync._router.fail_all(TransportClosedError("test transport closed"))

    assert [request["method"] for request in asyncio.run(scenario())] == [
        "turn/start",
        "turn/interrupt",
    ]


def test_cancelled_request_reports_transport_failure(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    async def scenario() -> list[JsonObject]:
        client = AsyncCodexClient()
        transport = RecordingStdin(client._sync, monkeypatch)
        task = asyncio.create_task(client.turn_start("thread-1", "prompt"))
        await transport.next_request()
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task
        client._sync._router.fail_all(TransportClosedError("transport lost during cancellation"))
        return transport.messages()

    assert [request["method"] for request in asyncio.run(scenario())] == ["turn/start"]
    assert "Cancelled SDK request failed" in caplog.text
    assert "transport lost during cancellation" in caplog.text


def test_cancelled_turn_cleanup_failure_is_reported_without_interrupting_another_turn(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    logged = threading.Event()
    logger = logging.getLogger("openai_codex._async_request")
    original_exception = logger.exception

    def record_failure(message: str, *args: object) -> None:
        original_exception(message, *args)
        logged.set()

    monkeypatch.setattr(logger, "exception", record_failure)

    async def scenario() -> list[JsonObject]:
        client = AsyncCodexClient()
        transport = RecordingStdin(client._sync, monkeypatch)
        task = asyncio.create_task(client.turn_start("thread-1", "prompt"))
        start = await transport.next_request()
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task
        transport.respond(start, TURN_RESULT)
        interrupt = await transport.next_request()
        client._sync._router.route_response(
            {
                "id": interrupt["id"],
                "error": {
                    "code": -32600,
                    "message": "expected active turn id `accepted-turn` but found `newer-turn`",
                },
            }
        )
        await wait_for_event(logged)
        assert {
            "waiters": client._sync._router._response_waiters,
            "turns": client._sync._router._turn_notifications,
            "pending": client._sync._router._pending_turn_notifications,
        } == {"waiters": {}, "turns": {}, "pending": {}}
        return transport.messages()

    assert [request["params"] for request in asyncio.run(scenario())] == [
        {"threadId": "thread-1", "input": [{"type": "text", "text": "prompt"}]},
        {"threadId": "thread-1", "turnId": "accepted-turn"},
    ]
    assert "Failed to interrupt cancelled SDK turn accepted-turn in thread thread-1" in caplog.text
    assert "newer-turn" in caplog.text


@pytest.mark.parametrize(
    ("failure_count", "error_code", "max_attempts", "attempts", "delays"),
    [
        (2, -32001, 3, 3, [0.25, 0.5]),
        (3, -32001, 2, 2, [0.25]),
        (1, -32602, 3, 1, []),
        (0, -32001, 0, 0, []),
    ],
)
def test_async_overload_retry_preserves_sync_policy_and_request_results(
    monkeypatch: pytest.MonkeyPatch,
    failure_count: int,
    error_code: int,
    max_attempts: int,
    attempts: int,
    delays: list[float],
) -> None:
    import openai_codex.retry as retry_module

    monkeypatch.setattr(retry_module.random, "uniform", lambda _low, _high: 0.0)

    def run(*, async_mode: bool) -> tuple[int, list[float], object]:
        client = AsyncCodexClient()
        transport = RecordingStdin(client._sync, monkeypatch)
        sleeps: list[float] = []

        def respond(request: JsonObject) -> None:
            if len(transport.requests) > failure_count:
                transport.respond(request, {"data": [], "nextCursor": None})
            else:
                client._sync._router.route_response(
                    {
                        "id": request["id"],
                        "error": {
                            "code": error_code,
                            "message": "rejected",
                            "data": (
                                {"codex_error_info": "server_overloaded"}
                                if error_code == -32001
                                else None
                            ),
                        },
                    }
                )

        async def async_sleep(delay: float) -> None:
            sleeps.append(delay)

        transport.on_request = respond
        monkeypatch.setattr(retry_module.time, "sleep", sleeps.append)
        monkeypatch.setattr(asyncio, "sleep", async_sleep)

        try:
            if async_mode:
                result = asyncio.run(
                    client.request_with_retry_on_overload(
                        "model/list",
                        None,
                        response_model=ModelListResponse,
                        max_attempts=max_attempts,
                    )
                )
            else:
                result = client._sync.request_with_retry_on_overload(
                    "model/list",
                    None,
                    response_model=ModelListResponse,
                    max_attempts=max_attempts,
                )
            outcome: object = result
        except (InvalidParamsError, ServerBusyError, ValueError) as exc:
            outcome = type(exc)
        assert len({request["id"] for request in transport.requests}) == len(transport.requests)
        return len(transport.requests), sleeps, outcome

    expected: object = (
        ValueError
        if max_attempts == 0
        else InvalidParamsError
        if error_code == -32602
        else ServerBusyError
        if failure_count >= max_attempts
        else ModelListResponse(data=[], next_cursor=None)
    )
    assert [run(async_mode=False), run(async_mode=True)] == [
        (attempts, delays, expected),
        (attempts, delays, expected),
    ]
