use sqlx::PgPool;

pub async fn init_pool(database_url: &str) -> PgPool {
    PgPool::connect(database_url)
        .await
        .expect("Failed to create database pool")
}