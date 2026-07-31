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
    match event_type {
        "wallet.credited" => {
            let email   = payload["email"].as_str().unwrap_or("");
            let amount  = payload["amount_kobo"].as_i64().unwrap_or(0);
            let balance = payload["running_balance_kobo"].as_i64().unwrap_or(0);
            if !email.is_empty() {
                let subject = "Your Cowri wallet has been credited";
                let body    = format!(
                    "Your wallet was credited ₦{:.2}.\nNew balance: ₦{:.2}.\n\nCowri",
                    amount as f64 / 100.0, balance as f64 / 100.0
                );
                send_email(email, subject, &body).await;
            }
            true
        }
        "ajo.payout" => {
            let email  = payload["email"].as_str().unwrap_or("");
            let amount = payload["amount_kobo"].as_i64().unwrap_or(0);
            let group  = payload["group_name"].as_str().unwrap_or("your Ajo group");
            if !email.is_empty() {
                let subject = format!("You received your Ajo payout from {group}");
                let body    = format!(
                    "₦{:.2} has been paid into your Cowri wallet from {}.\n\nCowri",
                    amount as f64 / 100.0, group
                );
                send_email(email, &subject, &body).await;
            }
            true
        }
        _ => {
            tracing::info!(event_type, "outbox event processed");
            true
        }
    }
}

async fn send_email(to: &str, subject: &str, body: &str) {
    use lettre::{
        AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
        message::header::ContentType,
        transport::smtp::authentication::Credentials,
    };

    let smtp_host     = std::env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".into());
    let smtp_port: u16 = std::env::var("SMTP_PORT").ok()
        .and_then(|p| p.parse().ok()).unwrap_or(465);
    let username = match std::env::var("SMTP_USERNAME") {
        Ok(u) => u,
        Err(_) => { tracing::debug!("SMTP_USERNAME not set — email skipped"); return; }
    };
    let password = match std::env::var("SMTP_PASSWORD") {
        Ok(p) => p,
        Err(_) => { tracing::debug!("SMTP_PASSWORD not set — email skipped"); return; }
    };
    let from = std::env::var("EMAIL_FROM")
        .unwrap_or_else(|_| format!("Cowri <{username}>"));

    let msg = match Message::builder()
        .from(from.parse().unwrap())
        .to(to.parse().unwrap())
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
    {
        Ok(m) => m,
        Err(e) => { tracing::warn!(error = %e, "Failed to build email"); return; }
    };

    let creds = Credentials::new(username, password);
    let transport = if smtp_port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host)
    };

    let transport = match transport {
        Ok(t) => t.credentials(creds).build(),
        Err(e) => { tracing::warn!(error = %e, "SMTP transport error"); return; }
    };

    match transport.send(msg).await {
        Ok(_)  => tracing::info!(to, subject, "Email sent"),
        Err(e) => tracing::warn!(to, error = %e, "Email failed"),
    }
}

// ── DB persistence helpers ────────────────────────────────────────────────────

pub async fn persist_user(pool: &sqlx::PgPool, user: &shared::User, pin_hash: &str, wallet: &shared::Wallet) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    let role = match user.role { shared::UserRole::Admin => "admin", _ => "user" };

    sqlx::query(
        "INSERT INTO users (id, name, phone, email, role, email_verified, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (id) DO NOTHING"
    )
    .bind(user.id).bind(&user.name).bind(&user.phone)
    .bind(&user.email).bind(role).bind(user.email_verified).bind(user.created_at)
    .execute(&mut *tx).await?;

    sqlx::query(
        "INSERT INTO pins (user_id, hash) VALUES ($1, $2) ON CONFLICT (user_id) DO NOTHING"
    )
    .bind(user.id).bind(pin_hash)
    .execute(&mut *tx).await?;

    sqlx::query(
        "INSERT INTO wallets (id, user_id, available_kobo, ledger_kobo, version)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id) DO NOTHING"
    )
    .bind(wallet.id).bind(wallet.user_id)
    .bind(wallet.available_kobo).bind(wallet.ledger_kobo).bind(wallet.version as i64)
    .execute(&mut *tx).await?;

    tx.commit().await
}

pub async fn persist_ajo_group(pool: &sqlx::PgPool, g: &shared::AjoGroup, admin_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    let freq = match g.frequency { shared::AjoFrequency::Daily => "daily", shared::AjoFrequency::Weekly => "weekly", _ => "monthly" };
    sqlx::query(
        "INSERT INTO ajo_groups (id, name, admin_id, contribution_kobo, frequency, member_count, current_cycle, status, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,'active',$8) ON CONFLICT (id) DO NOTHING"
    )
    .bind(g.id).bind(&g.name).bind(admin_id).bind(g.contribution_kobo)
    .bind(freq).bind(g.member_count as i32).bind(g.current_cycle as i32).bind(g.created_at)
    .execute(pool).await?;

    sqlx::query(
        "INSERT INTO ajo_members (group_id, user_id, payout_position, has_received)
         VALUES ($1,$2,0,false) ON CONFLICT DO NOTHING"
    )
    .bind(g.id).bind(admin_id)
    .execute(pool).await?;

    Ok(())
}

pub async fn persist_ajo_join(pool: &sqlx::PgPool, group_id: uuid::Uuid, user_id: uuid::Uuid, position: i32) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ajo_members (group_id, user_id, payout_position, has_received)
         VALUES ($1,$2,$3,false) ON CONFLICT DO NOTHING"
    )
    .bind(group_id).bind(user_id).bind(position)
    .execute(pool).await?;
    Ok(())
}

pub async fn persist_ajo_contribution(pool: &sqlx::PgPool, group_id: uuid::Uuid, user_id: uuid::Uuid, cycle: u32, next_cycle: u32, completed: bool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO ajo_contributions (group_id, user_id, cycle) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING"
    )
    .bind(group_id).bind(user_id).bind(cycle as i32)
    .execute(&mut *tx).await?;

    let status = if completed { "completed" } else { "active" };
    sqlx::query(
        "UPDATE ajo_groups SET current_cycle = $1, status = $2 WHERE id = $3"
    )
    .bind(next_cycle as i32).bind(status).bind(group_id)
    .execute(&mut *tx).await?;

    tx.commit().await
}

pub async fn persist_bill(pool: &sqlx::PgPool, bill: &shared::Bill, participants: &[(uuid::Uuid, i64)]) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO bills (id, title, creator_id, total_kobo, status, created_at)
         VALUES ($1,$2,$3,$4,'pending',$5) ON CONFLICT (id) DO NOTHING"
    )
    .bind(bill.id).bind(&bill.title).bind(bill.creator_id).bind(bill.total_kobo).bind(bill.created_at)
    .execute(&mut *tx).await?;

    for (uid, share) in participants {
        sqlx::query(
            "INSERT INTO bill_participants (bill_id, user_id, share_kobo, paid)
             VALUES ($1,$2,$3,false) ON CONFLICT DO NOTHING"
        )
        .bind(bill.id).bind(uid).bind(share)
        .execute(&mut *tx).await?;
    }

    tx.commit().await
}

pub async fn persist_bill_payment(pool: &sqlx::PgPool, bill_id: uuid::Uuid, user_id: uuid::Uuid, all_paid: bool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("UPDATE bill_participants SET paid = true WHERE bill_id = $1 AND user_id = $2")
        .bind(bill_id).bind(user_id)
        .execute(&mut *tx).await?;

    let status = if all_paid { "settled" } else { "partially_paid" };
    sqlx::query("UPDATE bills SET status = $1 WHERE id = $2")
        .bind(status).bind(bill_id)
        .execute(&mut *tx).await?;

    tx.commit().await
}

// ── Direct email send (for OTP — not via outbox) ──────────────────────────────

pub async fn send_otp_email(to: &str, otp: &str, name: &str) {
    let subject = "Your Cowri verification code";
    let body    = format!(
        "Hi {name},\n\nYour Cowri email verification code is:\n\n  {otp}\n\nThis code expires in 15 minutes.\n\nIf you didn't create a Cowri account, ignore this email.\n\nCowri"
    );
    send_email(to, subject, &body).await;
}
