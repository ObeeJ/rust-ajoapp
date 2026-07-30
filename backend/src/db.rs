use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use shared::*;

pub async fn run_migrations(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

// ── Wallet ────────────────────────────────────────────────────────────────────

pub async fn debit(
    pool:        &PgPool,
    wallet_id:   Uuid,
    amount_kobo: i64,
    reference:   &str,
    description: &str,
) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let row: Option<(i64,)> = sqlx::query_as(
        "UPDATE wallets
         SET available_kobo = available_kobo - $1,
             ledger_kobo    = ledger_kobo    - $1,
             version        = version + 1
         WHERE id = $2 AND available_kobo >= $1
         RETURNING available_kobo"
    )
    .bind(amount_kobo).bind(wallet_id)
    .fetch_optional(&mut *tx).await?;

    let (new_balance,) = row.ok_or(sqlx::Error::RowNotFound)?;

    sqlx::query(
        "INSERT INTO ledger_entries
             (wallet_id, kind, amount_kobo, running_balance_kobo, reference, description, status)
         VALUES ($1, 'debit', $2, $3, $4, $5, 'settled')
         ON CONFLICT (wallet_id, reference, kind) DO NOTHING"
    )
    .bind(wallet_id).bind(amount_kobo).bind(new_balance).bind(reference).bind(description)
    .execute(&mut *tx).await?;

    sqlx::query(
        "INSERT INTO transactions (wallet_id, kind, amount_kobo, reference, description, status)
         VALUES ($1, 'debit', $2, $3, $4, 'success')
         ON CONFLICT DO NOTHING"
    )
    .bind(wallet_id).bind(amount_kobo).bind(reference).bind(description)
    .execute(&mut *tx).await?;

    let payload = serde_json::json!({
        "wallet_id": wallet_id, "amount_kobo": amount_kobo,
        "reference": reference, "running_balance_kobo": new_balance
    });
    sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('wallet.debited', $1)")
        .bind(payload)
        .execute(&mut *tx).await?;

    tx.commit().await?;
    Ok(new_balance)
}

pub async fn credit(
    pool:        &PgPool,
    wallet_id:   Uuid,
    amount_kobo: i64,
    reference:   &str,
    description: &str,
) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let (new_balance,): (i64,) = sqlx::query_as(
        "UPDATE wallets
         SET available_kobo = available_kobo + $1,
             ledger_kobo    = ledger_kobo    + $1,
             version        = version + 1
         WHERE id = $2
         RETURNING available_kobo"
    )
    .bind(amount_kobo).bind(wallet_id)
    .fetch_one(&mut *tx).await?;

    sqlx::query(
        "INSERT INTO ledger_entries
             (wallet_id, kind, amount_kobo, running_balance_kobo, reference, description, status)
         VALUES ($1, 'credit', $2, $3, $4, $5, 'settled')
         ON CONFLICT (wallet_id, reference, kind) DO NOTHING"
    )
    .bind(wallet_id).bind(amount_kobo).bind(new_balance).bind(reference).bind(description)
    .execute(&mut *tx).await?;

    sqlx::query(
        "INSERT INTO transactions (wallet_id, kind, amount_kobo, reference, description, status)
         VALUES ($1, 'credit', $2, $3, $4, 'success')
         ON CONFLICT DO NOTHING"
    )
    .bind(wallet_id).bind(amount_kobo).bind(reference).bind(description)
    .execute(&mut *tx).await?;

    let payload = serde_json::json!({
        "wallet_id": wallet_id, "amount_kobo": amount_kobo,
        "reference": reference, "running_balance_kobo": new_balance
    });
    sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('wallet.credited', $1)")
        .bind(payload)
        .execute(&mut *tx).await?;

    tx.commit().await?;
    Ok(new_balance)
}

// ── Idempotency ───────────────────────────────────────────────────────────────

pub async fn get_idempotency(pool: &PgPool, key: &str) -> Option<(u16, String)> {
    let row: Option<(i16, String)> = sqlx::query_as(
        "SELECT status, body FROM idempotency_keys
         WHERE key = $1 AND created_at > NOW() - INTERVAL '24 hours'"
    )
    .bind(key)
    .fetch_optional(pool).await.ok().flatten();

    row.map(|(s, b)| (s as u16, b))
}

pub async fn set_idempotency(pool: &PgPool, key: &str, status: u16, body: &str) {
    let _ = sqlx::query(
        "INSERT INTO idempotency_keys (key, status, body)
         VALUES ($1, $2, $3) ON CONFLICT (key) DO NOTHING"
    )
    .bind(key).bind(status as i16).bind(body)
    .execute(pool).await;
}

// ── Webhook dedup ─────────────────────────────────────────────────────────────

pub async fn webhook_already_processed(pool: &PgPool, reference: &str) -> bool {
    let row: Option<(bool,)> = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM ledger_entries WHERE reference = $1 AND kind = 'credit')"
    )
    .bind(reference)
    .fetch_optional(pool).await.ok().flatten();

    row.map(|(b,)| b).unwrap_or(false)
}

// ── Outbox worker ─────────────────────────────────────────────────────────────

pub async fn outbox_worker(pool: PgPool) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let rows: Vec<(Uuid, String, serde_json::Value, i32)> = sqlx::query_as(
            "SELECT id, event_type, payload, attempts
             FROM outbox
             WHERE status = 'pending' AND next_retry <= NOW()
             ORDER BY created_at LIMIT 50"
        )
        .fetch_all(&pool).await.unwrap_or_default();

        for (id, event_type, payload, attempts) in rows {
            let delivered = deliver_event(&event_type, &payload).await;
            let attempts  = attempts + 1;

            if delivered {
                let _ = sqlx::query(
                    "UPDATE outbox SET status = 'delivered', attempts = $1 WHERE id = $2"
                )
                .bind(attempts).bind(id)
                .execute(&pool).await;
            } else {
                let backoff_secs = (5i64 * 2i64.pow(attempts as u32)).min(300);
                let next_retry   = Utc::now() + chrono::Duration::seconds(backoff_secs);
                let status       = if attempts >= 10 { "failed" } else { "pending" };

                let _ = sqlx::query(
                    "UPDATE outbox SET status = $1, attempts = $2, next_retry = $3 WHERE id = $4"
                )
                .bind(status).bind(attempts).bind(next_retry).bind(id)
                .execute(&pool).await;
            }
        }
    }
}

async fn deliver_event(event_type: &str, payload: &serde_json::Value) -> bool {
    tracing::info!(event_type, ?payload, "outbox event delivered");
    true
}
