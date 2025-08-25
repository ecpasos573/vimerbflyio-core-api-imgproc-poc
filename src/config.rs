use dotenv::dotenv;
use std::env;

pub struct AppConfig {
    pub vmbfcoreapi_imgproc_mkey: String,
    pub vmbfcoreapi_imgproc_muid: String,
    pub database_url: String,
    pub host: String,
    pub port: String,
    pub working_dir: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenv().ok();

        Self {
            vmbfcoreapi_imgproc_mkey: env::var("VMBFCOREAPI_IMGPROC_MKEY").expect("VMBFCOREAPI_IMGPROC_MKEY must be set"),
            vmbfcoreapi_imgproc_muid: env::var("VMBFCOREAPI_IMGPROC_MUID").expect("VMBFCOREAPI_IMGPROC_MUID must be set"),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            host: env::var("HOST").expect("HOST must be set"),
            port: env::var("PORT").expect("PORT must be set"),
            working_dir: env::var("WORKING_DIR").expect("WORKING_DIR must be set"),
        }
    }
}

