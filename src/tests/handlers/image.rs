#[cfg(test)]
mod tests {
    use actix_web::{test, App};
    use serde_json::Value;

    // Import the handler
    use crate::handlers::image::get_image_info;

    #[actix_rt::test]
    async fn test_get_image_info_success() {
        // Initialize test app with the route
        let app = test::init_service(
            App::new().service(get_image_info)
        ).await;

        // Simulate GET /image/test123
        let req = test::TestRequest::get()
            .uri("/image/test123")
            .to_request();

        // Send the request and read the response body
        let resp = test::call_and_read_body(&app, req).await;

        // Parse the response as JSON
        let body: Value = serde_json::from_slice(&resp).expect("Failed to parse JSON");

        // Assert response fields
        assert_eq!(body["url"], "https://cdn.example.com/images/test123.jpg");
        assert_eq!(body["filename"], "test123.jpg");
        assert_eq!(body["size"], 512000);
        assert_eq!(body["mime_type"], "image/jpeg");
        assert_eq!(body["width"], 1024);
        assert_eq!(body["height"], 768);
    }
}


/*
use actix_web::{test, App};
use crate::handlers::image::get_image_info; 
use serde_json::Value;

#[actix_rt::test]
async fn test_get_image_info() {
    let app = test::init_service(App::new().service(get_image_info)).await;

    let req = test::TestRequest::get().uri("/image/test123").to_request();
    let resp = test::call_and_read_body(&app, req).await;

    let body: Value = serde_json::from_slice(&resp).unwrap();

    assert_eq!(body["filename"], "test123.jpg");
    assert_eq!(body["url"], "https://cdn.example.com/images/test123.jpg");
    assert_eq!(body["size"], 512000);
    assert_eq!(body["mime_type"], "image/jpeg");
    assert_eq!(body["width"], 1024);
    assert_eq!(body["height"], 768);
}

*/

