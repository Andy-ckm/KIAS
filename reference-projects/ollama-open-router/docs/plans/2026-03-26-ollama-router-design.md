# Ollama Router Design

## Overview

A local proxy service that automatically rotates between multiple ollama.com API keys to handle rate limits and usage quotas.

## Architecture

```
OpenCode → Ollama Router (localhost:11435) → [http_proxy/https_proxy] → ollama.com
                                   ↓
                              State Store
                         (key_states.json + current_index.json)
```

## Components

| Component | Responsibility |
|-----------|----------------|
| Key Selector | Polling selection of available keys, skipping cooling keys |
| Rate Limit Handler | Detects 429 errors, marks keys as cooling, updates next available time |
| Request Proxy | Receives OpenCode requests, forwards to ollama.com, passes through responses |
| State Store | Memory + file persistence for key states and polling position |

## Error Detection

Based on actual API testing:

```json
{
  "error": "you have reached your session usage limit..."
}
```

| Error Keywords | Cooldown |
|----------------|----------|
| `"session usage limit"` | 72 hours (weekly quota) |
| TBD (hourly quota) | 4 hours |

```python
if "session usage limit" in error_message:
    cooldown_hours = 72
elif "rate limit" in error_message or "too many requests" in error_message:
    cooldown_hours = 4
else:
    cooldown_hours = 72  # Conservative fallback
```

## File Structure

```
ollama_router/
├── config.yaml           # API Keys configuration
├── state/
│   ├── key_states.json   # Key cooling states
│   └── current_index.json # Polling pointer
└── router.py             # Main application
```

## State Store Format

**key_states.json:**
```json
{
  "keys": [
    {
      "key": "sk-xxx...xxx",
      "state": "cooldown",
      "cooldown_until": "2026-03-29T12:00:00+08:00",
      "reason": "session_usage_limit"
    },
    {
      "key": "sk-yyy...yyy",
      "state": "available",
      "cooldown_until": null,
      "reason": null
    }
  ]
}
```

**current_index.json:**
```json
{
  "index": 2,
  "last_updated": "2026-03-26T12:00:00+08:00"
}
```

## Configuration (config.yaml)

```yaml
listen: "127.0.0.1:11435"

upstream: "https://ollama.com/v1"

proxy:
  http: "${http_proxy}"
  https: "${https_proxy}"
  no_proxy: "localhost,127.0.0.1"

keys:
  - "${OLLAMA_API_KEY_1}"
  - "${OLLAMA_API_KEY_2}"

cooldown:
  session_limit_hours: 72
  rate_limit_hours: 4
```

## OpenCode Configuration

Update `~/.config/opencode/opencode.json`:

```json
{
  "provider": {
    "ollama": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Ollama Router",
      "options": {
        "baseURL": "http://127.0.0.1:11435/v1"
      },
      "models": {
        "glm-4.7:cloud": {
          "name": "glm-4.7:cloud"
        }
      }
    }
  }
}
```

## Tech Stack

- Python
- FastAPI/Flask for HTTP server
- httpx for upstream requests (with proxy support)
- PyYAML for config parsing
- JSON file for state persistence

## Flow

1. OpenCode sends request to `http://127.0.0.1:11435/v1/chat/completions`
2. Ollama Router selects first available key (polling order)
3. Forwards request to `https://ollama.com/v1/chat/completions` with selected key
4. If 429: mark key as cooling, select next available key, retry
5. Pass through response to OpenCode
