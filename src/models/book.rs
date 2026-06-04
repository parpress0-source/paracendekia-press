use actix_web::HttpResponse;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use uuid::Uuid;

pub const DEFAULT_LIMIT: i64 = 10;
pub const MAX_LIMIT: i64 = 100;

#[derive(Deserialize)]
pub struct DeleteImageRequest {
    pub public_id: String,
}

#[derive(Deserialize, Serialize, FromRow)]
pub struct BookSql {
    pub id: Uuid,
    pub title: String,
    pub author: String,
    pub synopsis: String,
    pub cover_url: Option<String>,
    pub cover_public_id: Option<String>,
    pub isbn: Option<String>,
    pub published_at: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, FromRow)]
pub struct BookRequest {
    pub title: String,
    pub author: String,
    pub synopsis: String,
    pub cover_url: Option<String>,
    pub cover_public_id: Option<String>,
    pub isbn: Option<String>,
    pub published_at: Option<NaiveDate>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BookResponse {
    pub id: Uuid,
    pub title: String,
    pub author: String,
    pub synopsis: String,
    pub cover_url: Option<String>,
    pub cover_public_id: Option<String>,
    pub isbn: Option<String>,
    pub published_at: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct BooksQuery {
    pub search: Option<String>,
    /// Opaque base64 cursor dari response sebelumnya
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug)]
pub struct DecodedCursor {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_next_page: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Invalid cursor")]
    InvalidCursor,
}

impl actix_web::ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::Database(e) => {
                tracing::error!("DB error: {e}");
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Internal server error"
                }))
            }
            ApiError::InvalidCursor => HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid cursor"
            })),
        }
    }
}

#[derive(Deserialize, FromRow)]
pub struct BookUpdate {
    pub id: Uuid,
    pub title: Option<String>,
    pub author: Option<String>,
    pub synopsis: Option<String>,
    pub cover_url: Option<String>,
    pub cover_public_id: Option<String>,
    pub isbn: Option<String>,
    pub published_at: Option<NaiveDate>,
}
