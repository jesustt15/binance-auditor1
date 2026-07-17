use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde_json::json;
use std::sync::Arc;

use crate::models::Claims;

/// Hash a plaintext password with Argon2id
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("Password hashing error: {}", e))?;
    Ok(hash.to_string())
}

/// Verify a plaintext password against an Argon2 hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(hash).map_err(|e| format!("Invalid hash format: {}", e))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Generate a JWT access token (15 min expiry)
pub fn generate_access_token(
    user_id: &str,
    username: &str,
    role: &str,
    secret: &str,
) -> Result<String, String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        iat: now.timestamp() as usize,
        exp: (now.timestamp() + 900) as usize, // 15 min
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode error: {}", e))
}

/// Generate a JWT refresh token (7 day expiry)
pub fn generate_refresh_token(user_id: &str, secret: &str) -> Result<String, String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        username: String::new(),
        role: "refresh".to_string(),
        iat: now.timestamp() as usize,
        exp: (now.timestamp() + 604800) as usize, // 7 days
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode error: {}", e))
}

/// Decode and validate a JWT token
pub fn decode_token(token: &str, secret: &str) -> Result<Claims, String> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| format!("JWT decode error: {}", e))?;
    Ok(token_data.claims)
}

/// Extract Claims from Authorization header
pub fn extract_token_from_header(headers: &Parts) -> Option<String> {
    headers
        .headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Axum auth extractor
// ---------------------------------------------------------------------------

/// Authenticated user context injected by middleware
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub username: String,
    pub role: String,
}

/// Extract auth user from request — requires JWT to have been validated
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let jwt_secret = parts
            .extensions
            .get::<Arc<String>>()
            .ok_or(AuthError::MissingSecret)?;

        let token = extract_token_from_header(parts).ok_or(AuthError::MissingToken)?;

        let claims = decode_token(&token, jwt_secret).map_err(|_| AuthError::InvalidToken)?;

        Ok(AuthUser {
            user_id: claims.sub,
            username: claims.username,
            role: claims.role,
        })
    }
}

// ---------------------------------------------------------------------------
// Role guards
// ---------------------------------------------------------------------------

pub fn require_admin(auth: &AuthUser) -> Result<(), AuthError> {
    if auth.role == "admin" {
        Ok(())
    } else {
        Err(AuthError::Forbidden("Se requiere rol de admin".to_string()))
    }
}

/// Allowed roles: admin or cashier
pub fn require_auth_any(_auth: &AuthUser) -> Result<(), AuthError> {
    // Any authenticated user passes; role-specific checks in handlers
    Ok(())
}

// ---------------------------------------------------------------------------
// Auth errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum AuthError {
    MissingSecret,
    MissingToken,
    InvalidToken,
    Forbidden(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingSecret => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "JWT secret not configured".to_string(),
            ),
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "Missing Authorization header".to_string(),
            ),
            AuthError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "Invalid or expired token".to_string(),
            ),
            AuthError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
