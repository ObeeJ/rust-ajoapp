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

    let state = AppState { store: Store::new() };

    println!("🚀 Sanasana API running on http://0.0.0.0:3000");
    println!("📖 Docs: http://0.0.0.0:3000/_docs");

    App::new()
        .state(state)
        // Auth
        .route("POST", "/auth/register",        routes::register)
        .route("POST", "/auth/login",            routes::login)
        // Wallet
        .route("GET",  "/wallet",                routes::get_wallet)
        .route("GET",  "/wallet/transactions",   routes::get_transactions)
        .route("POST", "/wallet/fund",           routes::fund_wallet)
        .route("POST", "/webhook/paystack",      routes::paystack_webhook)
        // Ajo
        .route("GET",  "/ajo",                   routes::list_ajo)
        .route("POST", "/ajo",                   routes::create_ajo)
        .route("POST", "/ajo/:id/join",          routes::join_ajo)
        .route("POST", "/ajo/:id/contribute",    routes::contribute_ajo)
        // Bills
        .route("GET",  "/bills",                 routes::list_bills)
        .route("POST", "/bills",                 routes::create_bill)
        .route("POST", "/bills/:id/pay",         routes::pay_bill)
        .listen("0.0.0.0:3000")
        .await;
}
