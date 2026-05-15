from dataclasses import dataclass
from enum import Enum

import httpx


class KeyAction(str, Enum):
    """Action to take when an error is detected."""

    COOLDOWN = "cooldown"
    DISABLE = "disable"


@dataclass
class CooldownInfo:
    hours: int
    reason: str
    action: KeyAction = KeyAction.COOLDOWN


class RateLimitHandler:
    """Detects rate limit and usage limit responses from ollama.com.

    Ollama.com API error codes and corresponding actions:

    - 401 Unauthorized: Key is invalid/revoked → DISABLE key permanently
      Response: {"error": "unauthorized", "signin_url": "https://ollama.com/connect?..."}
    - 402 Payment Required: Usage limit exhausted → COOLDOWN
      Response: {"error": "You've reached your usage limit..."}
    - 403 Forbidden: Cloud features disabled or model unavailable → COOLDOWN (long)
      Response: {"error": "remote model is unavailable"}
    - 429 Too Many Requests: Rate/usage limit → COOLDOWN
      Response varies by limit type (session, weekly, rate)
    - 502 Bad Gateway: Cloud model unreachable → COOLDOWN (short, retry with another key)
    """

    def __init__(
        self,
        cooldown_session_hours: int = 5,
        cooldown_weekly_hours: int = 168,
        cooldown_rate_hours: int = 4,
    ):
        self.cooldown_session_hours = cooldown_session_hours
        self.cooldown_weekly_hours = cooldown_weekly_hours
        self.cooldown_rate_hours = cooldown_rate_hours

    def detect_cooldown(self, response: httpx.Response) -> CooldownInfo | None:
        status = response.status_code

        # 401: Key is invalid/revoked — permanently disable this key
        if status == 401:
            return self._detect_unauthorized(response)

        # 402: Usage limit exhausted (plan quota)
        if status == 402:
            return self._detect_usage_limit(response)

        # 403: Forbidden (cloud features disabled or model unavailable)
        if status == 403:
            return self._detect_forbidden(response)

        # 429: Rate limit or weekly usage limit
        if status == 429:
            return self._detect_rate_limit(response)

        # 502: Bad Gateway (cloud model unreachable — temporary, retry)
        if status == 502:
            return CooldownInfo(
                hours=0, reason="bad_gateway", action=KeyAction.COOLDOWN
            )

        return None

    def _detect_unauthorized(self, response: httpx.Response) -> CooldownInfo:
        """Handle 401 Unauthorized - key is invalid/revoked, must be disabled."""
        return CooldownInfo(hours=0, reason="unauthorized", action=KeyAction.DISABLE)

    def _detect_forbidden(self, response: httpx.Response) -> CooldownInfo:
        """Handle 403 Forbidden - cloud features disabled or model unavailable."""
        try:
            error_data = response.json()
            error_message = error_data.get("error", "")
        except Exception:
            error_message = ""

        error_lower = error_message.lower()

        if "unavailable" in error_lower:
            reason = "model_unavailable"
        else:
            reason = "forbidden"

        return CooldownInfo(hours=self.cooldown_session_hours, reason=reason)

    def _detect_usage_limit(self, response: httpx.Response) -> CooldownInfo:
        """Handle 402 Payment Required - usage limit exhausted."""
        try:
            error_data = response.json()
            error_message = error_data.get("error", "")
        except Exception:
            error_message = ""

        error_lower = error_message.lower()

        # Weekly usage limit
        if "weekly" in error_lower:
            return CooldownInfo(
                hours=self.cooldown_weekly_hours,
                reason="weekly_usage_limit",
            )

        # General usage limit (session-level)
        return CooldownInfo(
            hours=self.cooldown_session_hours,
            reason="usage_limit",
        )

    def _detect_rate_limit(self, response: httpx.Response) -> CooldownInfo:
        """Handle 429 Too Many Requests."""
        try:
            error_data = response.json()
            error_message = error_data.get("error", "")
        except Exception:
            error_message = ""

        error_lower = error_message.lower()

        # Weekly usage limit (also returns 429 in some cases)
        if "weekly usage limit" in error_lower or "weekly" in error_lower:
            return CooldownInfo(
                hours=self.cooldown_weekly_hours,
                reason="weekly_usage_limit",
            )

        # Session usage limit
        if "session usage limit" in error_lower or "usage limit" in error_lower:
            return CooldownInfo(
                hours=self.cooldown_session_hours,
                reason="session_usage_limit",
            )

        # Standard rate limit
        if "rate limit" in error_lower or "too many requests" in error_lower:
            return CooldownInfo(
                hours=self.cooldown_rate_hours,
                reason="rate_limit",
            )

        # Unknown 429 - treat as session limit (conservative)
        return CooldownInfo(hours=self.cooldown_session_hours, reason="unknown")
