use chrono::Utc;
use shared::*;
use uuid::Uuid;

use crate::store::Store;

pub fn get_wallet(store: &Store, user_id: Uuid) -> Option<Wallet> {
    store.wallets.lock().unwrap().get(&user_id).cloned()
}

pub fn get_transactions(store: &Store, user_id: Uuid, page: usize, per_page: usize) -> Vec<Transaction> {
    let wallet_id = store.wallets.lock().unwrap().get(&user_id).map(|w| w.id);
    let Some(wallet_id) = wallet_id else { return vec![] };

    let txns = store.transactions.lock().unwrap();
    txns.iter()
        .filter(|t| t.wallet_id == wallet_id)
        .rev() // newest first
        .skip(page * per_page)
        .take(per_page)
        .cloned()
        .collect()
}

/// Thread-safe debit — holds per-user wallet lock to prevent double-spend
pub fn debit_wallet(
    store: &Store,
    user_id: Uuid,
    amount_kobo: i64,
    reference: &str,
    description: &str,
) -> Result<(), ApiError> {
    if amount_kobo <= 0 {
        return Err(ApiError { error: "Amount must be positive".into() });
    }

    let lock    = store.wallet_lock(user_id);
    let _guard  = lock.lock().map_err(|_| ApiError { error: "Wallet lock error".into() })?;

    let mut wallets = store.wallets.lock().unwrap();
    let wallet = wallets
        .get_mut(&user_id)
        .ok_or(ApiError { error: "Wallet not found".into() })?;

    if wallet.balance_kobo < amount_kobo {
        return Err(ApiError { error: "Insufficient balance".into() });
    }

    wallet.balance_kobo -= amount_kobo;
    let wallet_id = wallet.id;
    drop(wallets);

    store.transactions.lock().unwrap().push(Transaction {
        id: Uuid::new_v4(),
        wallet_id,
        kind: TransactionKind::Debit,
        amount_kobo,
        reference: reference.to_string(),
        description: description.to_string(),
        status: TransactionStatus::Success,
        created_at: Utc::now(),
    });

    Ok(())
}

pub fn credit_wallet(
    store: &Store,
    user_id: Uuid,
    amount_kobo: i64,
    reference: &str,
    description: &str,
) {
    if amount_kobo <= 0 {
        return;
    }

    let mut wallets = store.wallets.lock().unwrap();
    let Some(wallet) = wallets.get_mut(&user_id) else { return };
    wallet.balance_kobo += amount_kobo;
    wallet.ledger_balance_kobo += amount_kobo;
    let wallet_id = wallet.id;
    drop(wallets);

    store.transactions.lock().unwrap().push(Transaction {
        id: Uuid::new_v4(),
        wallet_id,
        kind: TransactionKind::Credit,
        amount_kobo,
        reference: reference.to_string(),
        description: description.to_string(),
        status: TransactionStatus::Success,
        created_at: Utc::now(),
    });
}

pub async fn init_paystack_payment(
    amount_kobo: i64,
    email: &str,
    reference: &str,
) -> Result<PaystackInitResponse, ApiError> {
    if amount_kobo < 10000 {
        return Err(ApiError { error: "Minimum top-up is ₦100".into() });
    }
    if amount_kobo > 100_000_000 {
        return Err(ApiError { error: "Maximum top-up is ₦1,000,000".into() });
    }

    let secret_key = std::env::var("PAYSTACK_SECRET_KEY")
        .map_err(|_| ApiError { error: "Payment service unavailable".into() })?;

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.paystack.co/transaction/initialize")
        .bearer_auth(&secret_key)
        .json(&serde_json::json!({
            "email": email,
            "amount": amount_kobo,
            "reference": reference,
            "callback_url": std::env::var("APP_URL").unwrap_or_else(|_| "https://cowri.app".into()) + "/wallet/verify"
        }))
        .send()
        .await
        .map_err(|_| ApiError { error: "Payment service unavailable".into() })?;

    let body: serde_json::Value = res.json().await
        .map_err(|_| ApiError { error: "Payment service unavailable".into() })?;

    if body["status"] != true {
        return Err(ApiError {
            error: body["message"].as_str().unwrap_or("Payment init failed").to_string(),
        });
    }

    Ok(PaystackInitResponse {
        authorization_url: body["data"]["authorization_url"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        reference: reference.to_string(),
    })
}
