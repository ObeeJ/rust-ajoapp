mod store;
mod services;
mod routes;
mod middleware;
mod db;
#[cfg(test)]
mod tests;

use glideapi::{App, Config};
use sqlx::postgres::PgPoolOptions;
use store::Store;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub db:    sqlx::PgPool,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cowri=info,sqlx=warn".into()),
        )
        .init();

    // Fail fast on missing secrets
    for var in &["JWT_SECRET", "PAYSTACK_SECRET_KEY", "DATABASE_URL"] {
        if std::env::var(var).is_err() {
            eprintln!("FATAL: {var} environment variable is not set");
            std::process::exit(1);
        }
    }

    // ── Database ──────────────────────────────────────────────────────────────
    let database_url = std::env::var("DATABASE_URL").unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    db::run_migrations(&pool).await.expect("Migration failed");
    tracing::info!("Migrations applied");

    // ── Outbox worker (background) ────────────────────────────────────────────
    tokio::spawn(db::outbox_worker(pool.clone()));
    tracing::info!("Outbox worker started");

    let cors_origin = std::env::var("CORS_ORIGIN").ok();
    let state = AppState { store: Store::new(), db: pool };

    tracing::info!("🚀 Cowri API on http://0.0.0.0:3000");

    App::new()
        .config(Config { cors_origin, ..Config::default() })
        .state(state)
        .route("POST", "/v1/auth/register",       routes::register)
        .route("POST", "/v1/auth/login",           routes::login)
        .route("POST", "/v1/auth/refresh",         routes::refresh_token)
        .route("POST", "/v1/auth/logout",          routes::logout)
        .route("GET",  "/v1/wallet",               routes::get_wallet)
        .route("GET",  "/v1/wallet/transactions",  routes::get_transactions)
        .route("POST", "/v1/wallet/fund",          routes::fund_wallet)
        .route("POST", "/v1/webhook/paystack",     routes::paystack_webhook)
        .route("GET",  "/v1/ajo",                  routes::list_ajo)
        .route("POST", "/v1/ajo",                  routes::create_ajo)
        .route("POST", "/v1/ajo/:id/join",         routes::join_ajo)
        .route("POST", "/v1/ajo/:id/contribute",   routes::contribute_ajo)
        .route("GET",  "/v1/bills",                routes::list_bills)
        .route("POST", "/v1/bills",                routes::create_bill)
        .route("POST", "/v1/bills/:id/pay",        routes::pay_bill)
        .listen("0.0.0.0:3000")
        .await;
}
