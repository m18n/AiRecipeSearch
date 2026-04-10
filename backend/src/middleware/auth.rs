use std::{
    future::{ready, Future, Ready},
    pin::Pin,
    rc::Rc,
};

use actix_web::{body::EitherBody, dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, http::header::{self, HeaderValue}, Error, HttpMessage, HttpResponse};
use serde_json::json;
use crate::config::Config;
use crate::services::auth::validate_access_token;



#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: i32,
}



pub struct AuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service: Rc::new(service),
        }))
    }
}


pub struct AuthMiddlewareService<S> {
    service: Rc<S>
}


impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let config = req
            .app_data::<actix_web::web::Data<Config>>()
            .expect("Config must be registered as app data")
            .get_ref()
            .clone();

        Box::pin(async move {
            let token = extract_bearer_token(req.headers().get(header::AUTHORIZATION));

            let token = match token {
                Some(t) => t,
                None => {
                    return Ok(unauthorized_response(req, "Missing Authorization header"));
                }
            };
            let claims = match validate_access_token(&token, &config) {
                Ok(c) => c,
                Err(_) => {
                    return Ok(unauthorized_response(req, "Invalid or expired access token"));
                }
            };
            req.extensions_mut().insert(AuthenticatedUser {
                user_id: claims.sub,
            });
            let res = service.call(req).await?;
            Ok(res.map_into_left_body())
        })
    }
}



/// Extracts the raw token string from `Authorization: Bearer <token>`.
fn extract_bearer_token(header_value: Option<&HeaderValue>) -> Option<String> {
    let value = header_value?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    if token.is_empty() {
        return None;
    }
    Some(token.to_owned())
}

/// Builds a `401 Unauthorized` short-circuit response.
fn unauthorized_response<B: 'static>(
    req: ServiceRequest,
    message: &'static str,
) -> ServiceResponse<EitherBody<B>> {
    let response = HttpResponse::Unauthorized()
        .json(json!({ "error": "unauthorized", "message": message }));

    let (http_req, _payload) = req.into_parts();
    ServiceResponse::new(http_req, response).map_into_right_body()
}