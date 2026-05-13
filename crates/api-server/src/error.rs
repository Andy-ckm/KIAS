use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// API-level error that implements IntoResponse for axum
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, msg)
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, msg)
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, msg)
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, msg)
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": {
                "code": self.status.as_u16(),
                "message": self.message,
            }
        }));
        (self.status, body).into_response()
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.status, self.message)
    }
}

impl From<kias_common::KiasError> for ApiError {
    fn from(e: kias_common::KiasError) -> Self {
        use kias_common::KiasError;
        match e {
            KiasError::AgentNotFound(m) | KiasError::NodeNotFound(m) => ApiError::not_found(m),
            KiasError::AuthenticationFailed(m) => ApiError::unauthorized(m),
            KiasError::AuthorizationDenied(m) => ApiError::forbidden(m),
            KiasError::Validation(m) | KiasError::BadRequest(m) => ApiError::bad_request(m),
            KiasError::Conflict(m) => ApiError::conflict(m),
            KiasError::ServiceUnavailable(m) => ApiError::new(StatusCode::SERVICE_UNAVAILABLE, m),
            other => {
                tracing::error!("Internal error: {other}");
                ApiError::internal("Internal server error")
            }
        }
    }
}
