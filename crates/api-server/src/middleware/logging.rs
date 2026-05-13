use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Logging middleware that records method, path, status and latency for every request.
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    let response = next.run(request).await;

    let latency = start.elapsed();
    let status = response.status();

    tracing::info!(
        method = %method,
        path = %uri,
        status = %status.as_u16(),
        latency_ms = %latency.as_millis(),
        "request completed"
    );

    response
}
