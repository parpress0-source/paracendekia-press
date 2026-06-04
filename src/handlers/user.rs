use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, post, web};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use sqlx::PgPool;

use crate::models::users::{
    EmailUserRequest, SessionUser, UserRequest, UserSession, UsernameUserRequest,
};

fn hash_password(password: String) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hashed_password = argon2
        .hash_password(&password.as_bytes(), &salt)
        .unwrap()
        .to_string();
    hashed_password
}

fn verify_password(
    password: String,
    hashed_password: String,
) -> Result<(), argon2::password_hash::Error> {
    let argon2 = Argon2::default();
    let hashed_password = PasswordHash::new(&hashed_password).unwrap();
    argon2.verify_password(password.as_bytes(), &hashed_password)
}

#[post("/login_username")]
pub async fn login_username(
    pool: web::Data<PgPool>,
    user_request: web::Json<UsernameUserRequest>,
    session: Session,
) -> impl Responder {
    if user_request.username.trim() == "" || user_request.password == "" {
        HttpResponse::BadRequest().body("Username or Password must be not empty!")
    } else {
        let rows = sqlx::query_as::<_, UserSession>(
            "SELECT id, password, username, email, role FROM users WHERE username = $1",
        )
        .persistent(false)
        .bind(&user_request.username)
        .fetch_all(pool.get_ref())
        .await
        .unwrap();

        if rows.len() == 0 {
            HttpResponse::NotFound().json(serde_json::json!({
                "message": "email is not Registered!"
            }))
        } else {
            let session_user = SessionUser {
                id: rows[0].id,
                username: rows[0].username.clone(),
                email: rows[0].email.clone(),
                role: rows[0].role.clone(),
            };
            match verify_password(user_request.password.clone(), rows[0].password.clone()) {
                Ok(_) => {
                    session.insert("user", &session_user).unwrap();
                    HttpResponse::Ok().body("Login Successful!")
                }
                Err(_) => HttpResponse::Unauthorized().json(serde_json::json!({
                    "message": "Password is incorrect!"
                })),
            }
        }
    }
}

#[actix_web::post("/login_email")]
pub async fn login_email(
    pool: web::Data<PgPool>,
    user_request: web::Json<EmailUserRequest>,
    session: Session,
) -> impl Responder {
    if user_request.email.trim() == "" || user_request.password == "" {
        HttpResponse::BadRequest().body("Username or Password must be not empty!")
    } else {
        let rows: Vec<UserSession> = sqlx::query_as(
            "SELECT id, password, username, email, role FROM users WHERE email = $1",
        )
        .persistent(false)
        .bind(&user_request.email)
        .fetch_all(pool.get_ref())
        .await
        .unwrap();

        if rows.len() == 0 {
            HttpResponse::NotFound().json(serde_json::json!({
                "message": "email is not Registered!"
            }))
        } else {
            let session_user = SessionUser {
                id: rows[0].id,
                username: rows[0].username.clone(),
                email: rows[0].email.clone(),
                role: rows[0].role.clone(),
            };
            match verify_password(user_request.password.clone(), rows[0].password.clone()) {
                Ok(_) => {
                    session.insert("user", &session_user).unwrap();
                    HttpResponse::Ok().body("Login Successful!")
                }
                Err(_) => HttpResponse::Unauthorized().json(serde_json::json!({
                    "message": "Password is incorrect!"
                })),
            }
        }
    }
}

#[get("/logout")]
pub async fn logout(session: Session) -> impl Responder {
    session.purge();

    HttpResponse::Ok().body("Logout success")
}

#[post("/register")]
pub async fn register(
    pool: web::Data<PgPool>,
    user_request: web::Json<UserRequest>,
) -> impl Responder {
    if user_request.username.trim() == "" || user_request.password == "" {
        HttpResponse::BadRequest().body("Username or Password must be not empty!")
    } else {
        let rows: Vec<UserSession> =
            sqlx::query_as("SELECT id, username, email, role  FROM users WHERE username = $1")
                .persistent(false)
                .bind(&user_request.username)
                .fetch_all(pool.get_ref())
                .await
                .unwrap();
        if rows.len() == 0 {
            let hashed_password = hash_password(user_request.password.clone());
            sqlx::query("INSERT INTO users (username, email, email_verified, password, role) VALUES ($1, $2, $3, $4, $5 )")
                .persistent(false)
                .bind(&user_request.username)
                .bind(&user_request.email)
                .bind(&user_request.email_verified)
                .bind(&hashed_password)
                .bind(&user_request.role)
                .execute(pool.get_ref())
                .await
                .unwrap();
            HttpResponse::Ok().body("Register Successful!")
        } else {
            HttpResponse::BadRequest().body("Username is already taken!")
        }
    }
}

#[get("/me")]
pub async fn me(session: Session) -> impl Responder {
    let user: Option<SessionUser> = session.get("user").unwrap();

    match user {
        Some(user) => HttpResponse::Ok().json(user),

        None => HttpResponse::Unauthorized().body("Not logged in"),
    }
}
