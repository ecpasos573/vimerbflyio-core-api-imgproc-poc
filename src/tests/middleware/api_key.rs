use actix_web::{test, web, App, HttpResponse};
use crate::middleware::api_key::ApiKey;

#[actix_rt::test]
async fn test_api_key_middleware_success() {
    let app = test::init_service(
        App::new()
            .wrap(ApiKey {
                vmbfcoreapi_imgproc_mkey: "test-key".into(),
                vmbfcoreapi_imgproc_uid: "test-uid".into(),
            })
            .route("/protected", web::get().to(|| async { HttpResponse::Ok().body("OK") }))
    ).await;

    let req = test::TestRequest::get()
        .uri("/protected")
        .insert_header(("X-API-KEY", "test-key"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    assert_eq!(resp, "OK");
}

#[actix_rt::test]
async fn test_api_key_middleware_failure() {
    let app = test::init_service(
        App::new()
            .wrap(ApiKey {
                vmbfcoreapi_imgproc_mkey: "test-key".into(),
                vmbfcoreapi_imgproc_uid: "test-uid".into(),
            })
            .route("/protected", web::get().to(|| async { HttpResponse::Ok().body("OK") }))
    ).await;

    // request without header
    let req = test::TestRequest::get()
        .uri("/protected")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
}
