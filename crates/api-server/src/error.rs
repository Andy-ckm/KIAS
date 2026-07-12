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
            KiasError::AgentNotFound(m) | KiasError::NodeNotFound(m) | KiasError::NotFound(m) => {
                ApiError::not_found(m)
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_constructors() {
        let e = ApiError::not_found("agent not found");
        assert_eq!(e.status, StatusCode::NOT_FOUND);
        assert_eq!(e.message, "agent not found");

        let e = ApiError::bad_request("invalid input");
        assert_eq!(e.status, StatusCode::BAD_REQUEST);
        assert_eq!(e.message, "invalid input");

        let e = ApiError::internal("something broke");
        assert_eq!(e.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(e.message, "something broke");

        let e = ApiError::conflict("already exists");
        assert_eq!(e.status, StatusCode::CONFLICT);
        assert_eq!(e.message, "already exists");

        let e = ApiError::unauthorized("no token");
        assert_eq!(e.status, StatusCode::UNAUTHORIZED);
        assert_eq!(e.message, "no token");

        let e = ApiError::forbidden("insufficient permissions");
        assert_eq!(e.status, StatusCode::FORBIDDEN);
        assert_eq!(e.message, "insufficient permissions");
    }

    #[test]
    fn test_api_error_display() {
        let e = ApiError::not_found("not here");
        assert_eq!(format!("{e}"), "[404 Not Found] not here");
    }

    #[test]
    fn test_api_error_from_string() {
        let e = ApiError::new(StatusCode::BAD_GATEWAY, "gateway down".to_string());
        assert_eq!(e.status, StatusCode::BAD_GATEWAY);
        assert_eq!(e.message, "gateway down");
    }

    #[test]
    fn test_api_error_from_kias_error_not_found() {
        let kias = kias_common::KiasError::AgentNotFound("abc".to_string());
        let api: ApiError = kias.into();
        assert_eq!(api.status, StatusCode::NOT_FOUND);

        let kias = kias_common::KiasError::NotFound("run".to_string());
        let api: ApiError = kias.into();
        assert_eq!(api.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_api_error_from_kias_error_auth() {
        let kias = kias_common::KiasError::AuthenticationFailed("bad token".to_string());
        let api: ApiError = kias.into();
        assert_eq!(api.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_api_error_from_kias_error_validation() {
        let kias = kias_common::KiasError::Validation("invalid field".to_string());
        let api: ApiError = kias.into();
        assert_eq!(api.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_api_error_from_kias_error_conflict() {
        let kias = kias_common::KiasError::Conflict("duplicate".to_string());
        let api: ApiError = kias.into();
        assert_eq!(api.status, StatusCode::CONFLICT);
    }
}
