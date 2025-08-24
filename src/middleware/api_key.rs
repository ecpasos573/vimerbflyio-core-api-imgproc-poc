use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse, body::BoxBody,
};
use futures_util::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm, TokenData};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};


pub struct ApiKey {
    pub vmbfcoreapi_imgproc_mkey: String,
    pub vmbfcoreapi_imgproc_uid: String,
}

impl<S> Transform<S, ServiceRequest> for ApiKey
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = ApiKeyMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ApiKeyMiddleware {
            service: Rc::new(service),
            vmbfcoreapi_imgproc_mkey: self.vmbfcoreapi_imgproc_mkey.clone(),
            vmbfcoreapi_imgproc_uid: self.vmbfcoreapi_imgproc_uid.clone(),
        })
    }
}

pub struct ApiKeyMiddleware<S> {
    service: Rc<S>,
    vmbfcoreapi_imgproc_mkey: String,
    vmbfcoreapi_imgproc_uid: String,
}


#[derive(Debug, Deserialize)]
struct Claims {
    userId: String,     // subject (user id or email)
    iat: usize,         // issued at (timestamp)
    exp: usize,         // expiration timestamp (required by Validation)
    
}

fn is_auth_valid(token: &str, secret: &str, userid: &str) -> bool {
    let validation = Validation::new(Algorithm::HS256);

    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    ) {
        Ok(token_data) => {
            let claims = token_data.claims;

            // Current UNIX timestamp
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as usize;

            // Check both userId and expiry
            if claims.userId == userid && claims.exp > now {
                println!("Valid token for testuserid, exp: {}", claims.exp);
                true
            } else {
                println!("Invalid claims: {:?}", claims);
                false
            }
        }
        Err(err) => {
            println!("Authentication error: {:?}", err);
            false
        }
    }
}




impl<S> Service<ServiceRequest> for ApiKeyMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        
        let srv = Rc::clone(&self.service);

        let authorized = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|header| header.strip_prefix("Bearer "))
            .map(|token| is_auth_valid(token, &self.vmbfcoreapi_imgproc_mkey, &self.vmbfcoreapi_imgproc_uid))
            .unwrap_or(false);

        Box::pin(async move {
            if authorized {
                srv.call(req).await
            } else {
                let res = req.into_response(
                    HttpResponse::Unauthorized().finish()
                );
                Ok(res)
            }
        })
    }
}

