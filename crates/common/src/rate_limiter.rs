python
#!/usr/bin/env python3
"""
rate_limiter.py
----------------

A comprehensive, thread‑safe rate‑limiting toolkit that supports:

* Sliding‑window  (fixed time‑window, classic per‑key deque implementation)
* Token‑bucket   (configurable capacity & refill rate)
* Leaky‑bucket   (constant outflow rate, request排队)
* Per‑key limits (a manager that stores one limiter per arbitrary key)
* Adaptive       (token‑bucket whose refill rate is automatically
                  increased/decreased according to the ratio of
                  successful vs. dropped requests)

All limiters expose the same high‑level interface::

    limiter = SomeLimiter()
    if limiter.allow(key):          # returns True if the request is allowed
        do_work()
    else:
        raise RateLimitExceeded(key)

The module can be imported as a library or run directly to execute the test suite.

"""

from __future__ import annotations

import threading
import time
import math
import functools
import collections
from typing import (
    Dict,
    Tuple,
    Optional,
    Callable,
    Any,
    Type,
    TypeVar,
    Generic,
    Union,
)

__all__ = [
    "RateLimitExceeded",
    "SlidingWindowRateLimiter",
    "TokenBucketRateLimiter",
    "LeakyBucketRateLimiter",
    "AdaptiveRateLimiter",
    "RateLimiterManager",
    "limit_rate",
]

# ----------------------------------------------------------------------
# Exceptions
# ----------------------------------------------------------------------


class RateLimitExceeded(Exception):
    """Raised when a request is denied because the rate limit is exceeded."""

    def __init__(self, key: str = "", limit: float = 0.0, until: float = 0.0):
        self.key = key
        self.limit = limit
        self.until = until
        super().__init__(
            f"Rate limit exceeded for key '{key}'. "
            f"Allowed up to {limit:.2f} req/s. "
            f"Retry after {until:.2f}s."
        )


# ----------------------------------------------------------------------
# Helper utilities
# ----------------------------------------------------------------------


def _now() -> float:
    """Monotonic time (float seconds)."""
    return time.monotonic()


def _clamp(value: float, lower: float = 0.0, upper: float = 1.0) -> float:
    return max(lower, min(upper, value))


# ----------------------------------------------------------------------
# Base abstract limiter
# ----------------------------------------------------------------------


class BaseRateLimiter(Generic[str]):
    """Abstract base for all rate limiters.

    Sub‑classes must implement the :py:meth:`_try_acquire` method.
    The public :py:meth:`allow` method is thread‑safe and raises
    :py:class:`RateLimitExceeded` on denial.
    """

    lock: threading.Lock
    _default_key: str

    def __init__(self, *, default_key: str = "default") -> None:
        self.lock = threading.Lock()
        self._default_key = default_key

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def allow(self, key: Optional[str] = None) -> bool:
        """Return ``True`` if the request for ``key`` is permitted."""
        k = key if key is not None else self._default_key
        with self.lock:
            return self._try_acquire(k)

    def request(self, key: Optional[str] = None) -> None:
        """Perform a request, raising :py:class:`RateLimitExceeded` if denied."""
        if not self.allow(key):
            raise RateLimitExceeded(
                key=key or self._default_key,
                limit=self._limit_value(key),
                until=self._retry_after(key),
            )

    def _limit_value(self, key: str) -> float:
        """Hook for subclasses to expose their limit in req/s (used in exception)."""
        return 0.0

    def _retry_after(self, key: str) -> float:
        """Hook for subclasses to expose a wait time until the next request is allowed."""
        return 0.0

    # ------------------------------------------------------------------
    # Abstract method
    # ------------------------------------------------------------------

    def _try_acquire(self, key: str) -> bool:
        """Implements the concrete algorithm. Must be overridden."""
        raise NotImplementedError


# ----------------------------------------------------------------------
# Sliding‑Window Rate Limiter
# ----------------------------------------------------------------------


class SlidingWindowRateLimiter(BaseRateLimiter):
    """Fixed‑size sliding window.

    The window holds a *deque* of timestamps for each key. The request
    is allowed iff the number of timestamps inside the ``window_seconds``
    window is lower than ``limit``.
    """

    def __init__(
        self,
        limit: float = 10,
        window_seconds: float = 1.0,
        *,
        default_key: str = "default",
    ) -> None:
        super().__init__(default_key=default_key)
        self.limit = limit
        self.window_seconds = window_seconds
        self._history: Dict[str, collections.deque[float]] = collections.defaultdict(
            lambda: collections.deque()
        )

    def _try_acquire(self, key: str) -> bool:
        now = _now()
        cutoff = now - self.window_seconds
        # Remove timestamps outside the window
        q = self._history[key]
        while q and q[0] <= cutoff:
            q.popleft()
        if len(q) < self.limit:
            q.append(now)
            return True
        return False

    def _limit_value(self, key: str) -> float:
        return self.limit / self.window_seconds

    def _retry_after(self, key: str) -> float:
        """Time until the oldest request in the window expires."""
        q = self._history[key]
        if not q:
            return 0.0
        return q[0] + self.window_seconds - _now()


# ----------------------------------------------------------------------
# Token‑Bucket Rate Limiter
# ----------------------------------------------------------------------


class TokenBucketRateLimiter(BaseRateLimiter):
    """Token‑bucket with configurable capacity and refill rate.

    Tokens are added at ``refill_rate`` tokens per second, up to ``capacity``.
    A request consumes one token. If the bucket is empty the request is denied.
    """

    def __init__(
        self,
        capacity: float = 10,
        refill_rate: float = 5,
        *,
        default_key: str = "default",
    ) -> None:
        super().__init__(default_key=default_key)
        self.capacity = capacity
        self.refill_rate = refill_rate
        # Per‑key state: (tokens_left, timestamp_of_last_refill)
        self._buckets: Dict[str, Tuple[float, float]] = {}

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _get_bucket(self, key: str) -> Tuple[float, float]:
        if key not in self._buckets:
            # initialise with full bucket, at the current time
            self._buckets[key] = (self.capacity, _now())
        return self._buckets[key]

    def _refill(self, key: str) -> Tuple[float, float]:
        tokens, last = self._get_bucket(key)
        now = _now()
        elapsed = now - last
        # Add tokens, but never exceed capacity
        tokens = min(self.capacity, tokens + elapsed * self.refill_rate)
        return tokens, now

    # ------------------------------------------------------------------
    # Core algorithm
    # ------------------------------------------------------------------

    def _try_acquire(self, key: str) -> bool:
        tokens, last = self._get_bucket(key)
        tokens, now = self._refill(key)
        if tokens >= 1.0:
            tokens -= 1.0
            self._buckets[key] = (tokens, now)
            return True
        return False

    def _limit_value(self, key: str) -> float:
        return self.refill_rate

    def _retry_after(self, key: str) -> float:
        tokens, _ = self._get_bucket(key)
        if tokens >= 1.0:
            return 0.0
        needed = 1.0 - tokens
        return needed / self.refill_rate


# ----------------------------------------------------------------------
# Leaky‑Bucket Rate Limiter
# ----------------------------------------------------------------------


class LeakyBucketRateLimiter(BaseRateLimiter):
    """Leaky‑bucket (also known as “request queue”).

    Each request inserts a token into the bucket; the bucket leaks at
    ``leak_rate`` tokens per second. The bucket is capped at ``capacity``.
    If the bucket would overflow the request is denied.
    """

    def __init__(
        self,
        capacity: float = 10,
        leak_rate: float = 5,
        *,
        default_key: str = "default",
    ) -> None:
        super().__init__(default_key=default_key)
        self.capacity = capacity
        self.leak_rate = leak_rate
        # Per‑key state: (tokens_in_bucket, timestamp_of_last_leak)
        self._buckets: Dict[str, Tuple[float, float]] = {}

    def _get_bucket(self, key: str) -> Tuple[float, float]:
        if key not in self._buckets:
            self._buckets[key] = (0.0, _now())
        return self._buckets[key]

    def _leak(self, key: str) -> Tuple[float, float]:
        tokens, last = self._get_bucket(key)
        now = _now()
        elapsed = now - last
        # Leak tokens, but never go below zero
        tokens = max(0.0, tokens - elapsed * self.leak_rate)
        return tokens, now

    def _try_acquire(self, key: str) -> bool:
        tokens, last = self._get_bucket(key)
        tokens, now = self._leak(key)
        if tokens + 1.0 <= self.capacity:
            tokens += 1.0
            self._buckets[key] = (tokens, now)
            return True
        return False

    def _limit_value(self, key: str) -> float:
        return self.leak_rate

    def _retry_after(self, key: str) -> float:
        """Time until enough capacity is freed to accept one more request."""
        tokens, _ = self._get_bucket(key)
        if tokens < self.capacity:
            return 0.0
        # bucket is full, compute time to leak one token
        needed = tokens - (self.capacity - 1.0)
        return needed / self.leak_rate


# ----------------------------------------------------------------------
# Adaptive Rate Limiter (token‑bucket with dynamic refill rate)
# ----------------------------------------------------------------------


class AdaptiveRateLimiter(BaseRateLimiter):
    """Adaptive token‑bucket.

    It monitors the ratio of successful vs. denied requests and
    smoothly adjusts the refill rate:

    * ``refill_rate`` increases when the success ratio stays high.
    * ``refill_rate`` decreases when denials become frequent.
    * The rate is bounded between ``min_rate`` and ``max_rate``.
    """

    def __init__(
        self,
        capacity: float = 20,
        initial_rate: float = 10,
        min_rate: float = 1.0,
        max_rate: float = 100,
        adaptation_factor: float = 0.05,
        window_seconds: float = 60.0,
        *,
        default_key: str = "default",
    ) -> None:
        super().__init__(default_key=default_key)
        self.capacity = capacity
        self.initial_rate = initial_rate
        self.min_rate = min_rate
        self.max_rate = max_rate
        self.adaptation_factor = adaptation_factor
        self.window_seconds = window_seconds

        self._buckets: Dict[
            str, Tuple[float, float]
        ] = {}  # (tokens_left, timestamp_of_last_refill)
        # Success / failure sliding counters
        self._success: Dict[str, collections.deque[float]] = collections.defaultdict(
            lambda: collections.deque()
        )
        self._failure: Dict[str, collections.deque[float]] = collections.defaultdict(
            lambda: collections.deque()
        )
        self._refill_rates: Dict[str, float] = collections.defaultdict(
            lambda: initial_rate
        )

    def _now(self) -> float:
        return _now()

    def _record(self, key: str, success: bool) -> None:
        """Append a success/failure timestamp for the given key."""
        now = self._now()
        container = self._success if success else self._failure
        container[key].append(now)

    def _clean(self, key: str) -> None:
        """Prune timestamps older than window_seconds."""
        cutoff = self._now() - self.window_seconds
        for container in (self._success[key], self._failure[key]):
            while container and container[0] <= cutoff:
                container.popleft()

    def _success_ratio(self, key: str) -> float:
        """Return proportion of successes in the sliding window."""
        s = len(self._success[key])
        f = len(self._failure[key])
        total = s + f
        if total == 0:
            return 1.0  # optimistic when no data
        return s / total

    def _adapt(self, key: str) -> float:
        """Adjust refill rate based on success ratio."""
        ratio = self._success_ratio(key)
        # If successes are high → increase, else decrease
        current_rate = self._refill_rates[key]
        delta = self.adaptation_factor * current_rate * (ratio - 0.5)
        new_rate = _clamp(current_rate + delta, self.min_rate, self.max_rate)
        self._refill_rates[key] = new_rate
        return new_rate

    def _refill(self, key: str) -> Tuple[float, float]:
        tokens, last = self._buckets.get(key, (self.capacity, self._now()))
        now = self._now()
        refill_rate = self._refill_rates[key]
        tokens = min(self.capacity, tokens + (now - last) * refill_rate)
        return tokens, now

    def _try_acquire(self, key: str) -> bool:
        self._clean(key)
        tokens, now = self._refill(key)
        if tokens >= 1.0:
            tokens -= 1.0
            self._buckets[key] = (tokens, now)
            self._record(key, success=True)
            # Trigger adaptation after each successful request
            self._adapt(key)
            return True
        else:
            self._record(key, success=False)
            self._adapt(key)
            return False

    def _limit_value(self, key: str) -> float:
        return self._refill_rates[key]

    def _retry_after(self, key: str) -> float:
        tokens, _ = self._buckets.get(key, (self.capacity, self._now()))
        if tokens >= 1.0:
            return 0.0
        return (1.0 - tokens) / self._refill_rates[key]


# ----------------------------------------------------------------------
# Per‑Key Manager
# ----------------------------------------------------------------------


class RateLimiterManager(BaseRateLimiter):
    """Holds a separate rate limiter for each key.

    The *factory* argument is a callable that returns a fresh
    ``BaseRateLimiter`` instance (or any subclass).  The manager stores a
    limiter per key in a dictionary and forwards all calls to the correct
    limiter.  This makes it easy to enforce per‑client/ per‑IP/ per‑user
    limits without caring about the underlying algorithm.
    """

    def __init__(
        self,
        factory: Callable[[], BaseRateLimiter],
        *,
        default_key: str = "default",
    ) -> None:
        super().__init__(default_key=default_key)
        self._factory = factory
        self._limiters: Dict[str, BaseRateLimiter] = {}
        self._factory_lock = threading.Lock()

    def _limiter(self, key: str) -> BaseRateLimiter:
        """Lazily create a limiter for a key (thread‑safe)."""
        if key not in self._limiters:
            with self._factory_lock:
                # Double‑check
                if key not in self._limiters:
                    self._limiters[key] = self._factory()
        return self._limiters[key]

    def _try_acquire(self, key: str) -> bool:
        return self._limiter(key).allow(key)  # type: ignore[operator]

    def _limit_value(self, key: str) -> float:
        return self._limiter(key)._limit_value(key)  # type: ignore[arg-type, return-value]

    def _retry_after(self, key: str) -> float:
        return self._limiter(key)._retry_after(key)  # type: ignore[return-value]

    def clear(self, key: Optional[str] = None) -> None:
        """Remove a limiter for a given key, or all if ``key`` is ``None``."""
        if key is None:
            with self._factory_lock:
                self._limiters.clear()
        else:
            with self._factory_lock:
                self._limiters.pop(key, None)

    def update(
        self,
        key: str,
        limiter: BaseRateLimiter,
    ) -> None:
        """Manually replace the limiter for a given key."""
        with self._factory_lock:
            self._limiters[key] = limiter


# ----------------------------------------------------------------------
# Decorator helper
# ----------------------------------------------------------------------


_T = TypeVar("_T", bound=Callable[..., Any])


def limit_rate(
    limiter: BaseRateLimiter,
    key: Optional[str] = None,
    *,
    raise_on_limit: bool = False,
) -> Callable[[_T], _T]:
    """Decorator that applies a rate limiter to a callable.

    Args:
        limiter: Any :py:class:`BaseRateLimiter` instance.
        key: Optional key passed to ``limiter.allow()``.
        raise_on_limit: If ``True`` the decorator will raise
            :py:class:`RateLimitExceeded`; otherwise it silently returns
            ``False`` (the wrapped function is not called).

    Example:
        >>> limiter = SlidingWindowRateLimiter(limit=5, window_seconds=1.0)
        >>> @limit_rate(limiter, key="user_1")
        ... def heavy_operation():
        ...     pass
    """

    def decorator(func: _T) -> _T:
        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            if limiter.allow(key):
                return func(*args, **kwargs)
            if raise_on_limit:
                limiter.request(key)
            return None

        return wrapper  # type: ignore[return-value]

    return decorator


# ----------------------------------------------------------------------
# Simple end‑to‑end demo (optional)
# ----------------------------------------------------------------------


def _demo() -> None:
    """Illustrate each limiter in action (not part of the test suite)."""
    print("=== Sliding Window ===")
    sw = SlidingWindowRateLimiter(limit=3, window_seconds=2.0)
    for i in range(6):
        print(f"  Request {i}: {sw.allow()}")

    print("\n=== Token Bucket ===")
    tb = TokenBucketRateLimiter(capacity=5, refill_rate=2)
    for i in range(6):
        print(f"  Request {i}: {tb.allow()}")
        time.sleep(0.25)

    print("\n=== Leaky Bucket ===")
    lb = LeakyBucketRateLimiter(capacity=3, leak_rate=2)
    for i in range(5):
        print(f"  Request {i}: {lb.allow()}")
        time.sleep(0.3)

    print("\n=== Per‑Key Manager ===")
    factory = lambda: TokenBucketRateLimiter(capacity=2, refill_rate=1)
    manager = RateLimiterManager(factory)
    for uid in ("alice", "bob"):
        for j in range(3):
            print(f"  {uid} request {j}: {manager.allow(uid)}")
        # Sleep a bit so alice's bucket refills
        time.sleep(0.5)

    print("\n=== Adaptive Limiter (synthetic) ===")
    adaptive = AdaptiveRateLimiter(
        capacity=20, initial_rate=5, min_rate=1, max_rate=50
    )
    # Simulate a burst then a drought
    successes = sum(adaptive.allow() for _ in range(15))
    print(f"  First 15 requests succeeded: {successes}")
    # Now fail deliberately for a while (simulate by calling _record with False)
    # (In practice you would call allow and ignore the result)
    for _ in range(10):
        adaptive._record("default", False)
        adaptive._adapt("default")
    print(f"  Refill rate after failures: {adaptive._refill_rates['default']:.2f}")


# ----------------------------------------------------------------------
# Test suite
# ----------------------------------------------------------------------
import unittest
import threading as thr
import random


class TestSlidingWindowRateLimiter(unittest.TestCase):
    """Test the sliding‑window limiter."""

    def test_allows_up_to_limit(self):
        lim = SlidingWindowRateLimiter(limit=5, window_seconds=1.0)
        for _ in range(5):
            self.assertTrue(lim.allow())

    def test_blocks_when_exceeded(self):
        lim = SlidingWindowRateLimiter(limit=3, window_seconds=2.0)
        for _ in range(3):
            self.assertTrue(lim.allow())
        self.assertFalse(lim.allow())

    def test_window_resets_after_time(self):
        lim = SlidingWindowRateLimiter(limit=2, window_seconds=0.2)
        self.assertTrue(lim.allow())
        self.assertTrue(lim.allow())
        self.assertFalse(lim.allow())
        time.sleep(0.25)
        self.assertTrue(lim.allow())

    def test_concurrent_access(self):
        lim = SlidingWindowRateLimiter(limit=100, window_seconds=1.0)
        results = []
        barrier = thr.Barrier(20)

        def worker():
            barrier.wait()
            results.append(lim.allow())

        threads = [thr.Thread(target=worker) for _ in range(20)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        # All 20 should be allowed because capacity > 20
        self.assertEqual(sum(results), 20)


class TestTokenBucketRateLimiter(unittest.TestCase):
    """Test the token‑bucket limiter."""

    def test_burst_allowed(self):
        lim = TokenBucketRateLimiter(capacity=5, refill_rate=0)
        for _ in range(5):
            self.assertTrue(lim.allow())
        self.assertFalse(lim.allow())

    def test_refill_at_rate(self):
        lim = TokenBucketRateLimiter(capacity=5, refill_rate=10)
        # Consume all tokens
        for _ in range(5):
            lim.allow()
        self.assertFalse(lim.allow())
        # Wait for 0.3 seconds -> 3 tokens
        time.sleep(0.3)
        self.assertTrue(lim.allow())
        self.assertTrue(lim.allow())
        self.assertTrue(lim.allow())
        self.assertFalse(lim.allow())

    def test_tokens_capped_at_capacity(self):
        lim = TokenBucketRateLimiter(capacity=10, refill_rate=10)
        time.sleep(2)  # 20 tokens would be added, but cap = 10
        self.assertTrue(lim.allow())
        self.assertTrue(lim.allow())
        for _ in range(8):
            lim.allow()
        self.assertFalse(lim.allow())

    def test_retry_after_calculation(self):
        lim = TokenBucketRateLimiter(capacity=2, refill_rate=4)
        lim.allow()
        lim.allow()
        # exhausted
        retry = lim._retry_after("default")
        self.assertAlmostEqual(retry, 0.25, places=2