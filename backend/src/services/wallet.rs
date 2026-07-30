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
        .rev()
        .skip(page * per_page)
        .take(per_page)
        .cloned()
        .collect()
}

/// Double-entry debit.
/// Atomically under per-user lock:
///   1. Check available balance (two-phase: available_kobo, not ledger_kobo)
///   2. Decrement available_kobo AND ledger_kobo
///   3. Append immutable LedgerEntry (Debit leg)
///   4. Append Transaction summary
///   5. Stage OutboxEvent (delivered async — never in-flight here)
pub fn debit_wallet(
    store: &Store,
    user_id: Uuid,
    amount_kobo: i64,
    reference: &str,
    description: &str,
) -> Result<(), ApiError> {
    // Integer-only guard — reject any non-positive amount
    if amount_kobo <= 0 {
        return Err(ApiError { error: "Amount must be a positive integer in kobo".into() });
    }

    let lock   = store.wallet_lock(user_id);
    let _guard = lock.lock().map_err(|_| ApiError { error: "Wallet lock error".into() })?;

    // ── Atomic block: wallet + ledger + transaction + outbox ──────────────────
    let mut wallets = store.wallets.lock().unwrap();
    let wallet = wallets.get_mut(&user_id)
        .ok_or(ApiError { error: "Wallet not found".into() })?;

    // Two-phase check: use available_kobo (not ledger_kobo which includes holds)
    if wallet.available_kobo < amount_kobo {
        return Err(ApiError { error: "Insufficient balance".into() });
    }

    wallet.available_kobo -= amount_kobo;
    wallet.ledger_kobo    -= amount_kobo;
    wallet.version        += 1;
    let wallet_id          = wallet.id;
    let running_balance    = wallet.available_kobo;
    drop(wallets);

    let entry_id = Uuid::new_v4();
    let now      = Utc::now();

    store.ledger.lock().unwrap().push(LedgerEntry {
        id:                    entry_id,
        wallet_id,
        counterpart_wallet_id: None,
        kind:                  EntryKind::Debit,
        amount_kobo,
        running_balance_kobo:  running_balance,
        reference:             reference.to_string(),
        description:           description.to_string(),
        status:                EntryStatus::Settled,
        created_at:            now,
    });

    store.transactions.lock().unwrap().push(Transaction {
        id: entry_id, wallet_id,
        kind: TransactionKind::Debit, amount_kobo,
        reference: reference.to_string(), description: description.to_string(),
        status: TransactionStatus::Success, created_at: now,
    });

    // Stage outbox event — worker delivers this asynchronously
    stage_outbox(store, "wallet.debited", serde_json::json!({
        "wallet_id": wallet_id, "amount_kobo": amount_kobo,
        "reference": reference, "running_balance_kobo": running_balance
    }));

    Ok(())
}

/// Double-entry credit.
/// Atomically under per-user lock:
///   1. Increment available_kobo AND ledger_kobo
///   2. Append immutable LedgerEntry (Credit leg)
///   3. Append Transaction summary
///   4. Stage OutboxEvent
pub fn credit_wallet(
    store: &Store,
    user_id: Uuid,
    amount_kobo: i64,
    reference: &str,
    description: &str,
) {
    if amount_kobo <= 0 { return; }

    let lock   = store.wallet_lock(user_id);
    let Ok(_guard) = lock.lock() else { return };

    let mut wallets = store.wallets.lock().unwrap();
    let Some(wallet) = wallets.get_mut(&user_id) else { return };

    wallet.available_kobo += amount_kobo;
    wallet.ledger_kobo    += amount_kobo;
    wallet.version        += 1;
    let wallet_id          = wallet.id;
    let running_balance    = wallet.available_kobo;
    drop(wallets);

    let entry_id = Uuid::new_v4();
    let now      = Utc::now();

    store.ledger.lock().unwrap().push(LedgerEntry {
        id:                    entry_id,
        wallet_id,
        counterpart_wallet_id: None,
        kind:                  EntryKind::Credit,
        amount_kobo,
        running_balance_kobo:  running_balance,
        reference:             reference.to_string(),
        description:           description.to_string(),
        status:                EntryStatus::Settled,
        created_at:            now,
    });

    store.transactions.lock().unwrap().push(Transaction {
        id: entry_id, wallet_id,
        kind: TransactionKind::Credit, amount_kobo,
        reference: reference.to_string(), description: description.to_string(),
        status: TransactionStatus::Success, created_at: now,
    });

    stage_outbox(store, "wallet.credited", serde_json::json!({
        "wallet_id": wallet_id, "amount_kobo": amount_kobo,
        "reference": reference, "running_balance_kobo": running_balance
    }));
}

/// Ledger invariance check — sum(credits) - sum(debits) must equal current balance.
/// Call this in tests and reconciliation jobs.
pub fn assert_ledger_invariant(store: &Store, wallet_id: Uuid) -> Result<(), String> {
    let ledger = store.ledger.lock().unwrap();
    let wallet = store.wallets.lock().unwrap();
    let w = wallet.values().find(|w| w.id == wallet_id)
        .ok_or("wallet not found")?;

    let credits: i64 = ledger.iter()
        .filter(|e| e.wallet_id == wallet_id && e.kind == EntryKind::Credit)
        .map(|e| e.amount_kobo).sum();
    let debits: i64 = ledger.iter()
        .filter(|e| e.wallet_id == wallet_id && e.kind == EntryKind::Debit)
        .map(|e| e.amount_kobo).sum();

    let expected = credits - debits;
    if expected != w.available_kobo {
        return Err(format!(
            "LEDGER INVARIANT VIOLATED: credits({credits}) - debits({debits}) = {expected} ≠ balance({})",
            w.available_kobo
        ));
    }
    Ok(())
}

fn stage_outbox(store: &Store, event_type: &str, payload: serde_json::Value) {
    store.outbox.lock().unwrap().push(OutboxEvent {
        id:          Uuid::new_v4(),
        event_type:  event_type.to_string(),
        payload:     payload.to_string(),
        status:      OutboxStatus::Pending,
        attempts:    0,
        created_at:  Utc::now(),
        next_retry:  Utc::now(),
    });
}

pub async fn init_paystack_payment(
    amount_kobo: i64,
    email: &str,
    reference: &str,
) -> Result<PaystackInitResponse, ApiError> {
    if amount_kobo < 10_000 {
        return Err(ApiError { error: "Minimum top-up is ₦100".into() });
    }
    if amount_kobo > 100_000_000 {
        return Err(ApiError { error: "Maximum top-up is ₦1,000,000".into() });
    }

    let secret_key = std::env::var("PAYSTACK_SECRET_KEY")
        .map_err(|_| ApiError { error: "Payment service unavailable".into() })?;

    let app_url = std::env::var("APP_URL").unwrap_or_else(|_| "https://cowri.app".into());

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.paystack.co/transaction/initialize")
        .bearer_auth(&secret_key)
        .json(&serde_json::json!({
            "email": email,
            "amount": amount_kobo,
            "reference": reference,
            "callback_url": format!("{app_url}/wallet/verify")
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
        authorization_url: body["data"]["authorization_url"].as_str().unwrap_or("").to_string(),
        reference: reference.to_string(),
    })
}
