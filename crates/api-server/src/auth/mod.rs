pub mod rbac;

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// JWT Claims structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    /// Subject (user identifier).
    pub sub: String,
    /// User role.
    pub role: Role,
    /// Expiration time (Unix timestamp).
    pub exp: u64,
    /// Issued at (Unix timestamp).
    pub iat: u64,
    /// Issuer.
    pub iss: String,
}

/// User roles for RBAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Role {
    Admin,
    Operator,
    Viewer,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Admin => write!(f, "Admin"),
            Role::Operator => write!(f, "Operator"),
            Role::Viewer => write!(f, "Viewer"),
        }
    }
}

impl FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Admin" => Ok(Role::Admin),
            "Operator" => Ok(Role::Operator),
            "Viewer" => Ok(Role::Viewer),
            _ => Err(format!("Invalid role: {s}")),
        }
    }
}

/// JWT configuration.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// HMAC secret for signing tokens.
    pub secret: String,
    /// Issuer claim.
    pub issuer: String,
    /// Token expiration in hours.
    pub expiration_hours: u64,
}

impl JwtConfig {
    pub fn new(
        secret: impl Into<String>,
        issuer: impl Into<String>,
        expiration_hours: u64,
    ) -> Self {
        Self {
            secret: secret.into(),
            issuer: issuer.into(),
            expiration_hours,
        }
    }
}

/// Generate a JWT token from claims.
///
/// The `exp`, `iat`, and `iss` fields in the returned claims are **not**
/// overwritten — callers are expected to populate them beforehand.
pub fn generate_token(
    claims: &Claims,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Validate a JWT token and return the decoded claims.
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

/// Create a new `Claims` with the given subject, role, and issuer, using the
/// current time for `iat` and adding `expiration_hours` for `exp`.
pub fn create_claims(subject: &str, role: Role, config: &JwtConfig) -> Claims {
    let now = chrono::Utc::now().timestamp() as u64;
    Claims {
        sub: subject.to_string(),
        role,
        iat: now,
        exp: now + config.expiration_hours * 3600,
        iss: config.issuer.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> JwtConfig {
        JwtConfig::new("test-secret-key", "kias-test", 24)
    }

    #[test]
    fn test_generate_and_validate_token() {
        let config = test_config();
        let claims = create_claims("user-1", Role::Admin, &config);
        let token = generate_token(&claims, &config.secret).unwrap();
        let decoded = validate_token(&token, &config.secret).unwrap();
        assert_eq!(decoded.sub, "user-1");
        assert_eq!(decoded.role, Role::Admin);
        assert_eq!(decoded.iss, "kias-test");
    }

    #[test]
    fn test_validate_token_wrong_secret() {
        let config = test_config();
        let claims = create_claims("user-1", Role::Admin, &config);
        let token = generate_token(&claims, &config.secret).unwrap();
        let result = validate_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_expired_token() {
        let mut claims = Claims {
            sub: "user-1".to_string(),
            role: Role::Admin,
            iat: 1000,
            exp: 1001, // already expired
            iss: "kias-test".to_string(),
        };
        // Force expiration to be in the past
        claims.exp = 1; // Unix timestamp 1 = 1970
        let token = generate_token(&claims, "secret").unwrap();
        let result = validate_token(&token, "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_malformed_token() {
        let result = validate_token("not.a.valid.token", "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_claims_serialization_roundtrip() {
        let config = test_config();
        let claims = create_claims("user-42", Role::Operator, &config);
        let json = serde_json::to_string(&claims).unwrap();
        let deserialized: Claims = serde_json::from_str(&json).unwrap();
        assert_eq!(claims, deserialized);
    }

    #[test]
    fn test_role_display_and_from_str() {
        let roles = vec![Role::Admin, Role::Operator, Role::Viewer];
        for role in &roles {
            let s = role.to_string();
            let parsed: Role = s.parse().unwrap();
            assert_eq!(*role, parsed);
        }
    }

    #[test]
    fn test_role_from_str_invalid() {
        let result: Result<Role, _> = "Superuser".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_role_serialization_camel_case() {
        let role = Role::Admin;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"Admin\"");
    }

    #[test]
    fn test_create_claims_sets_correct_fields() {
        let config = test_config();
        let claims = create_claims("alice", Role::Viewer, &config);
        assert_eq!(claims.sub, "alice");
        assert_eq!(claims.role, Role::Viewer);
        assert_eq!(claims.iss, "kias-test");
        assert!(claims.exp > claims.iat);
        assert_eq!(claims.exp - claims.iat, 24 * 3600);
    }

    #[test]
    fn test_token_with_different_roles() {
        let config = test_config();
        for role in [Role::Admin, Role::Operator, Role::Viewer] {
            let claims = create_claims("user", role, &config);
            let token = generate_token(&claims, &config.secret).unwrap();
            let decoded = validate_token(&token, &config.secret).unwrap();
            assert_eq!(decoded.role, role);
        }
    }
}
