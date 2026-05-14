use std::fmt;

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;

/// Supported API versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum ApiVersion {
    #[default]
    V1,
    V2,
}

impl ApiVersion {
    /// Numeric representation (e.g. `1`, `2`).
    pub fn number(&self) -> u32 {
        match self {
            ApiVersion::V1 => 1,
            ApiVersion::V2 => 2,
        }
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiVersion::V1 => write!(f, "v1"),
            ApiVersion::V2 => write!(f, "v2"),
        }
    }
}

impl std::str::FromStr for ApiVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "v1" | "1" => Ok(ApiVersion::V1),
            "v2" | "2" => Ok(ApiVersion::V2),
            other => Err(format!("Unknown API version: {other}")),
        }
    }
}

/// Axum extractor that resolves the requested API version.
///
/// Resolution order (highest priority first):
/// 1. `X-API-Version` header
/// 2. `Accept: application/vnd.kias.v1+json` header
/// 3. URL path prefix `/api/v1/...`
/// 4. Default → `V1`
#[derive(Debug, Clone)]
pub struct VersionExtractor(pub ApiVersion);

#[async_trait]
impl<S> FromRequestParts<S> for VersionExtractor
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // 1. Check X-API-Version header
        if let Some(val) = parts.headers.get("x-api-version") {
            if let Ok(s) = val.to_str() {
                if let Ok(v) = s.parse::<ApiVersion>() {
                    return Ok(VersionExtractor(v));
                }
            }
        }

        // 2. Check Accept header for vendor media type
        if let Some(accept) = parts.headers.get("accept") {
            if let Ok(val) = accept.to_str() {
                if let Some(version) = parse_accept_version(val) {
                    return Ok(VersionExtractor(version));
                }
            }
        }

        // 3. Check URL path for /api/vN/
        if let Some(version) = extract_version_from_path(parts.uri.path()) {
            return Ok(VersionExtractor(version));
        }

        // 4. Default
        Ok(VersionExtractor(ApiVersion::default()))
    }
}

/// Parse the `Accept` header value for `application/vnd.kias.vN+json`.
fn parse_accept_version(accept: &str) -> Option<ApiVersion> {
    // Look for "application/vnd.kias.vN+json" anywhere in the Accept value
    // (may contain multiple comma-separated types).
    for media_type in accept.split(',').map(str::trim) {
        if let Some(rest) = media_type.strip_prefix("application/vnd.kias.v") {
            // Extract "N" from "application/vnd.kias.vN+json"

            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(v) = num_str.parse::<u32>() {
                return match v {
                    1 => Some(ApiVersion::V1),
                    2 => Some(ApiVersion::V2),
                    _ => None,
                };
            }
        }
    }
    None
}

/// Extract the version from the URL path, e.g. `/api/v2/agents` → `V2`.
fn extract_version_from_path(path: &str) -> Option<ApiVersion> {
    // Match `/api/vN/` or `/api/vN` at the start
    let stripped = path.strip_prefix("/api/")?;
    let version_part = stripped.split('/').next().unwrap_or(stripped);
    version_part.parse::<ApiVersion>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::request::Parts;

    /// Helper: build `Parts` with given headers and path.
    fn make_parts(headers: Vec<(&str, &str)>, path: &str) -> Parts {
        let req = axum::http::Request::builder()
            .method(axum::http::Method::GET)
            .uri(path)
            .body(())
            .unwrap();
        let (mut parts, _) = req.into_parts();
        for (name, value) in headers {
            parts.headers.insert(
                name.parse::<axum::http::header::HeaderName>().unwrap(),
                value.parse().unwrap(),
            );
        }
        parts
    }

    // ── Version parsing tests ──────────────────────────────────────────

    #[test]
    fn test_version_from_str_v1() {
        assert_eq!("v1".parse::<ApiVersion>().unwrap(), ApiVersion::V1);
        assert_eq!("V1".parse::<ApiVersion>().unwrap(), ApiVersion::V1);
        assert_eq!("1".parse::<ApiVersion>().unwrap(), ApiVersion::V1);
    }

    #[test]
    fn test_version_from_str_v2() {
        assert_eq!("v2".parse::<ApiVersion>().unwrap(), ApiVersion::V2);
        assert_eq!("2".parse::<ApiVersion>().unwrap(), ApiVersion::V2);
    }

    #[test]
    fn test_version_from_str_invalid() {
        assert!("v3".parse::<ApiVersion>().is_err());
        assert!("foo".parse::<ApiVersion>().is_err());
    }

    #[test]
    fn test_version_display() {
        assert_eq!(ApiVersion::V1.to_string(), "v1");
        assert_eq!(ApiVersion::V2.to_string(), "v2");
    }

    #[test]
    fn test_version_default_is_v1() {
        assert_eq!(ApiVersion::default(), ApiVersion::V1);
    }

    #[test]
    fn test_version_number() {
        assert_eq!(ApiVersion::V1.number(), 1);
        assert_eq!(ApiVersion::V2.number(), 2);
    }

    // ── Accept header parsing tests ────────────────────────────────────

    #[test]
    fn test_parse_accept_version_v1() {
        assert_eq!(
            parse_accept_version("application/vnd.kias.v1+json"),
            Some(ApiVersion::V1)
        );
    }

    #[test]
    fn test_parse_accept_version_v2() {
        assert_eq!(
            parse_accept_version("application/vnd.kias.v2+json"),
            Some(ApiVersion::V2)
        );
    }

    #[test]
    fn test_parse_accept_version_with_multiple_types() {
        let accept = "text/html, application/vnd.kias.v2+json, application/json";
        assert_eq!(parse_accept_version(accept), Some(ApiVersion::V2));
    }

    #[test]
    fn test_parse_accept_version_no_match() {
        assert_eq!(parse_accept_version("application/json"), None);
    }

    // ── URL path extraction tests ──────────────────────────────────────

    #[test]
    fn test_extract_version_from_path_v1() {
        assert_eq!(
            extract_version_from_path("/api/v1/agents"),
            Some(ApiVersion::V1)
        );
    }

    #[test]
    fn test_extract_version_from_path_v2() {
        assert_eq!(
            extract_version_from_path("/api/v2/agents"),
            Some(ApiVersion::V2)
        );
    }

    #[test]
    fn test_extract_version_from_path_no_api_prefix() {
        assert_eq!(extract_version_from_path("/health"), None);
    }

    #[test]
    fn test_extract_version_from_path_bare_api() {
        // "/api/" with nothing after → no version segment
        assert_eq!(extract_version_from_path("/api/"), None);
    }

    // ── VersionExtractor priority tests ────────────────────────────────

    #[tokio::test]
    async fn test_extractor_prefers_header() {
        let mut parts = make_parts(vec![("x-api-version", "v2")], "/api/v1/agents");
        let ext = VersionExtractor::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(ext.0, ApiVersion::V2);
    }

    #[tokio::test]
    async fn test_extractor_accept_header() {
        let mut parts = make_parts(
            vec![("accept", "application/vnd.kias.v2+json")],
            "/api/v1/agents",
        );
        let ext = VersionExtractor::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(ext.0, ApiVersion::V2);
    }

    #[tokio::test]
    async fn test_extractor_path_fallback() {
        let mut parts = make_parts(vec![], "/api/v2/agents");
        let ext = VersionExtractor::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(ext.0, ApiVersion::V2);
    }

    #[tokio::test]
    async fn test_extractor_default_v1() {
        let mut parts = make_parts(vec![], "/health");
        let ext = VersionExtractor::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(ext.0, ApiVersion::V1);
    }

    #[tokio::test]
    async fn test_extractor_x_header_overrides_accept() {
        let mut parts = make_parts(
            vec![
                ("x-api-version", "v1"),
                ("accept", "application/vnd.kias.v2+json"),
            ],
            "/api/v2/agents",
        );
        let ext = VersionExtractor::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        // X-API-Version has highest priority
        assert_eq!(ext.0, ApiVersion::V1);
    }
}
