use chrono::Utc;
use shared::*;
use uuid::Uuid;

use crate::store::Store;

pub fn get_wallet(store: &Store, user_id: Uuid) -> Option<Wallet> {
    store.wallets.lock().unwrap().get(&user_id).cloned()
}

pub fn get_transactions(store: &Store, user_id: Uuid) -> Vec<Transaction> {
    let wallet_id = store.wallets.lock().unwrap().get(&user_id).map(|w| w.id);
    let Some(wallet_id) = wallet_id else { return vec![] };
    store.transactions.lock().unwrap()
        .iter()
        .filter(|t| t.wallet_id == wallet_id)
        .cloned()
        .collect()
}

/// Called after Paystack webhook confirms payment
pub fn credit_wallet(store: &Store, user_id: Uuid, amount_kobo: i64, reference: &str, description: &str) {
    let mut wallets = store.wallets.lock().unwrap();
    let wallet = wallets.get_mut(&user_id).unwrap();
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

pub fn debit_wallet(store: &Store, user_id: Uuid, amount_kobo: i64, reference: &str, description: &str) -> Result<(), ApiError> {
    let mut wallets = store.wallets.lock().unwrap();
    let wallet = wallets.get_mut(&user_id)
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

/// Paystack: initialize a funding transaction
pub async fn init_paystack_payment(amount_kobo: i64, email: &str, reference: &str) -> Result<PaystackInitResponse, ApiError> {
    let secret_key = std::env::var("PAYSTACK_SECRET_KEY")
        .unwrap_or_else(|_| "sk_test_placeholder".into());

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.paystack.co/transaction/initialize")
        .bearer_auth(&secret_key)
        .json(&serde_json::json!({
            "email": email,
            "amount": amount_kobo,
            "reference": reference,
            "callback_url": "https://cowri.app/wallet/verify"
        }))
        .send()
        .await
        .map_err(|e| ApiError { error: e.to_string() })?;

    let body: serde_json::Value = res.json().await
        .map_err(|e| ApiError { error: e.to_string() })?;

    Ok(PaystackInitResponse {
        authorization_url: body["data"]["authorization_url"].as_str().unwrap_or("").to_string(),
        reference: reference.to_string(),
    })
}
