# pyright: reportMissingImports=false

import json
import logging
from datetime import datetime, timezone
from logging.handlers import RotatingFileHandler
from pathlib import Path

import httpx
from fastapi import FastAPI, Request, Response
from fastapi.responses import JSONResponse
from fastapi.templating import Jinja2Templates
from starlette.middleware.base import BaseHTTPMiddleware

from ollama_router.admin.routes import create_admin_api_router
from ollama_router.admin.views import create_admin_views_router
from ollama_router.anthropic.routes import create_anthropic_router
from ollama_router.config import Config, get_key_id
from ollama_router.handler import RateLimitHandler
from ollama_router.metrics import metrics
from ollama_router.proxy import ProxyClient
from ollama_router.request_context import (
    RequestIdFilter,
    generate_request_id,
    get_request_id,
    request_id_var,
    set_request_id,
)
from ollama_router.request_history import RequestHistory
from ollama_router.retry import RetryManager
from ollama_router.state import KeySelector, KeyState, StateStore

logger = logging.getLogger("ollama_router")


def setup_logging(config: Config) -> None:
    """Configure logging with file rotation and request ID filter."""
    log_config = config.logging

    # Root logger setup
    logger = logging.getLogger("ollama_router")
    logger.setLevel(getattr(logging, log_config.level.upper(), logging.INFO))

    # Clear existing handlers
    logger.handlers.clear()

    # Format with request ID
    formatter = logging.Formatter(
        "%(asctime)s.%(msecs)03d %(levelname)-8s [%(request_id)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    # Request ID filter
    request_id_filter = RequestIdFilter()

    # Console handler
    console_handler = logging.StreamHandler()
    console_handler.setFormatter(formatter)
    console_handler.addFilter(request_id_filter)
    logger.addHandler(console_handler)

    # File handler (if configured)
    if log_config.file:
        try:
            log_path = Path(log_config.file)
            log_path.parent.mkdir(parents=True, exist_ok=True)

            file_handler = RotatingFileHandler(
                filename=str(log_path),
                maxBytes=log_config.max_size_mb * 1024 * 1024,
                backupCount=log_config.backup_count,
                encoding="utf-8",
            )
            file_handler.setFormatter(formatter)
            file_handler.addFilter(request_id_filter)
            logger.addHandler(file_handler)
        except Exception as e:
            logger.warning("Failed to setup file logging: %s", e)


class RequestIdMiddleware(BaseHTTPMiddleware):
    """Middleware to set request ID and add to response header."""

    async def dispatch(self, request, call_next):
        request_id = generate_request_id()
        token = set_request_id(request_id)

        try:
            response = await call_next(request)
            response.headers["X-Request-ID"] = request_id
            return response
        finally:
            request_id_var.reset(token)


# Hop-by-hop headers that must not be forwarded between proxy hops.
_HOP_BY_HOP_HEADERS = frozenset(
    {
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailers",
        "transfer-encoding",
        "upgrade",
        "content-length",
        "content-encoding",
    }
)


def _safe_response_headers(response: httpx.Response) -> dict[str, str]:
    """Extract headers from upstream response, stripping hop-by-hop headers."""
    return {
        k: v
        for k, v in response.headers.items()
        if k.lower() not in _HOP_BY_HOP_HEADERS
    }


def create_app(config: Config, state_dir: str = "state") -> FastAPI:
    app = FastAPI()

    # Setup logging and request ID middleware
    setup_logging(config)
    app.add_middleware(RequestIdMiddleware)

    state_store = StateStore(state_dir=state_dir)
    state_store.load()
    config_key_set = set(config.keys)
    state_keys_dict = {k.key: k for k in state_store.keys}

    keys_from_config = [state_keys_dict.get(k, KeyState(key=k)) for k in config.keys]
    admin_added = [ks for ks in state_store.keys if ks.key not in config_key_set]
    state_store.keys = keys_from_config + admin_added

    selector = KeySelector(
        state_store.keys,
        index=state_store.current_index,
        last_failed_key=state_store.last_failed_key,
    )
    handler = RateLimitHandler(
        cooldown_session_hours=config.cooldown_session_hours,
        cooldown_weekly_hours=config.cooldown_weekly_hours,
        cooldown_rate_hours=config.cooldown_rate_hours,
    )
    proxy = ProxyClient(
        upstream=config.upstream,
        proxy_http=config.proxy_http,
        proxy_https=config.proxy_https,
    )
    request_history = RequestHistory(max_size=1000)

    retry_manager = RetryManager(
        selector=selector,
        handler=handler,
        state_store=state_store,
        history=request_history,
    )

    app.state.config = config
    app.state.selector = selector
    app.state.state_store = state_store
    app.state.request_history = request_history
    app.state.retry_manager = retry_manager
    app.state.templates = Jinja2Templates(directory="templates")
    app.state.proxy = proxy

    app.include_router(create_admin_api_router())
    app.include_router(create_admin_views_router())
    app.include_router(create_anthropic_router())

    @app.get("/health")
    async def health():
        keys_info = []
        for k in selector.keys:
            info = {
                "key_id": get_key_id(k.key),
                "status": k.status.value,
                "cooldown_until": k.cooldown_until.isoformat()
                if k.cooldown_until
                else None,
                "reason": k.reason,
                "is_available": k.is_available(),
            }
            if k.cooldown_until:
                remaining = (
                    k.cooldown_until - datetime.now(timezone.utc)
                ).total_seconds() / 60
                info["remaining_cooldown_minutes"] = max(0, int(remaining))
            keys_info.append(info)

        content = json.dumps(
            {
                "status": "ok",
                "available_keys": sum(1 for k in selector.keys if k.is_available()),
                "total_keys": len(selector.keys),
                "keys": keys_info,
            },
            indent=2,
        )
        return Response(content=content, media_type="application/json")

    @app.get("/metrics")
    async def metrics_endpoint():
        return Response(
            content=metrics.generate(), media_type="text/plain; version=0.0.4"
        )

    @app.api_route(
        "/{path:path}",
        methods=["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    )
    async def proxy_chat(path: str, request: Request):
        logger.info("request_start method=%s path=%s", request.method, path)
        metrics.inc("requests_total", {"method": request.method, "path": path})

        body = None
        if request.method in ("POST", "PUT", "PATCH"):
            body = await request.json()

        result = await retry_manager.execute_with_retry(
            method=request.method,
            path=f"/{path}",
            headers=dict(request.headers),
            body=body,
            proxy=proxy,
            request_id=get_request_id(),
        )

        if not result.success:
            if result.last_error == "No available API keys":
                return JSONResponse(
                    status_code=503,
                    content={
                        "error": "No available API keys. All keys are in cooldown."
                    },
                )
            if result.last_error and "timeout" in result.last_error.lower():
                return JSONResponse(
                    status_code=504,
                    content={"error": "Upstream server timeout."},
                )
            if result.response:
                return JSONResponse(
                    status_code=result.response.status_code,
                    content=result.response.json(),
                )
            return JSONResponse(
                status_code=502,
                content={"error": result.last_error or "Proxy error"},
            )

        assert result.response is not None
        return Response(
            content=result.response.content,
            status_code=result.response.status_code,
            headers=_safe_response_headers(result.response),
        )

    @app.on_event("shutdown")
    async def shutdown():
        state_store.current_index = selector.index
        state_store.last_failed_key = selector.last_failed_key
        state_store.save()
        await proxy.close()

    return app


def main():
    import uvicorn

    from ollama_router.config import load_config

    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s [%(name)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    config = load_config("config.yaml")
    app = create_app(config)
    logger.info(
        "starting server listen=%s upstream=%s keys=%d",
        config.listen,
        config.upstream,
        len(config.keys),
    )
    host, port = config.listen.split(":")
    uvicorn.run(app, host=host, port=int(port))


if __name__ == "__main__":
    main()
