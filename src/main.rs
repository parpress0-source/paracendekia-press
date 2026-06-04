use actix_cors::Cors;
use actix_identity::IdentityMiddleware;
use actix_session::{SessionMiddleware, config::PersistentSession, storage::RedisSessionStore};
use actix_web::{
    App, HttpServer,
    cookie::{Key, time::Duration},
    middleware::from_fn,
    web::{self, Data},
};

use crate::handlers::books::{
    create_book, delete_book, delete_cloudinary_image, get_books, get_single_book, update_book,
};

mod db;
mod handlers;
mod models;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    print!("Env load ok!");

    let pool = match db::db().await {
        Ok(pool) => {
            println!("Database connected!");
            pool
        }
        Err(err) => {
            eprintln!("Database error : {}", err);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed Connect Database!",
            ));
        }
    };

    let secret_key = std::env::var("SECRET_KEY").expect("SECRET_KEY not found");

    let key = Key::from(secret_key.as_bytes());

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL not found");

    let session_store = RedisSessionStore::new(redis_url).await.unwrap();

    println!("Start Server");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec!["Content-Type", "Authorization"])
            .supports_credentials();
        App::new()
            .wrap(cors)
            .app_data(Data::new(pool.clone()))
            .wrap(IdentityMiddleware::builder().build())
            .wrap(
                SessionMiddleware::builder(session_store.clone(), key.clone())
                    .cookie_secure(false)
                    .cookie_same_site(actix_web::cookie::SameSite::Lax)
                    .session_lifecycle(PersistentSession::default().session_ttl(Duration::days(7)))
                    .build(),
            )
            .service(handlers::user::login_username)
            .service(handlers::user::login_email)
            .service(handlers::user::register)
            .service(handlers::user::logout)
            .service(
                web::scope("api")
                    .wrap(from_fn(handlers::auth::auth_user))
                    .service(handlers::user::me)
                    .service(handlers::books::cloudinary_sign)
                    .service(create_book)
                    .service(get_books)
                    .service(update_book)
                    .service(delete_cloudinary_image)
                    .service(delete_book)
                    .service(get_single_book),
            )
    })
    .bind(("0.0.0.0", 7860))?
    .run()
    .await
}
