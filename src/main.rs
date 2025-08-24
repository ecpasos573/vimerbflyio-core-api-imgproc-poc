mod config;
mod db;
mod metrics;
mod handlers;
mod routes;
mod models;
mod middleware;


use actix_web::{App, HttpServer, middleware::Logger};
use std::io;
use dotenv::dotenv;
use tracing::{error};
use tracing_subscriber;

use middleware::api_key::ApiKey;

#[cfg(test)]
mod tests;

#[actix_web::main]
async fn main() -> io::Result<()> {
    // Initialize environment and logging first so errors get logged
    dotenv().ok();
    tracing_subscriber::fmt::init();

    if let Err(e) = (|| async {
        let config = config::AppConfig::from_env();
        // let pool = db::init_pool(&config.database_url).await;

        let bind_address = format!("{}:{}", config.host, config.port);

        HttpServer::new(move || {
            App::new()
                .wrap(ApiKey {
                    vmbfcoreapi_imgproc_mkey: config.vmbfcoreapi_imgproc_mkey.clone(),
                    vmbfcoreapi_imgproc_uid: config.vmbfcoreapi_imgproc_uid.clone(),
                })
                .wrap(Logger::default())
                // .app_data(web::Data::new(pool.clone()))
                .configure(routes::config)
        })
        .bind(bind_address)?
        .run()
        .await
    })().await {
        error!("Application error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}