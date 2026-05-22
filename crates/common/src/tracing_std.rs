python
"""
tracing_std.py
~~~~~~~~~~~~~~

A tiny, structured‑logging library that integrates tracing (spans,
propagation, sampling) with a pluggable export pipeline.

Highlights
----------
* **Structured logs** – each emitted record is a ``dict`` that can be
  serialized to JSON (or any other format) by the chosen exporter.
* **Span model** – lightweight span objects that hold trace/span IDs,
  operation name, timestamps, and an optional bag of tags.
* **Propagation** – ``TraceContext`` (W3C Trace‑Context) strings are
  generated from a span and can be parsed back.
* **Sampling** – ``Sampler`` abstraction with four concrete policies:
  ``AlwaysOnSampler``, ``AlwaysOffSampler``, ``RandomSampler``,
  ``RateLimitingSampler``.
* **Export** – ``LogExporter`` base class; ``ConsoleExporter``,
  ``FileExporter`` and ``HttpExporter`` are provided.
* **Context injection** – the ``TracingLogger`` automatically injects the
  current trace/span IDs (and any user tags) into every log record.

The library is deliberately minimal, so it does **not** require any external
tracing SDK.  It can be easily replaced by OpenTelemetry or OpenCensus if
you need a richer feature set.
"""

from __future__ import annotations

import abc
import contextvars
import json
import logging
import os
import sys
import threading
import time
import uuid
from datetime import datetime, timezone
from typing import Any, Callable, Dict, Iterable, List, Optional, Tuple, Union
from unittest.mock import Mock

# ---------------------------------------------------------------------------
# Type‑aliases / helpers
# ---------------------------------------------------------------------------

LogLevel = int  # standard logging level (DEBUG=10, INFO=20, …)

TRACE_LEVEL = 5  # “TRACE” – lower than DEBUG
logging.addLevelName(TRACE_LEVEL, "TRACE")

JsonDict = Dict[str, Any]

# ---------------------------------------------------------------------------
# Trace identifiers
# ---------------------------------------------------------------------------

def generate_id(bits: int = 128) -> str:
    """Return a random hex string of *bits* length."""
    byte_len = bits // 8
    return uuid.uuid4().hex[: byte_len * 2]  # each byte → two hex chars


# ---------------------------------------------------------------------------
# Span model
# ---------------------------------------------------------------------------

class SpanContext:
    """Minimal representation of a tracing span context (W3C Trace‑Context)."""

    __slots__ = ("trace_id", "span_id", "trace_flags", "tracestate")

    def __init__(
        self,
        trace_id: str,
        span_id: str,
        trace_flags: int = 0,
        tracestate: Optional[str] = None,
    ) -> None:
        self.trace_id = trace_id
        self.span_id = span_id
        self.trace_flags = trace_flags
        self.tracestate = tracestate or ""

    @property
    def traceparent(self) -> str:
        """W3C ``traceparent`` header value."""
        version = "00"
        return f"{version}-{self.trace_id}-{self.span_id}-{self.trace_flags:02x}"

    @classmethod
    def from_traceparent(cls, traceparent: str) -> SpanContext:
        """Parse a ``traceparent`` string into a SpanContext."""
        try:
            version, trace_id, span_id, flags = traceparent.split("-")
        except ValueError:
            raise ValueError(f"Invalid traceparent: {traceparent!r}")
        return cls(
            trace_id=trace_id,
            span_id=span_id,
            trace_flags=int(flags, 16),
            tracestate=None,
        )

    def to_dict(self) -> JsonDict:
        return {
            "trace_id": self.trace_id,
            "span_id": self.span_id,
            "trace_flags": self.trace_flags,
            "tracestate": self.tracestate,
        }


class Span:
    """
    A lightweight span that records start/end timestamps, operation name,
    parent reference, and an arbitrary set of tags.
    """

    __slots__ = (
        "name",
        "context",
        "parent_id",
        "start_time",
        "end_time",
        "tags",
        "_ended",
    )

    def __init__(
        self,
        name: str,
        context: SpanContext,
        parent_id: Optional[str] = None,
        start_time: Optional[float] = None,
    ) -> None:
        self.name = name
        self.context = context
        self.parent_id = parent_id
        self.start_time = start_time or time.time()
        self.end_time: Optional[float] = None
        self.tags: JsonDict = {}
        self._ended = False

    def set_tag(self, key: str, value: Any) -> None:
        """Add a tag to the span."""
        self.tags[key] = value

    def end(self, end_time: Optional[float] = None) -> None:
        """Mark the span as finished."""
        if self._ended:
            raise RuntimeError("Span already ended")
        self.end_time = end_time or time.time()
        self._ended = True

    @property
    def duration_ms(self) -> Optional[float]:
        if self.end_time is None:
            return None
        return (self.end_time - self.start_time) * 1_000

    def to_dict(self) -> JsonDict:
        return {
            "name": self.name,
            "context": self.context.to_dict(),
            "parent_id": self.parent_id,
            "start_time": self.start_time,
            "end_time": self.end_time,
            "duration_ms": self.duration_ms,
            "tags": self.tags,
        }


# ---------------------------------------------------------------------------
# Sampling
# ---------------------------------------------------------------------------

class Sampler(abc.ABC):
    """Abstract sampler – decide whether a new span should be recorded."""

    @abc.abstractmethod
    def should_sample(self, operation_name: str) -> bool:
        """Return ``True`` if the span should be recorded."""
        raise NotImplementedError


class AlwaysOnSampler(Sampler):
    def should_sample(self, operation_name: str) -> bool:
        return True


class AlwaysOffSampler(Sampler):
    def should_sample(self, operation_name: str) -> bool:
        return False


class RandomSampler(Sampler):
    """
    Randomly samples a fraction *p* of spans (0 ≤ p ≤ 1).
    """

    def __init__(self, probability: float = 0.1) -> None:
        if not (0.0 <= probability <= 1.0):
            raise ValueError("probability must be between 0 and 1")
        self.probability = probability

    def should_sample(self, operation_name: str) -> bool:
        return uuid.uuid4().random() < self.probability


class RateLimitingSampler(Sampler):
    """
    Allows at most *qps* spans per second (averaged over a sliding window).
    """

    def __init__(self, qps: float = 100.0) -> None:
        if qps <= 0:
            raise ValueError("qps must be positive")
        self.qps = qps
        self._lock = threading.Lock()
        self._timestamps: List[float] = []

    def should_sample(self, operation_name: str) -> bool:
        now = time.time()
        with self._lock:
            # Evict timestamps older than 1 s
            self._timestamps = [ts for ts in self._timestamps if now - ts < 1.0]
            if len(self._timestamps) < self.qps:
                self._timestamps.append(now)
                return True
            return False


# ---------------------------------------------------------------------------
# Export pipeline
# ---------------------------------------------------------------------------

class LogExporter(abc.ABC):
    """Abstract exporter – receives structured log records."""

    @abc.abstractmethod
    def export(self, records: Iterable[JsonDict]) -> None:
        """Persist the *records* (called with a list of log dicts)."""
        raise NotImplementedError


class ConsoleExporter(LogExporter):
    """
    Writes each record as a JSON line to stdout.
    """

    def __init__(
        self,
        out: Optional[Callable[[str], None]] = None,
        pretty: bool = False,
    ) -> None:
        self._out = out or (lambda s: sys.stdout.write(s + "\n"))
        self._pretty = pretty

    def export(self, records: Iterable[JsonDict]) -> None:
        for rec in records:
            self._out(
                json.dumps(rec, indent=2 if self._pretty else None, default=str)
            )


class FileExporter(LogExporter):
    """
    Appends JSON lines to a local file.
    """

    def __init__(self, path: str, **kwargs: Any) -> None:
        self._path = path
        self._kwargs = kwargs
        self._lock = threading.Lock()

    def export(self, records: Iterable[JsonDict]) -> None:
        with self._lock:
            with open(self._path, "a", encoding="utf-8") as f:
                for rec in records:
                    f.write(json.dumps(rec, default=str) + "\n")


class HttpExporter(LogExporter):
    """
    Sends JSON payloads to a remote HTTP collector (POST JSON).
    Uses ``requests`` if available, otherwise falls back to ``urllib``.
    """

    def __init__(self, url: str, timeout: float = 5.0) -> None:
        self.url = url
        self.timeout = timeout
        self._requests = self._import_requests()

    @staticmethod
    def _import_requests():
        try:
            import requests
            return requests
        except ImportError:
            import urllib.request, urllib.error
            return urllib

    def export(self, records: Iterable[JsonDict]) -> None:
        payload = json.dumps(list(records), default=str)
        if self._requests.__name__ == "requests":
            self._requests.post(
                self.url, data=payload, timeout=self.timeout,
                headers={"Content-Type": "application/json"},
            )
        else:
            req = self._requests.request.Request(
                self.url,
                data=payload.encode("utf-8"),
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            self._requests.request.urlopen(req, timeout=self.timeout)


# ---------------------------------------------------------------------------
# Tracing logger – core integration point
# ---------------------------------------------------------------------------

class TracingLogger:
    """
    High‑level logger that binds a ``Tracer`` and a ``LogExporter`` together.

    Features
    --------
    * **Structured logging** – ``debug()`` / ``info()`` / ``warning()`` /
      ``error()`` accept arbitrary keyword arguments that become part of the
      JSON record.
    * **Automatic context injection** – every emitted record contains the
      current ``trace_id`` and ``span_id``.
    * **Sampling** – only records whose span is sampled are emitted.
    * **Export** – records are batched and sent to the configured exporter.
    """

    def __init__(
        self,
        service_name: str,
        sampler: Sampler,
        exporter: LogExporter,
        min_level: LogLevel = logging.INFO,
        batch_size: int = 100,
        flush_interval: float = 5.0,
    ) -> None:
        self.service_name = service_name
        self.sampler = sampler
        self.exporter = exporter
        self.min_level = min_level
        self.batch_size = batch_size
        self.flush_interval = flush_interval

        # Thread‑safe storage for the current span.
        self._span_stack: contextvars.ContextVar[List[Span]] = contextvars.ContextVar(
            "span_stack", default=[]
        )
        self._batch: List[JsonDict] = []
        self._batch_lock = threading.Lock()
        self._last_flush = time.time()

    # ------------------------------------------------------------------
    # Tracing helpers
    # ------------------------------------------------------------------

    def start_span(
        self,
        name: str,
        parent: Optional[Span] = None,
        tags: Optional[JsonDict] = None,
    ) -> Optional[Span]:
        """
        Create a new span if the sampler decides to record it.
        Returns ``None`` when the span is not sampled.
        """
        if not self.sampler.should_sample(name):
            return None

        trace_id = generate_id()
        span_id = generate_id(bits=64)
        parent_id = parent.context.span_id if parent else None
        ctx = SpanContext(trace_id=trace_id, span_id=span_id)

        span = Span(name=name, context=ctx, parent_id=parent_id)
        if tags:
            for k, v in tags.items():
                span.set_tag(k, v)

        stack = self._span_stack.get()
        stack.append(span)
        self._span_stack.set(stack)
        return span

    def end_span(self, span: Optional[Span]) -> None:
        """Close the supplied span and remove it from the current stack."""
        if span is None:
            return
        span.end()
        stack = self._span_stack.get()
        if stack and stack[-1] is span:
            stack.pop()
            self._span_stack.set(stack)

    def span(self, name: str, tags: Optional[JsonDict] = None):
        """
        Context‑manager that automatically creates and ends a span.

        Usage::

            with logger.span("my_operation"):
                do_work()
        """
        return _SpanContextManager(self, name, tags)

    # ------------------------------------------------------------------
    # Log emission
    # ------------------------------------------------------------------

    def _current_span(self) -> Optional[Span]:
        stack = self._span_stack.get()
        return stack[-1] if stack else None

    def _emit(self, level: LogLevel, msg: str, **kwargs: Any) -> None:
        if level < self.min_level:
            return

        span = self._current_span()
        if span is None:
            # No sampled span → still emit a “no‑trace” record
            record = self._build_record(level, msg, None, **kwargs)
        else:
            record = self._build_record(level, msg, span, **kwargs)

        with self._batch_lock:
            self._batch.append(record)
            now = time.time()
            if (
                len(self._batch) >= self.batch_size
                or now - self._last_flush >= self.flush_interval
            ):
                self._flush()

    def _build_record(
        self,
        level: LogLevel,
        msg: str,
        span: Optional[Span],
        **kwargs: Any,
    ) -> JsonDict:
        now_iso = datetime.now(timezone.utc).isoformat()
        record: JsonDict = {
            "timestamp": now_iso,
            "level": logging.getLevelName(level),
            "logger": self.service_name,
            "message": msg,
        }

        if span is not None:
            record["trace"] = {
                "trace_id": span.context.trace_id,
                "span_id": span.context.span_id,
            }
            record["span_tags"] = span.tags

        if kwargs:
            record["extra"] = kwargs

        return record

    def _flush(self) -> None:
        if not self._batch:
            return
        try:
            self.exporter.export(self._batch[:])
        except Exception as exc:
            # In a real system you might want a fallback or a dead‑letter queue.
            sys.stderr.write(f"[tracing_std] Export error: {exc}\n")
        finally:
            self._batch.clear()
            self._last_flush = time.time()

    # ------------------------------------------------------------------
    # Public logging methods
    # ------------------------------------------------------------------

    def trace(self, msg: str, **kwargs: Any) -> None:
        self._emit(TRACE_LEVEL, msg, **kwargs)

    def debug(self, msg: str, **kwargs: Any) -> None:
        self._emit(logging.DEBUG, msg, **kwargs)

    def info(self, msg: str, **kwargs: Any) -> None:
        self._emit(logging.INFO, msg, **kwargs)

    def warning(self, msg: str, **kwargs: Any) -> None:
        self._emit(logging.WARNING, msg, **kwargs)

    def error(self, msg: str, **kwargs: Any) -> None:
        self._emit(logging.ERROR, msg, **kwargs)

    def critical(self, msg: str, **kwargs: Any) -> None:
        self._emit(logging.CRITICAL, msg, **kwargs)

    def close(self) -> None:
        """Flush any remaining records and release resources."""
        with self._batch_lock:
            self._flush()


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------

class _SpanContextManager:
    """Simple context manager used by ``TracingLogger.span``."""

    __slots__ = ("_logger", "_name", "_tags", "_span")

    def __init__(
        self, logger: TracingLogger, name: str, tags: Optional[JsonDict]
    ) -> None:
        self._logger = logger
        self._name = name
        self._tags = tags
        self._span: Optional[Span] = None

    def __enter__(self) -> Span:
        self._span = self._logger.start_span(self._name, tags=self._tags)
        return self._span

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        if self._span is not None:
            if exc_type is not None:
                self._span.set_tag("error", True)
                self._span.set_tag("error.type", exc_type.__name__)
                self._span.set_tag("error.msg", str(exc_val))
            self._logger.end_span(self._span)
        return None  # do not suppress exceptions