use actix_web::{
    HttpResponse, Responder, delete, get, post, put,
    web::{self, Data, Json},
};
use base64::{Engine, engine::general_purpose};
use chrono::{DateTime, Utc};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

use crate::models::{
    book::{
        ApiError, BookSql, BookUpdate, BooksQuery, DEFAULT_LIMIT, DecodedCursor,
        DeleteImageRequest, MAX_LIMIT, PaginatedResponse,
    },
    users::SignatureResponse,
};
use sqlx::PgPool;

use crate::models::book::{BookRequest, BookResponse};

#[get("/cloudinary/sign")]
pub async fn cloudinary_sign() -> impl Responder {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let params = format!(
        "timestamp={}{}",
        timestamp,
        std::env::var("CLOUDINARY_API_SECRET").unwrap()
    );

    let mut hasher = Sha1::new();
    hasher.update(params.as_bytes());

    let signature = hex::encode(hasher.finalize());

    HttpResponse::Ok().json(SignatureResponse {
        timestamp,
        signature,
        api_key: std::env::var("CLOUDINARY_API_KEY").unwrap(),
    })
}

#[post("/cloudinary/delete")]
pub async fn delete_cloudinary_image(body: web::Json<DeleteImageRequest>) -> impl Responder {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 1. Ambil env variables dengan aman (disarankan pakai expect/unwrap)
    let api_secret =
        std::env::var("CLOUDINARY_API_SECRET").expect("CLOUDINARY_API_SECRET must be set");
    let api_key = std::env::var("CLOUDINARY_API_KEY").expect("CLOUDINARY_API_KEY must be set");
    let cloud_name =
        std::env::var("CLOUDINARY_CLOUD_NAME").expect("CLOUDINARY_CLOUD_NAME must be set");

    // 2. Buat string untuk signature (Formula Cloudinary: public_id & timestamp + secret)
    let string_to_sign = format!(
        "public_id={}&timestamp={}{}",
        body.public_id, timestamp, api_secret
    );

    // 3. Hashing menggunakan Sha1
    let mut hasher = Sha1::new();
    hasher.update(string_to_sign.as_bytes());
    let signature = hex::encode(hasher.finalize());

    // 4. Gunakan HashMap untuk menghindari masalah lifetime/temporary value di `.form()`
    let mut form_data = HashMap::new();
    form_data.insert("public_id", body.public_id.clone());
    form_data.insert("api_key", api_key);
    form_data.insert("timestamp", timestamp.to_string());
    form_data.insert("signature", signature);

    let client = reqwest::Client::new();
    let url = format!(
        "https://api.cloudinary.com/v1_1/{}/image/destroy",
        cloud_name
    );

    // 5. Kirim request
    let response: Result<reqwest::Response, reqwest::Error> = client
        .post(url)
        .form(&form_data) // HashMap otomatis di-serialize menjadi application/x-www-form-urlencoded
        .send()
        .await;

    match response {
        Ok(res) => {
            // 1. Ambil status code dari reqwest, lalu ubah menjadi u16 (misal: 200, 400, 404)
            let status_raw: u16 = res.status().as_u16();

            let json: serde_json::Value = res.json().await.unwrap_or_default();

            // 2. Konversi u16 menjadi StatusCode milik Actix menggunakan TryFrom
            if let Ok(actix_status) = actix_web::http::StatusCode::from_u16(status_raw) {
                HttpResponse::build(actix_status).json(json)
            } else {
                // Jika status code dari Cloudinary aneh/tidak valid di Actix
                HttpResponse::InternalServerError().json(json)
            }
        }
        Err(_) => HttpResponse::InternalServerError().json(serde_json::json!({
            "message": "Failed delete image"
        })),
    }
}

#[post("/books")]
pub async fn create_book(pool: Data<PgPool>, book_req: Json<BookRequest>) -> impl Responder {
    let result = sqlx::query_as::<_, BookResponse>(
        r#"
        INSERT INTO books (
            title,
            author,
            synopsis,
            cover_url,
            cover_public_id,
            isbn,
            published_at
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7
        )
        RETURNING
            id,
            title,
            author,
            synopsis,
            cover_url,
            cover_public_id,
            isbn,
            published_at,
            created_at,
            updated_at
        "#,
    )
    .persistent(false)
    .bind(&book_req.title)
    .bind(&book_req.author)
    .bind(&book_req.synopsis)
    .bind(&book_req.cover_url)
    .bind(&book_req.cover_public_id)
    .bind(&book_req.isbn)
    .bind(book_req.published_at)
    .fetch_one(pool.get_ref())
    .await;

    match result {
        Ok(book) => HttpResponse::Created().json(book),

        Err(err) => {
            eprintln!("Create book error: {err}");

            HttpResponse::InternalServerError().json(serde_json::json!({
                "message": "Failed to create book"
            }))
        }
    }
}

fn encode_cursor(created_at: DateTime<Utc>, id: Uuid) -> String {
    let raw = format!("{},{}", created_at.timestamp_micros(), id);
    general_purpose::URL_SAFE_NO_PAD.encode(raw)
}

fn decode_cursor(cursor: &str) -> Result<DecodedCursor, ApiError> {
    let bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| ApiError::InvalidCursor)?;

    let s = String::from_utf8(bytes).map_err(|_| ApiError::InvalidCursor)?;

    let mut parts = s.splitn(2, ',');

    let micros: i64 = parts
        .next()
        .ok_or(ApiError::InvalidCursor)?
        .parse()
        .map_err(|_| ApiError::InvalidCursor)?;

    let id: Uuid = parts
        .next()
        .ok_or(ApiError::InvalidCursor)?
        .parse()
        .map_err(|_| ApiError::InvalidCursor)?;

    let created_at = DateTime::from_timestamp_micros(micros).ok_or(ApiError::InvalidCursor)?;

    Ok(DecodedCursor { created_at, id })
}

// ─── Handler ─────────────────────────────────────────────────────────────────

#[get("/books")]
pub async fn get_books(
    pool: web::Data<PgPool>,
    query: web::Query<BooksQuery>,
) -> Result<impl Responder, ApiError> {
    // Sanitasi limit
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    // Decode cursor (jika ada); cursor invalid → 400
    let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

    // Normalkan search: string kosong diperlakukan sama seperti None
    let search = query.search.as_deref().filter(|s| !s.is_empty());

    let rows: Vec<BookSql> = match (search, &cursor) {
        // ── Case 1: ada search, ada cursor ───────────────────────────────────
        (Some(q), Some(c)) => {
            sqlx::query_as(
                r#"
                SELECT
                    id, title, author, synopsis,
                    cover_url, cover_public_id, isbn,
                    published_at, created_at
                FROM books
                WHERE search_vector @@ plainto_tsquery('simple', $1)
                  AND (created_at, id) < ($2, $3)
                ORDER BY created_at DESC, id DESC
                LIMIT $4
                "#,
            )
            .persistent(false)
            .bind(q)
            .bind(c.created_at)
            .bind(c.id)
            .bind(limit + 1)
            .fetch_all(pool.get_ref())
            .await?
        }

        // ── Case 2: ada search, tanpa cursor (halaman pertama) ───────────────
        (Some(q), None) => {
            sqlx::query_as(
                r#"
                SELECT
                    id, title, author, synopsis,
                    cover_url, cover_public_id, isbn,
                    published_at, created_at
                FROM books
                WHERE search_vector @@ plainto_tsquery('simple', $1)
                ORDER BY created_at DESC, id DESC
                LIMIT $2
                "#,
            )
            .persistent(false)
            .bind(q)
            .bind(limit + 1)
            .fetch_all(pool.get_ref())
            .await?
        }

        // ── Case 3: tanpa search, ada cursor ─────────────────────────────────
        (None, Some(c)) => {
            sqlx::query_as(
                r#"
                SELECT
                    id, title, author, synopsis,
                    cover_url, cover_public_id, isbn,
                    published_at, created_at
                FROM books
                WHERE (created_at, id) < ($1, $2)
                ORDER BY created_at DESC, id DESC
                LIMIT $3
                "#,
            )
            .persistent(false)
            .bind(c.created_at)
            .bind(c.id)
            .bind(limit + 1)
            .fetch_all(pool.get_ref())
            .await?
        }

        // ── Case 4: tanpa search, tanpa cursor (halaman pertama) ─────────────
        (None, None) => {
            sqlx::query_as(
                r#"
                SELECT
                    id, title, author, synopsis,
                    cover_url, cover_public_id, isbn,
                    published_at, created_at
                FROM books
                ORDER BY created_at DESC, id DESC
                LIMIT $1
                "#,
            )
            .persistent(false)
            .bind(limit + 1)
            .fetch_all(pool.get_ref())
            .await?
        }
    };

    // Deteksi halaman berikutnya
    let has_next_page = rows.len() > limit as usize;
    let mut books = rows;
    if has_next_page {
        books.pop(); // buang item ke-(limit+1)
    }

    // Encode cursor dari item terakhir (bukan dari item yang di-pop)
    let next_cursor = if has_next_page {
        books.last().map(|b| encode_cursor(b.created_at, b.id))
    } else {
        None
    };

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        data: books,
        next_cursor,
        has_next_page,
    }))
}

#[put("/books")]
pub async fn update_book(pool: web::Data<PgPool>, book_req: Json<BookUpdate>) -> impl Responder {
    let result = sqlx::query_as::<_, BookResponse>(
        r#"
    UPDATE books
    SET
        title = COALESCE($1, title),
        author = COALESCE($2, author),
        synopsis = COALESCE($3, synopsis),
        cover_url = COALESCE($4, cover_url),
        cover_public_id = COALESCE($5, cover_public_id),
        isbn = COALESCE($6, isbn),
        published_at = COALESCE($7, published_at)
    WHERE id = $8
    RETURNING
        id,
        title,
        author,
        synopsis,
        cover_url,
        cover_public_id,
        isbn,
        published_at,
        created_at,
        updated_at
    "#,
    )
    .persistent(false)
    .bind(&book_req.title)
    .bind(&book_req.author)
    .bind(&book_req.synopsis)
    .bind(&book_req.cover_url)
    .bind(&book_req.cover_public_id)
    .bind(&book_req.isbn)
    .bind(book_req.published_at)
    .bind(book_req.id)
    .fetch_one(pool.get_ref())
    .await;

    match result {
        Ok(book) => HttpResponse::Ok().json(book),

        Err(sqlx::Error::RowNotFound) => HttpResponse::NotFound().json(serde_json::json!({
            "message": "Book not found"
        })),

        Err(err) => {
            tracing::error!("Update book error: {}", err);

            HttpResponse::InternalServerError().json(serde_json::json!({
                "message": "Failed to update book",
                "error" : &err.to_string(),
            }))
        }
    }
}

#[delete("/books/{id}")]
pub async fn delete_book(pool: web::Data<PgPool>, path: web::Path<Uuid>) -> impl Responder {
    let id = path.into_inner();

    match sqlx::query("DELETE FROM books WHERE id = $1")
        .persistent(false)
        .bind(id)
        .execute(pool.get_ref())
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "message": "Book not found"
                }));
            }

            HttpResponse::Ok().json(serde_json::json!({
                "message": "Book deleted successfully"
            }))
        }

        Err(err) => HttpResponse::InternalServerError().json(serde_json::json!({
            "message": format!("Database error: {}", err)
        })),
    }
}

#[get("/books/{id}")]
pub async fn get_single_book(pool: web::Data<PgPool>, path: web::Path<Uuid>) -> impl Responder {
    let id = path.into_inner();

    match sqlx::query_as::<_, BookSql>("SELECT * FROM books WHERE id = $1")
        .persistent(false)
        .bind(id)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(Some(book)) => HttpResponse::Ok().json(book),

        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "message": "Book not found"
        })),

        Err(err) => {
            eprintln!("Database error: {}", err);

            HttpResponse::InternalServerError().json(serde_json::json!({
                "message": "Database error"
            }))
        }
    }
}
