use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Logging middleware that records only non-sensitive request metadata.
///
/// Query strings are deliberately excluded because they can contain access
/// tokens, email addresses, search terms, document identifiers, or other
/// personal and confidential data. Correlation should use an explicit request
/// id header rather than copying user-controlled values into logs.
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let start = std::time::Instant::now();

    let response = next.run(request).await;

    let latency = start.elapsed();
    let status = response.status();

    tracing::info!(
        method = %method,
        path = %path,
        status = %status.as_u16(),
        latency_ms = %latency.as_millis(),
        "request completed"
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn create_logging_router() -> Router {
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .route(
                "/fail",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "error") }),
            )
            .layer(axum::middleware::from_fn(logging_middleware))
    }

    #[tokio::test]
    async fn test_logging_passes_through_200() {
        let app = create_logging_router();
        let resp = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_logging_passes_through_500() {
        let app = create_logging_router();
        let resp = app
            .oneshot(Request::builder().uri("/fail").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_logging_preserves_404() {
        let app = create_logging_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_logging_does_not_change_query_requests() {
        let app = create_logging_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/test?email=private%40example.invalid&token=do-not-log")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_logging_preserves_method() {
        let app = Router::new()
            .route("/post-only", axum::routing::post(|| async { "posted" }))
            .layer(axum::middleware::from_fn(logging_middleware));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/post-only")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            resp.status() == StatusCode::METHOD_NOT_ALLOWED
                || resp.status() == StatusCode::NOT_FOUND
        );
    }
}
