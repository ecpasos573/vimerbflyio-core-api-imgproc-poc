use actix_web::web;

use crate::handlers::{
    health::health_check, 
    image::get_image_info,
    image::get_image_metadata_info,
    image::get_images_from_source,
    image::get_images_from_requests,
    image::resize_image,
};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(health_check)
        .service(get_image_info)
        .service(get_image_metadata_info)
        .service(get_images_from_source)
        .service(get_images_from_requests)
        .service(resize_image);
}