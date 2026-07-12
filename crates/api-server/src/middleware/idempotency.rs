//! Idempotency middleware for API-level duplicate request detection.
//!
//! Implements end-to-end idempotency for state-mutating requests (POST/PUT/PATCH)
//! using the `X-Idempotency-Key` header.
//!
//! # Behavior
//!
//! - **With `X-Idempotency-Key`**: Checks idempotency store for cached response.
//!   If found and operation matches → returns cached response immediately.
//!   Otherwise → processes request and caches the response.
//! - **Without header**: Passes through unchanged (idempotent by default for GET).
//!
//! # Response Caching
//!
//! Cached responses are stored in SQLite with a configurable TTL (default 24h).
//! The cache key is the idempotency key itself; the operation hash (method+path+body)
//! prevents false positives when the same key is reused for different operations.
//! Request bodies are hashed for comparison but never persisted by this middleware.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use bytes::Bytes;
use sha2::{Digest, Sha256};

use crate::AppState;

/// Default TTL for idempotency keys (24 hours).
const DEFAULT_TTL_SECONDS: i64 = 86400;

/// Header name for idempotency key (standard convention).
pub const IDEMPOTENCY_KEY_HEADER: &str = "X-Idempotency-Key";

/// Idempotency middleware.
///
/// Only applies to POST, PUT, PATCH methods that carry an `X-Idempotency-Key`.
/// GET/DELETE requests pass through without idempotency checks.
pub async fn idempotency_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    // Only apply to mutating methods with idempotency key
    let method = request.method().clone();
    if !matches!(method, Method::POST | Method::PUT | Method::PATCH) {
        return next.run(request).await;
    }

    // Extract idempotency key
    let idempotency_key = match request.headers().get(IDEMPOTENCY_KEY_HEADER) {
        Some(v) => match v.to_str() {
            Ok(s) if !s.is_empty() => s.to_string(),
            _ => return next.run(request).await,
        },
        None => return next.run(request).await,
    };

    // No store configured → skip idempotency
    let store = match state.idempotency_store.as_ref() {
        Some(s) => s,
        None => return next.run(request).await,
    };

    // Read and buffer the request body
    let (parts, body) = request.into_parts();
    let body_bytes = axum::body::to_bytes(body, 1024 * 1024)
        .await
        .unwrap_or_else(|_| Bytes::new())
        .to_vec();
    let path = parts.uri.path().to_string();
    let body_hash = compute_operation_hash(&method, &path, &body_bytes);

    // Check for cached response
    match store.get_by_key(&idempotency_key).await {
        Ok(Some(entry)) if entry.operation_hash == body_hash && entry.is_completed() => {
            tracing::debug!(key = %idempotency_key, "idempotency: cache hit");
            let status =
                StatusCode::from_u16(entry.response_status as u16).unwrap_or(StatusCode::OK);
            let body = entry.response_body.unwrap_or_default();
            let mut resp = Response::new(Body::from(Bytes::copy_from_slice(body.as_slice())));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                "X-Idempotency-Replayed",
                axum::http::HeaderValue::from_static("true"),
            );
            return resp;
        }
        Ok(Some(_)) => {
            // Same key, different operation → conflict
            tracing::warn!(key = %idempotency_key, "idempotency: key reuse with different operation");
            return Response::builder()
                .status(StatusCode::CONFLICT)
                .body(Body::from("Idempotency key reused for different operation"))
                .unwrap();
        }
        _ => {}
    }

    // Register only the operation digest. Persisting a request body would turn the
    // idempotency table into a second, less-governed copy of prompts or PII.
    let pending = kias_data_store::models::IdempotencyRow::new_pending(
        &idempotency_key,
        method.to_string(),
        path,
        body_hash,
        None,
        DEFAULT_TTL_SECONDS,
    );
    if let Err(e) = store.insert_pending(&pending).await {
        tracing::warn!(error = %e, "idempotency: failed to insert pending entry");
    }

    // Reconstruct request with buffered body and run handler
    let request = Request::from_parts(
        parts,
        Body::from(Bytes::copy_from_slice(body_bytes.as_slice())),
    );
    let response = next.run(request).await;

    // Cache the response (best-effort)
    let status = response.status().as_u16() as i32;
    if let Err(e) = store
        .complete(&idempotency_key, status, Vec::new(), "{}")
        .await
    {
        tracing::warn!(error = %e, "idempotency: failed to cache response");
    }

    response
}

/// Compute operation hash: SHA256(method + ":" + path + ":" + body_sha256).
fn compute_operation_hash(method: &Method, path: &str, body: &[u8]) -> String {
    let body_hash = format!("{:x}", Sha256::digest(body));
    format!("{}:{}:{}", method, path, body_hash)
}
