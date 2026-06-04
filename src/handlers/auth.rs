use actix_session::Session;
use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
};

use crate::models::users::SessionUser;

pub async fn auth_user(
    session: Session,
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let user: Option<SessionUser> = session.get("user").unwrap();

    match user {
        Some(user) => {
            req.extensions_mut().insert(user);

            next.call(req).await
        }

        None => Ok(req.into_response(HttpResponse::Unauthorized().body("Unauthorized"))),
    }
}
