pub mod models;
pub mod repo;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::AppConfig;
use crate::constants::DB_MAX_CONNECTIONS;

/// Cria o pool de conexões e aplica as migrations pendentes (src-tauri/migrations/).
pub async fn init_pool(config: &AppConfig) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(DB_MAX_CONNECTIONS)
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}
