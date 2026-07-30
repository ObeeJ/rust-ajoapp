mod store;
mod services;
mod routes;
mod middleware;

use glideapi::App;
use store::Store;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Fail fast — refuse to start without required secrets
    let required = ["JWT_SECRET", "PAYSTACK_SECRET_KEY"];
    for var in &required {
        if std::env::var(var).is_err() {
            eprintln!("FATAL: {} environment variable is not set", var);
            std::process::exit(1);
        }
    }

    let state = AppState { store: Store::new() };

    println!("🚀 Cowri API running on http://0.0.0.0:3000");

    App::new()
        .state(state)
        // Auth
        .route("POST", "/v1/auth/register",          routes::register)
        .route("POST", "/v1/auth/login",              routes::login)
        .route("POST", "/v1/auth/refresh",            routes::refresh_token)
        .route("POST", "/v1/auth/logout",             routes::logout)
        // Wallet
        .route("GET",  "/v1/wallet",                  routes::get_wallet)
        .route("GET",  "/v1/wallet/transactions",     routes::get_transactions)
        .route("POST", "/v1/wallet/fund",             routes::fund_wallet)
        .route("POST", "/v1/webhook/paystack",        routes::paystack_webhook)
        // Ajo
        .route("GET",  "/v1/ajo",                     routes::list_ajo)
        .route("POST", "/v1/ajo",                     routes::create_ajo)
        .route("POST", "/v1/ajo/:id/join",            routes::join_ajo)
        .route("POST", "/v1/ajo/:id/contribute",      routes::contribute_ajo)
        // Bills
        .route("GET",  "/v1/bills",                   routes::list_bills)
        .route("POST", "/v1/bills",                   routes::create_bill)
        .route("POST", "/v1/bills/:id/pay",           routes::pay_bill)
        .listen("0.0.0.0:3000")
        .await;
}
