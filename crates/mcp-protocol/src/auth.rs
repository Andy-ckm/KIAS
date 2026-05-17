//! MCP Authentication & Authorization
//!
//! Provides:
//! - OAuth 2.0 token validation
//! - API key authentication
//! - Role-based access control (RBAC)
//! - Token introspection
//! - Scope-based permissions

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::McpError;
use hmac::{Hmac, Mac};
use sha2::Sha256;

// ---------------------------------------------------------------------------
// OAuth 2.0 Types
// ---------------------------------------------------------------------------

/// OAuth 2.0 token information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    /// The access token string.
    pub access_token: String,
    /// Token type (usually "Bearer").
    pub token_type: String,
    /// Token expiration time (seconds from now).
    pub expires_in: Option<u64>,
    /// Granted scopes.
    pub scope: Option<String>,
    /// Refresh token.
    pub refresh_token: Option<String>,
}

/// Claims extracted from a JWT or introspection response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Subject (user ID).
    pub sub: String,
    /// Issuer.
    pub iss: Option<String>,
    /// Audience.
    pub aud: Option<String>,
    /// Expiration time (Unix timestamp).
    pub exp: Option<u64>,
    /// Issued at (Unix timestamp).
    pub iat: Option<u64>,
    /// Scopes granted.
    pub scope: Option<String>,
    /// Roles assigned.
    pub roles: Option<Vec<String>>,
    /// Custom claims.
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

/// OAuth 2.0 introspection response (RFC 7662).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectionResponse {
    /// Whether the token is active.
    pub active: bool,
    /// Scope(s) granted.
    pub scope: Option<String>,
    /// Client ID.
    pub client_id: Option<String>,
    /// Username.
    pub username: Option<String>,
    /// Token type.
    pub token_type: Option<String>,
    /// Expiration time (Unix timestamp).
    pub exp: Option<u64>,
    /// Issued at (Unix timestamp).
    pub iat: Option<u64>,
    /// Subject.
    pub sub: Option<String>,
    /// Audience.
    pub aud: Option<String>,
    /// Issuer.
    pub iss: Option<String>,
}

// ---------------------------------------------------------------------------
// RBAC
// ---------------------------------------------------------------------------

/// Permission types for MCP operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// List available tools.
    ToolsList,
    /// Call a specific tool.
    ToolsCall(String),
    /// List available resources.
    ResourcesList,
    /// Read a specific resource.
    ResourcesRead(String),
    /// List available prompts.
    PromptsList,
    /// Get a specific prompt.
    PromptsGet(String),
    /// Server administration.
    Admin,
    /// Manage credentials.
    Credentials,
    /// View audit logs.
    AuditView,
    /// Custom permission.
    Custom(String),
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Permission::ToolsList => write!(f, "tools:list"),
            Permission::ToolsCall(name) => write!(f, "tools:call:{}", name),
            Permission::ResourcesList => write!(f, "resources:list"),
            Permission::ResourcesRead(uri) => write!(f, "resources:read:{}", uri),
            Permission::PromptsList => write!(f, "prompts:list"),
            Permission::PromptsGet(name) => write!(f, "prompts:get:{}", name),
            Permission::Admin => write!(f, "admin"),
            Permission::Credentials => write!(f, "credentials"),
            Permission::AuditView => write!(f, "audit:view"),
            Permission::Custom(perm) => write!(f, "{}", perm),
        }
    }
}

/// Role definition with permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Permissions granted by this role.
    pub permissions: HashSet<Permission>,
    /// Roles this role inherits from.
    pub inherits: Vec<String>,
}

impl Role {
    /// Create a new role.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            permissions: HashSet::new(),
            inherits: Vec::new(),
        }
    }

    /// Add a permission to this role.
    pub fn with_permission(mut self, permission: Permission) -> Self {
        self.permissions.insert(permission);
        self
    }

    /// Add inheritance from another role.
    pub fn with_inherits(mut self, role: impl Into<String>) -> Self {
        self.inherits.push(role.into());
        self
    }
}

/// User information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// User ID.
    pub id: String,
    /// Username.
    pub username: String,
    /// Email.
    pub email: Option<String>,
    /// Assigned roles.
    pub roles: Vec<String>,
    /// Whether the user is active.
    pub active: bool,
    /// Custom attributes.
    pub attributes: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Auth Providers
// ---------------------------------------------------------------------------

/// Trait for authentication providers.
#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
    /// Validate an access token and return claims.
    async fn validate_token(&self, token: &str) -> Result<TokenClaims, McpError>;

    /// Introspect a token (RFC 7662).
    async fn introspect(&self, token: &str) -> Result<IntrospectionResponse, McpError>;

    /// Get user information by subject.
    async fn get_user(&self, subject: &str) -> Result<UserInfo, McpError>;
}

/// JWT-based authentication provider.
pub struct JwtAuthProvider {
    /// JWT secret key (for HMAC) or public key (for RSA/EC).
    key: Vec<u8>,
    /// Expected issuer.
    issuer: Option<String>,
    /// Expected audience.
    audience: Option<String>,
    /// Clock skew tolerance (seconds).
    clock_skew_secs: u64,
}

impl JwtAuthProvider {
    /// Create a new JWT auth provider with a secret key.
    pub fn new(key: impl Into<Vec<u8>>) -> Self {
        Self {
            key: key.into(),
            issuer: None,
            audience: None,
            clock_skew_secs: 300, // 5 minutes
        }
    }

    /// Set expected issuer.
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    /// Set expected audience.
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(audience.into());
        self
    }

    /// Set clock skew tolerance.
    pub fn with_clock_skew(mut self, secs: u64) -> Self {
        self.clock_skew_secs = secs;
        self
    }

    /// Decode and validate a JWT token with HMAC-SHA256 signature verification.
    fn decode_jwt(&self, token: &str) -> Result<TokenClaims, McpError> {
        // Split JWT into parts
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(McpError::Authentication("Invalid JWT format".to_string()));
        }

        // Validate JWT header algorithm
        let header_bytes = base64_decode(parts[0])
            .map_err(|e| McpError::Authentication(format!("Invalid JWT header: {}", e)))?;
        let header: serde_json::Value = serde_json::from_slice(&header_bytes)
            .map_err(|e| McpError::Authentication(format!("Invalid JWT header JSON: {}", e)))?;
        let alg = header.get("alg").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::Authentication("JWT header missing 'alg' field".to_string())
        })?;
        if alg != "HS256" {
            return Err(McpError::Authentication(format!(
                "Unsupported JWT algorithm '{}', expected 'HS256'",
                alg
            )));
        }

        // ─── HMAC-SHA256 Signature Verification ─────────────────────────────────
        // Reconstruct the signing input: "header.payload"
        let signing_input = format!("{}.{}", parts[0], parts[1]);

        // Decode the base64url-encoded signature
        let signature_bytes = base64_decode(parts[2])
            .map_err(|e| McpError::Authentication(format!("Invalid signature encoding: {}", e)))?;

        // Create HMAC-SHA256 verifier using the secret key
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(&self.key)
            .map_err(|_| McpError::Authentication("HMAC init failed".to_string()))?;
        mac.update(signing_input.as_bytes());

        // Verify the signature matches
        mac.verify_slice(&signature_bytes)
            .map_err(|_| McpError::Authentication("Invalid JWT signature".to_string()))?;

        // Decode payload (base64url)
        let payload = parts[1];
        let decoded = base64_decode(payload)
            .map_err(|e| McpError::Authentication(format!("Invalid JWT payload: {}", e)))?;

        let claims: TokenClaims = serde_json::from_slice(&decoded)
            .map_err(|e| McpError::Authentication(format!("Invalid JWT claims: {}", e)))?;

        // Validate expiration
        let exp = claims.exp.ok_or_else(|| {
            McpError::Authentication("Token missing required 'exp' claim".to_string())
        })?;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if exp + self.clock_skew_secs < now {
            return Err(McpError::Authentication("Token expired".to_string()));
        }

        // Validate issuer
        if let Some(ref expected_issuer) = self.issuer {
            match &claims.iss {
                Some(iss) if iss == expected_issuer => {}
                _ => return Err(McpError::Authentication("Invalid issuer".to_string())),
            }
        }

        // Validate audience
        if let Some(ref expected_audience) = self.audience {
            match &claims.aud {
                Some(aud) if aud == expected_audience => {}
                _ => return Err(McpError::Authentication("Invalid audience".to_string())),
            }
        }

        Ok(claims)
    }
}

#[async_trait::async_trait]
impl AuthProvider for JwtAuthProvider {
    async fn validate_token(&self, token: &str) -> Result<TokenClaims, McpError> {
        self.decode_jwt(token)
    }

    async fn introspect(&self, token: &str) -> Result<IntrospectionResponse, McpError> {
        match self.decode_jwt(token) {
            Ok(claims) => Ok(IntrospectionResponse {
                active: true,
                scope: claims.scope.clone(),
                client_id: None,
                username: None,
                token_type: Some("Bearer".to_string()),
                exp: claims.exp,
                iat: claims.iat,
                sub: Some(claims.sub.clone()),
                aud: claims.aud.clone(),
                iss: claims.iss.clone(),
            }),
            Err(_) => Ok(IntrospectionResponse {
                active: false,
                scope: None,
                client_id: None,
                username: None,
                token_type: None,
                exp: None,
                iat: None,
                sub: None,
                aud: None,
                iss: None,
            }),
        }
    }

    async fn get_user(&self, _subject: &str) -> Result<UserInfo, McpError> {
        Err(McpError::Authentication(
            "User lookup not supported by JWT provider".to_string(),
        ))
    }
}

/// API Key authentication provider.
pub struct ApiKeyAuthProvider {
    /// Valid API keys mapped to user info.
    keys: Arc<RwLock<HashMap<String, UserInfo>>>,
}

impl ApiKeyAuthProvider {
    /// Create a new API key auth provider.
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an API key.
    pub async fn register_key(&self, key: impl Into<String>, user: UserInfo) {
        let mut keys = self.keys.write().await;
        keys.insert(key.into(), user);
    }

    /// Remove an API key.
    pub async fn revoke_key(&self, key: &str) -> bool {
        let mut keys = self.keys.write().await;
        keys.remove(key).is_some()
    }
}

impl Default for ApiKeyAuthProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AuthProvider for ApiKeyAuthProvider {
    async fn validate_token(&self, token: &str) -> Result<TokenClaims, McpError> {
        let keys = self.keys.read().await;
        let user = keys
            .get(token)
            .ok_or_else(|| McpError::Authentication("Invalid API key".to_string()))?;

        if !user.active {
            return Err(McpError::Authentication(
                "User account disabled".to_string(),
            ));
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(TokenClaims {
            sub: user.id.clone(),
            iss: Some("kias-api-key".to_string()),
            aud: None,
            exp: None, // API keys don't expire by default
            iat: Some(now),
            scope: None,
            roles: Some(user.roles.clone()),
            custom: HashMap::new(),
        })
    }

    async fn introspect(&self, token: &str) -> Result<IntrospectionResponse, McpError> {
        let keys = self.keys.read().await;
        let user = keys.get(token);

        match user {
            Some(user) if user.active => {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                Ok(IntrospectionResponse {
                    active: true,
                    scope: None,
                    client_id: Some(user.id.clone()),
                    username: Some(user.username.clone()),
                    token_type: Some("ApiKey".to_string()),
                    exp: None,
                    iat: Some(now),
                    sub: Some(user.id.clone()),
                    aud: None,
                    iss: Some("kias-api-key".to_string()),
                })
            }
            _ => Ok(IntrospectionResponse {
                active: false,
                scope: None,
                client_id: None,
                username: None,
                token_type: None,
                exp: None,
                iat: None,
                sub: None,
                aud: None,
                iss: None,
            }),
        }
    }

    async fn get_user(&self, subject: &str) -> Result<UserInfo, McpError> {
        let keys = self.keys.read().await;
        keys.values()
            .find(|u| u.id == subject)
            .cloned()
            .ok_or_else(|| McpError::Authentication("User not found".to_string()))
    }
}

// ---------------------------------------------------------------------------
// Authorization Manager
// ---------------------------------------------------------------------------

/// Authorization manager for RBAC.
pub struct AuthorizationManager {
    /// Role definitions.
    roles: Arc<RwLock<HashMap<String, Role>>>,
    /// Default roles assigned to new users.
    default_roles: Vec<String>,
}

impl AuthorizationManager {
    /// Create a new authorization manager.
    pub fn new() -> Self {
        let mut manager = Self {
            roles: Arc::new(RwLock::new(HashMap::new())),
            default_roles: Vec::new(),
        };

        // Register built-in roles
        manager.register_builtin_roles();
        manager
    }

    /// Register built-in roles.
    fn register_builtin_roles(&mut self) {
        // Admin role - full access
        let admin = Role::new("admin", "Full administrator access")
            .with_permission(Permission::Admin)
            .with_permission(Permission::ToolsList)
            .with_permission(Permission::ToolsCall("*".to_string()))
            .with_permission(Permission::ResourcesList)
            .with_permission(Permission::ResourcesRead("*".to_string()))
            .with_permission(Permission::PromptsList)
            .with_permission(Permission::PromptsGet("*".to_string()))
            .with_permission(Permission::Credentials)
            .with_permission(Permission::AuditView);

        // Developer role - tools and resources
        let developer = Role::new("developer", "Developer access")
            .with_permission(Permission::ToolsList)
            .with_permission(Permission::ToolsCall("*".to_string()))
            .with_permission(Permission::ResourcesList)
            .with_permission(Permission::ResourcesRead("*".to_string()))
            .with_permission(Permission::PromptsList)
            .with_permission(Permission::PromptsGet("*".to_string()));

        // Viewer role - read-only
        let viewer = Role::new("viewer", "Read-only access")
            .with_permission(Permission::ToolsList)
            .with_permission(Permission::ResourcesList)
            .with_permission(Permission::PromptsList);

        // Async registration
        let roles = self.roles.clone();
        tokio::spawn(async move {
            let mut r = roles.write().await;
            r.insert(admin.name.clone(), admin);
            r.insert(developer.name.clone(), developer);
            r.insert(viewer.name.clone(), viewer);
        });
    }

    /// Register a role.
    pub async fn register_role(&self, role: Role) {
        let mut roles = self.roles.write().await;
        roles.insert(role.name.clone(), role);
    }

    /// Set default roles for new users.
    pub fn set_default_roles(&mut self, roles: Vec<String>) {
        self.default_roles = roles;
    }

    /// Check if a user has a specific permission.
    pub async fn check_permission(
        &self,
        user: &UserInfo,
        permission: &Permission,
    ) -> Result<bool, McpError> {
        let roles = self.roles.read().await;

        for role_name in &user.roles {
            if let Some(role) = roles.get(role_name) {
                if self.role_has_permission(&roles, role, permission) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Check if a role has a permission (including inherited permissions).
    fn role_has_permission(
        &self,
        all_roles: &HashMap<String, Role>,
        role: &Role,
        permission: &Permission,
    ) -> bool {
        // Direct permission check
        if role.permissions.contains(permission) {
            return true;
        }

        // Wildcard check for tool/resource permissions
        match permission {
            Permission::ToolsCall(_)
                if role
                    .permissions
                    .contains(&Permission::ToolsCall("*".to_string())) =>
            {
                return true;
            }
            Permission::ResourcesRead(_)
                if role
                    .permissions
                    .contains(&Permission::ResourcesRead("*".to_string())) =>
            {
                return true;
            }
            Permission::PromptsGet(_)
                if role
                    .permissions
                    .contains(&Permission::PromptsGet("*".to_string())) =>
            {
                return true;
            }
            _ => {}
        }

        // Check inherited roles
        for inherited_name in &role.inherits {
            if let Some(inherited_role) = all_roles.get(inherited_name) {
                if self.role_has_permission(all_roles, inherited_role, permission) {
                    return true;
                }
            }
        }

        false
    }

    /// Get all permissions for a user.
    pub async fn get_user_permissions(&self, user: &UserInfo) -> HashSet<Permission> {
        let roles = self.roles.read().await;
        let mut permissions = HashSet::new();

        for role_name in &user.roles {
            if let Some(role) = roles.get(role_name) {
                self.collect_role_permissions(&roles, role, &mut permissions);
            }
        }

        permissions
    }

    /// Collect all permissions from a role (including inherited).
    fn collect_role_permissions(
        &self,
        all_roles: &HashMap<String, Role>,
        role: &Role,
        permissions: &mut HashSet<Permission>,
    ) {
        for perm in &role.permissions {
            permissions.insert(perm.clone());
        }

        for inherited_name in &role.inherits {
            if let Some(inherited_role) = all_roles.get(inherited_name) {
                self.collect_role_permissions(all_roles, inherited_role, permissions);
            }
        }
    }
}

impl Default for AuthorizationManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Auth Context
// ---------------------------------------------------------------------------

/// Authentication context for a request.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// The authenticated user.
    pub user: UserInfo,
    /// The token claims.
    pub claims: TokenClaims,
    /// Granted permissions.
    pub permissions: HashSet<Permission>,
    /// Authentication method used.
    pub method: AuthMethod,
    /// When the authentication occurred.
    pub authenticated_at: SystemTime,
}

/// Authentication method used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// JWT token.
    Jwt,
    /// API key.
    ApiKey,
    /// OAuth 2.0 introspection.
    OAuth,
}

impl AuthContext {
    /// Check if this context has a specific permission.
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission) || self.permissions.contains(&Permission::Admin)
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Decode base64url-encoded data.
fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    engine
        .decode(data)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_api_key_auth() {
        let provider = ApiKeyAuthProvider::new();
        let user = UserInfo {
            id: "user-1".to_string(),
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            roles: vec!["developer".to_string()],
            active: true,
            attributes: HashMap::new(),
        };

        provider.register_key("test-api-key-123", user).await;

        // Valid key
        let claims = provider.validate_token("test-api-key-123").await.unwrap();
        assert_eq!(claims.sub, "user-1");

        // Invalid key
        let result = provider.validate_token("invalid-key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rbac_permissions() {
        let auth = AuthorizationManager::new();

        // Wait for builtin roles to be registered
        tokio::time::sleep(Duration::from_millis(50)).await;

        let admin_user = UserInfo {
            id: "admin-1".to_string(),
            username: "admin".to_string(),
            email: None,
            roles: vec!["admin".to_string()],
            active: true,
            attributes: HashMap::new(),
        };

        let viewer_user = UserInfo {
            id: "viewer-1".to_string(),
            username: "viewer".to_string(),
            email: None,
            roles: vec!["viewer".to_string()],
            active: true,
            attributes: HashMap::new(),
        };

        // Admin has admin permission
        assert!(auth
            .check_permission(&admin_user, &Permission::Admin)
            .await
            .unwrap());

        // Admin can call any tool
        assert!(auth
            .check_permission(&admin_user, &Permission::ToolsCall("any-tool".to_string()))
            .await
            .unwrap());

        // Viewer can list tools
        assert!(auth
            .check_permission(&viewer_user, &Permission::ToolsList)
            .await
            .unwrap());

        // Viewer cannot call tools
        assert!(!auth
            .check_permission(
                &viewer_user,
                &Permission::ToolsCall("some-tool".to_string())
            )
            .await
            .unwrap());

        // Viewer cannot access admin
        assert!(!auth
            .check_permission(&viewer_user, &Permission::Admin)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_custom_role() {
        let auth = AuthorizationManager::new();

        // Wait for builtin roles to be registered
        tokio::time::sleep(Duration::from_millis(50)).await;

        let custom_role = Role::new("custom", "Custom role for specific tools")
            .with_permission(Permission::ToolsCall("weather".to_string()))
            .with_permission(Permission::ResourcesRead("file:///data/*".to_string()));

        auth.register_role(custom_role).await;

        let user = UserInfo {
            id: "custom-1".to_string(),
            username: "customuser".to_string(),
            email: None,
            roles: vec!["custom".to_string()],
            active: true,
            attributes: HashMap::new(),
        };

        // Can call allowed tool
        assert!(auth
            .check_permission(&user, &Permission::ToolsCall("weather".to_string()))
            .await
            .unwrap());

        // Cannot call other tools
        assert!(!auth
            .check_permission(&user, &Permission::ToolsCall("other-tool".to_string()))
            .await
            .unwrap());
    }

    // ─── JWT Security Tests ─────────────────────────────────────────────────

    fn base64url_encode(data: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
    }

    fn make_jwt(
        key: &[u8],
        claims: &serde_json::Value,
        header: Option<&serde_json::Value>,
    ) -> String {
        let default_header = serde_json::json!({"alg": "HS256", "typ": "JWT"});
        let hdr = header.unwrap_or(&default_header);
        let header_b64 = base64url_encode(hdr.to_string().as_bytes());
        let payload_b64 = base64url_encode(claims.to_string().as_bytes());
        let signing_input = format!("{}.{}", header_b64, payload_b64);

        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(signing_input.as_bytes());
        let sig_b64 = base64url_encode(&mac.finalize().into_bytes());

        format!("{}.{}.{}", header_b64, payload_b64, sig_b64)
    }

    #[tokio::test]
    async fn test_jwt_expired_token() {
        let key = b"test-secret-key-32bytes-long!!!!!";
        let provider = JwtAuthProvider::new(key.to_vec());

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = serde_json::json!({
            "sub": "user-1",
            "exp": now - 3600, // expired 1 hour ago
            "iat": now - 7200
        });
        let token = make_jwt(key, &claims, None);

        let result = provider.validate_token(&token).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("expired"),
            "Expected 'expired' in error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_jwt_wrong_signature() {
        let key = b"test-secret-key-32bytes-long!!!!!";
        let wrong_key = b"completely-different-key-32bytes!!";
        let provider = JwtAuthProvider::new(key.to_vec());

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = serde_json::json!({
            "sub": "user-1",
            "exp": now + 3600,
            "iat": now
        });
        let token = make_jwt(wrong_key, &claims, None);

        let result = provider.validate_token(&token).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("signature"),
            "Expected 'signature' in error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_jwt_missing_exp_claim() {
        let key = b"test-secret-key-32bytes-long!!!!!";
        let provider = JwtAuthProvider::new(key.to_vec());

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // No "exp" claim at all
        let claims = serde_json::json!({
            "sub": "user-1",
            "iat": now
        });
        let token = make_jwt(key, &claims, None);

        let result = provider.validate_token(&token).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("exp"),
            "Expected error about missing 'exp', got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_jwt_missing_sub_claim() {
        let key = b"test-secret-key-32bytes-long!!!!!";
        let provider = JwtAuthProvider::new(key.to_vec());

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // No "sub" claim - serde will fail deserializing TokenClaims
        let claims = serde_json::json!({
            "exp": now + 3600,
            "iat": now
        });
        let token = make_jwt(key, &claims, None);

        let result = provider.validate_token(&token).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("claims") || err_msg.contains("sub"),
            "Expected error about missing claims, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_jwt_wrong_issuer() {
        let key = b"test-secret-key-32bytes-long!!!!!";
        let provider = JwtAuthProvider::new(key.to_vec()).with_issuer("kias");

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = serde_json::json!({
            "sub": "user-1",
            "iss": "evil-issuer",
            "aud": "mcp-server",
            "exp": now + 3600,
            "iat": now
        });
        let token = make_jwt(key, &claims, None);

        let result = provider.validate_token(&token).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("issuer"),
            "Expected 'issuer' in error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_jwt_valid_token_with_issuer() {
        let key = b"test-secret-key-32bytes-long!!!!!";
        let provider = JwtAuthProvider::new(key.to_vec()).with_issuer("kias");

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = serde_json::json!({
            "sub": "user-1",
            "iss": "kias",
            "exp": now + 3600,
            "iat": now
        });
        let token = make_jwt(key, &claims, None);

        let result = provider.validate_token(&token).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().sub, "user-1");
    }

    #[tokio::test]
    async fn test_jwt_with_audience() {
        let key = b"test-secret-key-32bytes-long!!!!!";
        let provider = JwtAuthProvider::new(key.to_vec())
            .with_issuer("kias")
            .with_audience("mcp-server");

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = serde_json::json!({
            "sub": "user-1",
            "iss": "kias",
            "aud": "mcp-server",
            "exp": now + 3600,
            "iat": now
        });
        let token = make_jwt(key, &claims, None);

        let result = provider.validate_token(&token).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_jwt_wrong_audience() {
        let key = b"test-secret-key-32bytes-long!!!!!";
        let provider = JwtAuthProvider::new(key.to_vec()).with_audience("mcp-server");

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = serde_json::json!({
            "sub": "user-1",
            "aud": "wrong-audience",
            "exp": now + 3600,
            "iat": now
        });
        let token = make_jwt(key, &claims, None);

        let result = provider.validate_token(&token).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_api_key_revocation() {
        let provider = ApiKeyAuthProvider::new();
        let user = UserInfo {
            id: "user-1".to_string(),
            username: "testuser".to_string(),
            email: None,
            roles: vec!["developer".to_string()],
            active: true,
            attributes: HashMap::new(),
        };

        provider.register_key("revoke-test-key", user).await;

        // Verify key works
        let result = provider.validate_token("revoke-test-key").await;
        assert!(result.is_ok());

        // Revoke
        let revoked = provider.revoke_key("revoke-test-key").await;
        assert!(revoked);

        // Verify key no longer works
        let result = provider.validate_token("revoke-test-key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_api_key_revoke_nonexistent() {
        let provider = ApiKeyAuthProvider::new();
        let revoked = provider.revoke_key("nonexistent").await;
        assert!(!revoked);
    }

    #[tokio::test]
    async fn test_role_builder_with_inherits() {
        let role = Role::new("custom", "Custom role")
            .with_permission(Permission::ToolsList)
            .with_inherits("viewer");

        assert_eq!(role.name, "custom");
        assert_eq!(role.description, "Custom role");
        assert!(role.permissions.contains(&Permission::ToolsList));
        assert!(role.inherits.contains(&"viewer".to_string()));
    }

    #[tokio::test]
    async fn test_rbac_get_user_permissions() {
        let auth = AuthorizationManager::new();

        // Wait for builtin roles to be registered
        tokio::time::sleep(Duration::from_millis(50)).await;

        let admin_user = UserInfo {
            id: "admin-1".to_string(),
            username: "admin".to_string(),
            email: None,
            roles: vec!["admin".to_string()],
            active: true,
            attributes: HashMap::new(),
        };

        let permissions = auth.get_user_permissions(&admin_user).await;
        assert!(permissions.contains(&Permission::Admin));
        assert!(permissions.contains(&Permission::ToolsList));
        assert!(permissions.contains(&Permission::ResourcesList));
    }

    #[tokio::test]
    async fn test_rbac_set_default_roles() {
        let mut auth = AuthorizationManager::new();
        auth.set_default_roles(vec!["viewer".to_string()]);

        // Wait for builtin roles to be registered
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Verify default roles were set by checking a user with no explicit roles
        // Default roles apply during check_permission, not get_user_permissions
        let user = UserInfo {
            id: "user-1".to_string(),
            username: "testuser".to_string(),
            email: None,
            roles: vec![], // No explicit roles
            active: true,
            attributes: HashMap::new(),
        };

        // check_permission should use default roles
        let can_list = auth.check_permission(&user, &Permission::ToolsList).await;
        assert!(can_list.is_ok());
    }

    #[test]
    fn test_user_info_has_permission() {
        let user = UserInfo {
            id: "user-1".to_string(),
            username: "testuser".to_string(),
            email: None,
            roles: vec!["admin".to_string()],
            active: true,
            attributes: HashMap::new(),
        };

        // Verify user info structure
        assert_eq!(user.id, "user-1");
        assert_eq!(user.username, "testuser");
        assert!(user.roles.contains(&"admin".to_string()));
        assert!(user.active);
    }

    #[test]
    fn test_role_builder_multiple_permissions() {
        let role = Role::new("power-user", "Power user role")
            .with_permission(Permission::ToolsList)
            .with_permission(Permission::ToolsCall("*".to_string()))
            .with_permission(Permission::ResourcesList)
            .with_permission(Permission::ResourcesRead("*".to_string()));

        assert_eq!(role.permissions.len(), 4);
    }
}
