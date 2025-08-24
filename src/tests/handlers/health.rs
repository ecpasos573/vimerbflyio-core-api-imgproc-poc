#[cfg(test)]
mod tests {
    use actix_web::{test, App};
    use crate::handlers::health::health_check;

    #[actix_rt::test]
    async fn test_health_check() {
        let app = test::init_service(App::new().service(health_check)).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}




/*
use actix_web::{test, App};
use crate::handlers::health::health_check; 

#[actix_rt::test]
async fn test_health_check() {
    let app = test::init_service(App::new().service(health_check)).await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_and_read_body(&app, req).await;

    assert_eq!(resp, "OK");
}
*/

