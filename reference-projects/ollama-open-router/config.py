import hashlib
import os
import re
import warnings
from dataclasses import dataclass, field
from typing import Literal

import yaml


def expand_env(value: str) -> str | None:
    if isinstance(value, str):
        pattern = re.compile(r"\$\{([^}]+)\}")
        match = pattern.match(value)
        if match:
            env_expr = match.group(1)
            # Support ${VAR:-default} syntax
            if ":-" in env_expr:
                env_var, default = env_expr.split(":-", 1)
                return os.environ.get(env_var, default)
            else:
                return os.environ.get(env_expr)
        return value
    return value


def is_likely_api_key(value: str) -> bool:
    if not value:
        return False
    if value.startswith("${") and value.endswith("}"):
        return False
    if re.match(r"^[A-Z_]+$", value):
        return False
    if "." in value or len(value) > 20:
        return True
    return False


@dataclass
class LoggingConfig:
    level: Literal["DEBUG", "INFO", "WARNING", "ERROR"] = "INFO"
    file: str | None = None
    max_size_mb: int = 10
    backup_count: int = 5

    @classmethod
    def from_dict(cls, data: dict) -> "LoggingConfig":
        return cls(
            level=data.get("level", "INFO").upper(),
            file=data.get("file"),
            max_size_mb=data.get("max_size_mb", 10),
            backup_count=data.get("backup_count", 5),
        )


@dataclass
class Config:
    listen: str
    upstream: str
    keys: list[str]
    cooldown_session_hours: int = 5
    cooldown_weekly_hours: int = 168
    cooldown_rate_hours: int = 4
    proxy_http: str | None = None
    proxy_https: str | None = None
    proxy_no_proxy: str | None = None
    admin_username: str = "admin"
    admin_password: str = "admin"
    admin_session_secret: str = "change-me"
    logging: LoggingConfig = field(default_factory=LoggingConfig)


def validate_keys(keys: list[str]) -> list[str]:
    valid_keys = []
    has_hardcoded = False

    for key in keys:
        if not key:
            continue

        if is_likely_api_key(key):
            has_hardcoded = True
            masked = f"...{key[-4:]}" if len(key) > 4 else "***"
            warnings.warn(
                f"Security warning: Hardcoded API key detected ({masked}). "
                "Consider using environment variables instead: ${{YOUR_API_KEY}}",
                UserWarning,
                stacklevel=3,
            )

        valid_keys.append(key)

    if has_hardcoded:
        warnings.warn(
            " SECURITY WARNING: One or more API keys appear to be hardcoded in config.yaml. "
            "This is a security risk. Consider using environment variables.\n"
            "Example:\n"
            "  keys:\n"
            "    - ${OLLAMA_API_KEY_1}\n"
            "    - ${OLLAMA_API_KEY_2}",
            UserWarning,
            stacklevel=3,
        )

    if not valid_keys:
        raise ValueError(
            "No valid API keys configured. "
            "Add keys to config.yaml or set environment variables.\n"
            "Example:\n"
            "  keys:\n"
            "    - ${OLLAMA_API_KEY_1}\n"
            "    - ${OLLAMA_API_KEY_2}\n\n"
            "Then set the environment variable:\n"
            "  export OLLAMA_API_KEY_1=your-api-key"
        )

    return valid_keys


def get_key_id(key: str) -> str:
    if not key:
        return "empty"
    return hashlib.sha256(key.encode()).hexdigest()[:8]


def load_config(path: str) -> Config:
    with open(path, encoding="utf-8") as f:
        data = yaml.safe_load(f)

    raw_keys = data.get("keys", [])
    keys = [expand_env(k) if isinstance(k, str) else k for k in raw_keys]
    keys = [k for k in keys if k]

    keys = validate_keys(keys)

    proxy_data = data.get("proxy", {})
    admin_data = data.get("admin", {})

    # Parse logging config with env var overrides
    logging_data = data.get("logging", {})
    log_level = os.environ.get("OLLAMA_LOG_LEVEL") or logging_data.get("level", "INFO")
    log_file = os.environ.get("OLLAMA_LOG_FILE") or logging_data.get("file")
    logging_config = LoggingConfig(
        level=log_level,
        file=log_file,
        max_size_mb=logging_data.get("max_size_mb", 10),
        backup_count=logging_data.get("backup_count", 5),
    )

    return Config(
        listen=data.get("listen", "127.0.0.1:11435"),
        upstream=data.get("upstream", "https://ollama.com/v1"),
        keys=keys,
        cooldown_session_hours=data.get("cooldown", {}).get("session_limit_hours", 5),
        cooldown_weekly_hours=data.get("cooldown", {}).get("weekly_limit_hours", 168),
        cooldown_rate_hours=data.get("cooldown", {}).get("rate_limit_hours", 4),
        proxy_http=expand_env(proxy_data.get("http")),
        proxy_https=expand_env(proxy_data.get("https")),
        proxy_no_proxy=proxy_data.get("no_proxy"),
        admin_username=expand_env(admin_data.get("username", "admin")) or "admin",
        admin_password=expand_env(admin_data.get("password", "admin")) or "admin",
        admin_session_secret=expand_env(admin_data.get("session_secret", "change-me"))
        or "change-me",
        logging=logging_config,
    )
