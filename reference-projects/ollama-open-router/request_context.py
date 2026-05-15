"""Request ID tracking via contextvars for async-safe propagation."""

import contextvars
import logging
import secrets
from typing import Final

# Context variable for request ID
request_id_var: Final[contextvars.ContextVar[str | None]] = contextvars.ContextVar(
    "request_id", default=None
)


def generate_request_id() -> str:
    """Generate a unique request ID."""
    return f"req_{secrets.token_hex(4)}"


def get_request_id() -> str:
    """Get current request ID, or 'no-request' if not set."""
    return request_id_var.get() or "no-request"


def set_request_id(req_id: str) -> contextvars.Token:
    """Set request ID and return token for reset."""
    return request_id_var.set(req_id)


class RequestIdFilter(logging.Filter):
    """Logging filter that injects request_id into LogRecord."""

    def filter(self, record: logging.LogRecord) -> bool:
        record.request_id = get_request_id()
        return True
