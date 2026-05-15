"""Retry management with key switching and error handling."""

import logging
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import TYPE_CHECKING

import httpx

from ollama_router.config import get_key_id
from ollama_router.handler import CooldownInfo, KeyAction, RateLimitHandler
from ollama_router.request_history import RequestHistory, RequestRecord
from ollama_router.state import KeySelector, KeyState, StateStore

if TYPE_CHECKING:
    from ollama_router.proxy import ProxyClient

logger = logging.getLogger("ollama_router")

MAX_RETRIES = 3


@dataclass
class RetryResult:
    """Result of retry execution."""

    response: httpx.Response | None
    success: bool
    attempts: int
    last_error: str | None = None


class RetryManager:
    """Handles retry loop with key switching and error handling."""

    def __init__(
        self,
        selector: KeySelector,
        handler: RateLimitHandler,
        state_store: StateStore,
        history: RequestHistory,
    ):
        self.selector = selector
        self.handler = handler
        self.state_store = state_store
        self.history = history

    def _sync_and_save(self):
        """Sync selector state to state store and persist."""
        self.state_store.current_index = self.selector.index
        self.state_store.last_failed_key = self.selector.last_failed_key
        self.state_store.save()

    async def execute_with_retry(
        self,
        method: str,
        path: str,
        headers: dict,
        body: dict | None,
        proxy: "ProxyClient",
        request_id: str,
    ) -> RetryResult:
        """Execute request with retry logic."""
        for attempt in range(MAX_RETRIES):
            selected_key = self.selector.select()
            if selected_key is None:
                logger.warning("all_keys_exhausted path=%s", path)
                return RetryResult(
                    response=None,
                    success=False,
                    attempts=attempt,
                    last_error="No available API keys",
                )

            headers["Authorization"] = f"Bearer {selected_key.key}"
            start_ts = time.perf_counter()

            try:
                response = await proxy.forward(
                    method=method,
                    path=path,
                    headers=headers,
                    json_data=body,
                )
            except Exception as e:
                latency = round((time.perf_counter() - start_ts) * 1000, 2)
                key_id = get_key_id(selected_key.key)
                logger.error(
                    "proxy_error key_id=%s path=%s attempt=%d/%d error_type=%s error=%r latency=%.2fms",
                    key_id,
                    path,
                    attempt + 1,
                    MAX_RETRIES,
                    type(e).__name__,
                    e,
                    latency,
                )
                self._record_request(
                    request_id=request_id,
                    method=method,
                    path=path,
                    status_code=502,
                    key_id=key_id,
                    latency=latency,
                )
                return RetryResult(
                    response=None,
                    success=False,
                    attempts=attempt + 1,
                    last_error=str(e),
                )

            latency = round((time.perf_counter() - start_ts) * 1000, 2)

            # Check for rate limit / auth error
            cooldown_info = self.handler.detect_cooldown(response)
            if cooldown_info:
                if cooldown_info.action == KeyAction.DISABLE:
                    self.selector.mark_disabled(
                        selected_key.key,
                        cooldown_info.reason,
                    )
                    self.selector.update_last_failed_key(selected_key.key)
                    self._sync_and_save()
                    logger.warning(
                        "key_disabled key_id=%s reason=%s attempt=%d/%d",
                        get_key_id(selected_key.key),
                        cooldown_info.reason,
                        attempt + 1,
                        MAX_RETRIES,
                    )
                    if attempt < MAX_RETRIES - 1:
                        continue

                    self._record_request(
                        request_id=request_id,
                        method=method,
                        path=path,
                        status_code=response.status_code,
                        key_id=get_key_id(selected_key.key),
                        latency=latency,
                    )
                    return RetryResult(
                        response=response,
                        success=False,
                        attempts=attempt + 1,
                        last_error=f"Key disabled: {cooldown_info.reason}",
                    )

                self.selector.mark_cooldown(
                    selected_key.key,
                    cooldown_info.hours,
                    cooldown_info.reason,
                )
                self.selector.update_last_failed_key(selected_key.key)
                self._sync_and_save()
                logger.info(
                    "key_cooldown key_id=%s reason=%s hours=%d attempt=%d/%d",
                    get_key_id(selected_key.key),
                    cooldown_info.reason,
                    cooldown_info.hours,
                    attempt + 1,
                    MAX_RETRIES,
                )
                if attempt < MAX_RETRIES - 1:
                    continue

                self._record_request(
                    request_id=request_id,
                    method=method,
                    path=path,
                    status_code=response.status_code,
                    key_id=get_key_id(selected_key.key),
                    latency=latency,
                )
                return RetryResult(
                    response=response,
                    success=False,
                    attempts=attempt + 1,
                    last_error="Rate limited",
                )

            # Success
            self.selector.update_last_failed_key(None)
            self.selector.last_used_key = selected_key.key
            logger.info(
                "request_done path=%s status=%d key_id=%s",
                path,
                response.status_code,
                get_key_id(selected_key.key),
            )
            self._record_request(
                request_id=request_id,
                method=method,
                path=path,
                status_code=response.status_code,
                key_id=get_key_id(selected_key.key),
                latency=latency,
            )
            return RetryResult(
                response=response,
                success=True,
                attempts=attempt + 1,
            )

        return RetryResult(
            response=None,
            success=False,
            attempts=MAX_RETRIES,
            last_error="Max retries exceeded",
        )

    def _record_request(
        self,
        request_id: str,
        method: str,
        path: str,
        status_code: int,
        key_id: str | None,
        latency: float,
    ) -> None:
        """Record a request to history."""
        record = RequestRecord(
            timestamp=datetime.now(timezone.utc),
            request_id=request_id,
            method=method,
            path=path,
            status_code=status_code,
            key_id=key_id,
            latency_ms=latency,
        )
        self.history.add(record)
