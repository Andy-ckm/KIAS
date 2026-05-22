python
#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
resilience_manager.py
---------------------

A **unified resilience decorator** that combines four core resilience patterns:

* Circuit‑Breaker
* Retry (with exponential back‑off)
* Bulkhead (semaphore‑based concurrency limiter)
* Timeout (wall‑clock limit)

All patterns are optional and can be configured independently.  The decorator
is safe to use in a multi‑threaded environment (the shared state of the
circuit‑breaker is protected by a lock).  The implementation follows the
same mental model used in popular Java/Go libraries (e.g. Hystrix, go-resilience).

Usage example
~~~~~~~~~~~~~~

    >>> from resilience_manager import ResilienceConfig, ResilienceManager
    >>> cfg = ResilienceConfig(
    ...     retry={"max_attempts": 3, "backoff_base": 0.1},
    ...     circuit_breaker={"failure_threshold": 5, "reset_timeout": 30},
    ...     bulkhead={"max_concurrent_calls": 4},
    ...     timeout={"timeout_seconds": 1.5}
    ... )
    >>> rm = ResilienceManager(cfg)
    >>> @rm.decorate
    ... def unreliable_service(x):
    ...     # calls an external API that may time‑out or raise
    ...     ...
"""

from __future__ import annotations

import concurrent.futures
import logging
import random
import threading
import time
from dataclasses import dataclass, field
from enum import Enum
from functools import wraps
from typing import Any, Callable, Dict, Optional, Sequence, Tuple, Type

__all__ = [
    "CircuitState",
    "CircuitBreaker",
    "RetryPolicy",
    "BulkheadPolicy",
    "TimeoutPolicy",
    "ResilienceConfig",
    "ResilienceManager",
    "resilience",
]

# ----------------------------------------------------------------------
# Logging
# ----------------------------------------------------------------------
logger = logging.getLogger("resilience")
logger.addHandler(logging.NullHandler())


# ----------------------------------------------------------------------
# 1️⃣ Circuit Breaker
# ----------------------------------------------------------------------
class CircuitState(Enum):
    """Possible states of the circuit breaker."""
    CLOSED = "closed"       # normal operation
    OPEN = "open"           # reject calls immediately
    HALF_OPEN = "half_open"  # allow a single test call


@dataclass
class CircuitBreaker:
    """
    A thread‑safe implementation of the Circuit Breaker pattern.

    Parameters
    ----------
    failure_threshold: int
        Number of consecutive failures required to **open** the circuit.
    reset_timeout: float
        Seconds to wait before attempting a **half‑open** transition.
    half_open_max_calls: int
        How many calls are allowed in *half_open* state (default = 1).
    expected_exceptions: Sequence[Type[Exception]]
        Subset of exceptions that count towards the failure counter.
        All other exceptions are ignored (they may be business‑level).
    """
    failure_threshold: int = 5
    reset_timeout: float = 30.0
    half_open_max_calls: int = 1
    expected_exceptions: Sequence[Type[Exception]] = (Exception,)

    # internal state
    _state: CircuitState = field(default=CircuitState.CLOSED, repr=False)
    _failure_count: int = field(default=0, repr=False)
    _last_failure_time: float = field(default=0.0, repr=False)
    _half_open_calls: int = field(default=0, repr=False)
    _lock: threading.Lock = field(default=threading.Lock, repr=False)

    # ------------------------------------------------------------------
    # Public helpers
    # ------------------------------------------------------------------
    def record_success(self) -> None:
        """Reset the breaker to CLOSED after a successful call."""
        with self._lock:
            self._failure_count = 0
            self._state = CircuitState.CLOSED

    def record_failure(self) -> None:
        """Record a failure and possibly open the circuit."""
        with self._lock:
            self._failure_count += 1
            self._last_failure_time = time.monotonic()
            if self._failure_count >= self.failure_threshold:
                self._state = CircuitState.OPEN
                logger.warning(
                    "CircuitBreaker opened after %d failures",
                    self._failure_count,
                )

    @property
    def state(self) -> CircuitState:
        """Return the current circuit state, checking for automatic half-open."""
        with self._lock:
            self._maybe_transition()
            return self._state

    def allow_request(self) -> bool:
        """Return True if the current request is allowed through the breaker."""
        with self._lock:
            self._maybe_transition()
            if self._state == CircuitState.OPEN:
                return False
            if self._state == CircuitState.HALF_OPEN:
                if self._half_open_calls < self.half_open_max_calls:
                    self._half_open_calls += 1
                    return True
                return False
            # CLOSED
            return True

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------
    def _maybe_transition(self) -> None:
        """Check elapsed time and update state if needed."""
        if self._state == CircuitState.OPEN:
            elapsed = time.monotonic() - self._last_failure_time
            if elapsed >= self.reset_timeout:
                logger.debug("CircuitBreaker entering HALF_OPEN after %.2fs", elapsed)
                self._state = CircuitState.HALF_OPEN
                self._half_open_calls = 0
        elif self._state == CircuitState.HALF_OPEN:
            # If we are still half-open we just keep the counter; no auto‑close.
            pass


# ----------------------------------------------------------------------
# 2️⃣ Retry
# ----------------------------------------------------------------------
@dataclass
class RetryPolicy:
    """
    Configuration for automatic retry with exponential back‑off.

    Parameters
    ----------
    max_attempts: int
        Maximum number of **total** attempts (including the first call).
    backoff_base: float
        Base for exponential back‑off (in seconds).  The *n*‑th retry sleeps
        ``backoff_base * (2 ** (n-1))`` seconds, optionally jittered.
    backoff_max: float
        Upper bound on the sleep time (in seconds).
    jitter: bool
        If True, add a random jitter in [0, backoff] to the sleep time.
    retriable_exceptions: Sequence[Type[Exception]]
        Exceptions that trigger a retry.  Sub‑classes of these are also retried.
    """
    max_attempts: int = 3
    backoff_base: float = 0.1
    backoff_max: float = 30.0
    jitter: bool = True
    retriable_exceptions: Sequence[Type[Exception]] = (Exception,)

    @staticmethod
    def is_retriable(exc: Exception, policy: RetryPolicy) -> bool:
        """Return True if *exc* matches any retriable exception type."""
        return any(isinstance(exc, exc_type) for exc_type in policy.retriable_exceptions)

    @staticmethod
    def compute_delay(attempt: int, policy: RetryPolicy) -> float:
        """Calculate sleep time for the given attempt index."""
        delay = policy.backoff_base * (2 ** (attempt - 1))
        if policy.jitter:
            delay += random.random() * delay
        return min(delay, policy.backoff_max)


# ----------------------------------------------------------------------
# 3️⃣ Bulkhead
# ----------------------------------------------------------------------
@dataclass
class BulkheadPolicy:
    """
    Concurrency‑limit (semaphore) policy.

    Parameters
    ----------
    max_concurrent_calls: int
        Maximum number of simultaneous executions that may run.
    """
    max_concurrent_calls: int = 5

    _semaphore: threading.Semaphore = field(init=False, repr=False)

    def __post_init__(self) -> None:
        self._semaphore = threading.Semaphore(self.max_concurrent_calls)

    def acquire(self) -> threading.Lock:
        """
        Acquire the semaphore. Returns a context‑manager that releases on exit.

        Example
        -------
            with bulkhead.acquire():
                # only one thread here at a time
                ...
        """
        return _BulkheadContext(self._semaphore)


class _BulkheadContext:
    __slots__ = ("_sem",)

    def __init__(self, sem: threading.Semaphore) -> None:
        self._sem = sem

    def __enter__(self) -> None:
        self._sem.acquire()

    def __exit__(self, *_: Any) -> None:
        self._sem.release()


# ----------------------------------------------------------------------
# 4️⃣ Timeout
# ----------------------------------------------------------------------
@dataclass
class TimeoutPolicy:
    """
    Wall‑clock timeout policy.

    Parameters
    ----------
    timeout_seconds: float
        Upper bound (in seconds) for a single call.  When exceeded a
        ``TimeoutError`` is raised.
    """
    timeout_seconds: float = 5.0


def _run_with_timeout(
    func: Callable[..., Any],
    args: Tuple[Any, ...],
    kwargs: Dict[str, Any],
    timeout_seconds: float,
) -> Any:
    """
    Execute *func*(*args, **kwargs) inside a one‑off ``ThreadPoolExecutor``
    and raise ``TimeoutError`` if the call does not finish within
    *timeout_seconds*.

    Note
    ----
    This is a simple, cross‑platform solution that works in both
    single‑threaded and multi‑threaded programs.  The underlying thread
    continues to run until the function returns, but the caller will see a
    ``TimeoutError`` immediately after the timeout.
    """
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
        future = executor.submit(func, *args, **kwargs)
        try:
            return future.result(timeout=timeout_seconds)
        except concurrent.futures.TimeoutError:
            raise TimeoutError(
                f"Call to {func.__name__!r} exceeded the timeout of {timeout_seconds}s"
            )


# ----------------------------------------------------------------------
# 5️⃣ Resilience configuration
# ----------------------------------------------------------------------
@dataclass
class ResilienceConfig:
    """
    Aggregated configuration for the ``ResilienceManager``.

    All fields are optional.  A field set to ``None`` disables the associated
    policy (i.e. no circuit breaker, no retry, etc.).
    """
    retry: Optional[RetryPolicy] = field(default_factory=RetryPolicy)
    circuit_breaker: Optional[CircuitBreaker] = field(default_factory=CircuitBreaker)
    bulkhead: Optional[BulkheadPolicy] = field(default_factory=BulkheadPolicy)
    timeout: Optional[TimeoutPolicy] = field(default_factory=TimeoutPolicy)


# ----------------------------------------------------------------------
# 6️⃣ Core Resilience Manager
# ----------------------------------------------------------------------
class ResilienceManager:
    """
    A unified resilience decorator factory.

    The manager receives a ``ResilienceConfig`` that determines which
    policies are active and how they are tuned.  The ``decorate`` method
    returns a wrapper that executes the target callable through the
    following chain:

    1. **Bulkhead** – acquire a semaphore slot.
    2. **Circuit‑Breaker** – check whether the breaker allows the request.
    3. **Retry** – attempt the call repeatedly on allowed exceptions.
    4. **Timeout** – enforce a wall‑clock limit.

    Each step is optional (config ``None`` disables it).

    Example
    -------
        >>> cfg = ResilienceConfig(
        ...     retry=RetryPolicy(max_attempts=3),
        ...     circuit_breaker=CircuitBreaker(failure_threshold=5),
        ...     bulkhead=BulkheadPolicy(max_concurrent_calls=10),
        ...     timeout=TimeoutPolicy(timeout_seconds=2.0)
        ... )
        >>> manager = ResilienceManager(cfg)
        >>> @manager.decorate
        ... def risky_function(x):
        ...     ...

    The decorator can also be applied without parentheses when the default
    configuration (all policies enabled with sensible defaults) is sufficient:

        >>> @ResilienceManager()          # uses default config
        ... def my_func(): ...
    """

    # Default configuration – all policies are enabled.
    DEFAULT_CONFIG: ResilienceConfig = ResilienceConfig()

    def __init__(self, config: Optional[ResilienceConfig] = None) -> None:
        self.config = config if config is not None else self.DEFAULT_CONFIG

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------
    def decorate(self, func: Callable[..., Any]) -> Callable[..., Any]:
        """
        Wrap *func* with the resilience pipeline and return the wrapper.
        """
        @wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            return self._execute_pipeline(func, args, kwargs)

        return wrapper

    def __call__(self, func: Callable[..., Any]) -> Callable[..., Any]:
        """
        Allow the manager to be used as a bare decorator::

            @ResilienceManager()
            def foo(): ...
        """
        return self.decorate(func)

    # ------------------------------------------------------------------
    # Private execution pipeline
    # ------------------------------------------------------------------
    def _execute_pipeline(
        self,
        func: Callable[..., Any],
        args: Tuple[Any, ...],
        kwargs: Dict[str, Any],
    ) -> Any:
        # 1️⃣ Bulkhead (outermost) – limits concurrency
        bulkhead_ctx = (
            self.config.bulkhead.acquire()
            if self.config.bulkhead is not None
            else _NoOpContext()
        )

        with bulkhead_ctx:
            # 2️⃣ Circuit‑breaker – short‑circuit if open
            if self.config.circuit_breaker is not None:
                if not self.config.circuit_breaker.allow_request():
                    raise CircuitBreakerOpen(
                        "Circuit breaker is OPEN; request rejected"
                    )

            # 3️⃣ Retry + Timeout – the core work is performed here
            if self.config.retry is not None and self.config.timeout is not None:
                return self._call_with_retry_and_timeout(func, args, kwargs)
            elif self.config.retry is not None:
                return self._call_with_retry(func, args, kwargs)
            elif self.config.timeout is not None:
                return self._call_with_timeout(func, args, kwargs)
            else:
                # No retry nor timeout – plain call
                return func(*args, **kwargs)

    # ------------------------------------------------------------------
    # Helper wrappers for each combination
    # ------------------------------------------------------------------
    def _call_with_retry_and_timeout(
        self,
        func: Callable[..., Any],
        args: Tuple[Any, ...],
        kwargs: Dict[str, Any],
    ) -> Any:
        """Combine retry (inner) with timeout (outermost of the retry loop)."""
        retry_cfg = self.config.retry
        assert retry_cfg is not None
        timeout_cfg = self.config.timeout
        assert timeout_cfg is not None

        attempt = 0
        last_exc: Optional[Exception] = None

        while attempt < retry_cfg.max_attempts:
            attempt += 1
            try:
                # Wrap each attempt with timeout
                return _run_with_timeout(
                    func, args, kwargs, timeout_cfg.timeout_seconds
                )
            except TimeoutError as exc:
                # A timeout is treated as a failure and may be retried
                last_exc = exc
                logger.debug(
                    "Attempt %d/%d timed out (%s)",
                    attempt,
                    retry_cfg.max_attempts,
                    exc,
                )
                self._record_failure_on_circuit_breaker(exc)
                if not self._should_retry(attempt, exc):
                    break
                self._sleep_before_retry(attempt, retry_cfg)
                continue
            except Exception as exc:
                last_exc = exc
                logger.debug(
                    "Attempt %d/%d raised %r",
                    attempt,
                    retry_cfg.max_attempts,
                    exc,
                )
                self._record_failure_on_circuit_breaker(exc)
                if not RetryPolicy.is_retriable(exc, retry_cfg):
                    raise  # non‑retriable exception propagates immediately
                if not self._should_retry(attempt, exc):
                    break
                self._sleep_before_retry(attempt, retry_cfg)

        # All attempts exhausted
        raise last_exc or Exception("ResilienceManager: all retries exhausted")

    def _call_with_retry(
        self,
        func: Callable[..., Any],
        args: Tuple[Any, ...],
        kwargs: Dict[str, Any],
    ) -> Any:
        """Plain retry without timeout."""
        retry_cfg = self.config.retry
        assert retry_cfg is not None

        attempt = 0
        last_exc: Optional[Exception] = None

        while attempt < retry_cfg.max_attempts:
            attempt += 1
            try:
                return func(*args, **kwargs)
            except Exception as exc:
                last_exc = exc
                self._record_failure_on_circuit_breaker(exc)
                if not RetryPolicy.is_retriable(exc, retry_cfg):
                    raise
                if not self._should_retry(attempt, exc):
                    break
                self._sleep_before_retry(attempt, retry_cfg)

        raise last_exc or Exception("ResilienceManager: all retries exhausted")

    def _call_with_timeout(
        self,
        func: Callable[..., Any],
        args: Tuple[Any, ...],
        kwargs: Dict[str, Any],
    ) -> Any:
        """Single call with timeout (no retry)."""
        timeout_cfg = self.config.timeout
        assert timeout_cfg is not None

        try:
            return _run_with_timeout(func, args, kwargs, timeout_cfg.timeout_seconds)
        except TimeoutError as exc:
            self._record_failure_on_circuit_breaker(exc)
            raise

    # ------------------------------------------------------------------
    # Helper utilities
    # ------------------------------------------------------------------
    def _should_retry(self, attempt: int, exc: Exception) -> bool:
        """Decide whether another attempt is worthwhile."""
        retry_cfg = self.config.retry
        assert retry_cfg is not None
        return attempt < retry_cfg.max_attempts

    def _sleep_before_retry(self, attempt: int, retry_cfg: RetryPolicy) -> None:
        """Sleep for the computed back‑off delay."""
        delay = RetryPolicy.compute_delay(attempt, retry_cfg)
        logger.debug("Sleeping %.3fs before retry %d", delay, attempt + 1)
        time.sleep(delay)

    def _record_failure_on_circuit_breaker(self, exc: Exception) -> None:
        """Record the failure in the circuit breaker if one is configured."""
        if self.config.circuit_breaker is None:
            return
        # Only record expected exceptions
        if any(isinstance(exc, exc_type) for exc_type in self.config.circuit_breaker.expected_exceptions):
            self.config.circuit_breaker.record_failure()


class CircuitBreakerOpen(Exception):
    """Raised when a request is rejected because the circuit breaker is OPEN."""
    pass


class _NoOpContext:
    """A dummy context manager that does nothing – used when bulkhead is disabled."""
    __slots__ = ()

    def __enter__(self) -> None:
        pass

    def __exit__(self, *_: Any) -> None:
        pass


# ----------------------------------------------------------------------
# 7️⃣ Decorator convenience function
# ----------------------------------------------------------------------
def resilience(
    func: Optional[Callable[..., Any]] = None,
    *,
    config: Optional[ResilienceConfig] = None,
) -> Callable[..., Any] | Callable[[Callable[..., Any]], Callable[..., Any]]:
    """
    Lightweight decorator entry point.

    Can be used with or without parentheses::

        @resilience
        def a(): ...

        @resilience(config=ResilienceConfig(...))
        def b(): ...

    Parameters
    ----------
    func: Callable, optional
        The callable to wrap (provided automatically when used as a decorator
        without parentheses).
    config: ResilienceConfig, optional
        The configuration to use.  If omitted the default configuration
        (all policies enabled) is used.
    """
    if func is not None:
        # Used as a bare decorator without parentheses
        return ResilienceManager().decorate(func)

    # Return a decorator factory when called with parentheses
    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        manager = ResilienceManager(config)
        return manager.decorate(fn)

    return decorator


# ----------------------------------------------------------------------
# 8️⃣ Unit Tests
# ----------------------------------------------------------------------
import unittest


class TestRetryPolicy(unittest.TestCase):
    """Test the retry logic in isolation."""

    def test_retry_succeeds_on_second_attempt(self):
        call_count = 0

        @resilience(config=ResilienceConfig(
            retry=RetryPolicy(max_attempts=3, backoff_base=0.01, jitter=False)
        ))
        def flaky():
            nonlocal call_count
            call_count += 1
            if call_count < 2:
                raise ValueError("Transient error")
            return "ok"

        result = flaky()
        self.assertEqual(result, "ok")
        self.assertEqual(call_count, 2)

    def test_retry_exhaustion_raises(self):
        @resilience(config=ResilienceConfig(
            retry=RetryPolicy(max_attempts=2, backoff_base=0.01, jitter=False)
        ))
        def always_fails():
            raise RuntimeError("Permanent failure")

        with self.assertRaises(RuntimeError) as cm:
            always_fails()
        self.assertIn("Permanent failure", str(cm.exception))


class TestCircuitBreaker(unittest.TestCase):
    """Test the circuit breaker behaviour."""

    def test_circuit_opens_after_threshold(self):
        failure_counter = 0

        @resilience(config=ResilienceConfig(
            circuit_breaker=CircuitBreaker(
                failure_threshold=3,
                reset_timeout=60,
                expected_exceptions=(RuntimeError,)
            ),
            retry=RetryPolicy(max_attempts=1)  # disable retry for clarity
        ))
        def failing():
            nonlocal failure_counter
            failure_counter += 1
            raise RuntimeError("boom")

        # First three calls should raise (the decorator will let them through)
        for _ in range(3):
            with self.assertRaises(RuntimeError):
                failing()

        # Circuit should now be OPEN – next call is rejected without invoking `failing`
        with self.assertRaises(CircuitBreakerOpen):
            failing()

        self.assertEqual(failure_counter, 3)

    def test_circuit_half_open_after_cooldown(self):
        call_counter = 0

        cb_cfg = CircuitBreaker(
            failure_threshold=2,
            reset_timeout=0.2,   # very short so test runs fast
            expected_exceptions=(RuntimeError,)
        )

        @resilience(config=ResilienceConfig(
            circuit_breaker=cb_cfg,
            retry=RetryPolicy(max_attempts=1)
        ))
        def sometimes_failing():
            nonlocal call_counter
            call_counter += 1
            if call_counter <= 2:
                raise RuntimeError("boom")
            return "recovered"

        # Trigger opening
        for _ in range(2):
            with self.assertRaises(RuntimeError):
                sometimes_failing()

        # Circuit should be OPEN now
        with self.assertRaises(CircuitBreakerOpen):
            sometimes_failing()

        # Wait for half‑open transition
        time.sleep(0.25)

        # Next call should be allowed (half‑open) – it should succeed
        result = sometimes_failing()
        self.assertEqual(result, "recovered")

        # After a successful call the breaker resets to CLOSED
        # Another failing call should again be rejected after threshold
        @resilience(config=ResilienceConfig(
            circuit_breaker=CircuitBreaker(
                failure_threshold=2,
                reset_timeout=60,
                expected_exceptions=(RuntimeError,)
            ),
            retry=RetryPolicy(max_attempts=1)
        ))
        def failing2():
            raise RuntimeError("boom")

        for _ in range(2):
            with self.assertRaises(RuntimeError):
                failing2()
        with self.assertRaises(CircuitBreakerOpen):
            failing2()


class TestBulkhead(unittest.TestCase):
    """Test concurrency limiting via bulkhead."""

    def test_bulkhead_blocks_excess_concurrent_calls(self):
        active = 0
        max_active = 0
        lock = threading.Lock()
        barrier = threading.Barrier(2)  # keep both threads alive until both entered

        @resilience(config=ResilienceConfig(
            bulkhead=BulkheadPolicy(max_concurrent_calls=1),
            retry=RetryPolicy(max_attempts=1)
        ))
        def slow_work():
            nonlocal active, max_active
            with lock:
                active += 1
                max_active = max(max_active, active)
            try:
                barrier.wait()  # let the other thread start as well
                time.sleep(0.05)
            finally:
                with lock:
                    active -= 1

        def runner():
            slow_work()

        t1 = threading.Thread(target=runner)
        t2 = threading.Thread(target=runner)
        t1.start()
        t2.start()
        t1.join()
        t2.join()

        self.assertEqual(max_active, 1,
            "Bulkhead should have limited concurrency to 1")


class TestTimeout(unittest.TestCase):
    """Test timeout enforcement."""

    def test_timeout_raises_on_slow_call(self):
        @resilience(config=ResilienceConfig(
            timeout=TimeoutPolicy(timeout_seconds=0.1),
            retry=RetryPolicy(max_attempts=1)
        ))
        def slow():
            time.sleep(1.0)  # much longer than timeout

        with self.assertRaises(TimeoutError):
            slow()


class TestCombinedResilience(unittest.TestCase):
    """Integration test: combine all four policies."""

    def test_combined_retry_circuit_bulkhead_timeout(self):
        call_count = 0

        @resilience(config=ResilienceConfig(
            retry=RetryPolicy(max_attempts=3, backoff_base=0.01, jitter=False),
            circuit_breaker=CircuitBreaker(
                failure_threshold=10,  # high so it never opens in this test
                reset_timeout=60,
                expected_exceptions=(RuntimeError,)
            ),
            bulkhead=BulkheadPolicy(max_concurrent_calls=3),
            timeout=TimeoutPolicy(timeout_seconds=2.0)
        ))
        def mixed_behaviour():
            nonlocal call_count
            call_count += 1
            if call_count == 1:
                raise RuntimeError("first failure")
            if call_count == 2:
                raise TimeoutError("simulated timeout")  # timeout should be retried
            return "success"

        result = mixed_behaviour()
        self.assertEqual(result, "success")
        self.assertEqual(call_count, 3)

    def test_disabled_policies_are_noops(self):
        """When all policies are None, the decorator behaves like a plain call."""
        @resilience(config=ResilienceConfig(
            retry=None,
            circuit_breaker=None,
            bulkhead=None,
            timeout=None
        ))
        def plain(x):
            return x * 2

        self.assertEqual(plain(21), 42)


if __name__ == "__main__":
    unittest.main()