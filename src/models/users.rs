use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct UsernameUserRequest {
    #[validate(length(min = 3, message = "Username minimal 3 karakter"))]
    pub username: String,

    #[validate(length(min = 6, message = "Password minimal 6 karakter"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EmailUserRequest {
    #[validate(length(min = 3, message = "Email tidak valid"))]
    pub email: String,

    #[validate(length(min = 6, message = "Password minimal 6 karakter"))]
    pub password: String,
}

#[derive(Deserialize, FromRow)]
pub struct UserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub email_verified: bool,
    pub role: Option<String>,
}

#[derive(Deserialize, FromRow)]
pub struct UserSession {
    pub id: Uuid,
    pub password: String,
    pub username: String,
    pub email: String,
    pub role: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SessionUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: Option<String>,
}

#[derive(Serialize)]
pub struct SignatureResponse {
    pub timestamp: u64,
    pub signature: String,
    pub api_key: String,
}
