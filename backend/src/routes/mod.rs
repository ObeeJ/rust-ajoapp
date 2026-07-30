use glideapi::{FromRequest, Json, Request, Response};
use hmac::{Hmac, Mac};
use sha2::Sha512;
use shared::*;
use chrono::Utc;

use crate::AppState;
use crate::services::auth as auth_svc;
use crate::middleware::extract_user;
use crate::db;

const DEFAULT_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize     = 100;

/// Parse a single named value from a Cookie header string
fn parse_cookie(header: &str, name: &str) -> Option<String> {
    header.split(';').find_map(|part| {
        let part = part.trim();
        let (k, v) = part.split_once('=')?;
        if k.trim() == name { Some(v.trim().to_string()) } else { None }
    })
}

fn ok<T: serde::Serialize>(status: u16, v: T) -> Response {
    Response {
        status,
        body: serde_json::to_string(&v).unwrap_or_else(|_| "{}".into()),
        headers: vec![],
    }
}

fn err(status: u16, msg: &str) -> Response {
    ok(status, ApiError { error: msg.into() })
}

/// Build a Secure, HttpOnly, SameSite=Strict cookie value
fn cookie(name: &str, value: &str, max_age_secs: u32) -> String {
    format!(
        "{name}={value}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age={max_age_secs}"
    )
}

fn set_auth_cookies(mut resp: Response, access: &str, refresh: &str) -> Response {
    resp.headers.push(("Set-Cookie".into(), cookie("access_token",  access,  900)));       // 15 min
    resp.headers.push(("Set-Cookie".into(), cookie("refresh_token", refresh, 604800)));    // 7 days
    resp
}

fn clear_auth_cookies(mut resp: Response) -> Response {
    resp.headers.push(("Set-Cookie".into(), cookie("access_token",  "", 0)));
    resp.headers.push(("Set-Cookie".into(), cookie("refresh_token", "", 0)));
    resp
}

fn parse_pagination(req: &Request) -> (usize, usize) {
    // GlideAPI doesn't expose a query map — parse from path manually
    let raw_path = &req.path;
    let qs = raw_path.splitn(2, '?').nth(1).unwrap_or("");

    let mut page     = 0usize;
    let mut per_page = DEFAULT_PAGE_SIZE;

    for pair in qs.split('&') {
        let mut kv = pair.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("page"),     Some(v)) => page     = v.parse().unwrap_or(0),
            (Some("per_page"), Some(v)) => per_page = v.parse().unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE),
            _ => {}
        }
    }
    (page, per_page)
}

fn state(req: &Request) -> Result<AppState, Response> {
    glideapi::State::<AppState>::from_request(req)
        .map(|glideapi::State(s)| s)
        .map_err(|_| err(500, "Internal server error"))
}

fn auth(req: &Request) -> Result<uuid::Uuid, Response> {
    extract_user(req).ok_or_else(|| err(401, "Unauthorized"))
}

// ── Auth ──────────────────────────────────────────────────────────────────────

pub async fn register(req: Request) -> Response {
    let state = match state(&req) { Ok(s) => s, Err(e) => return e };
    let Json(body) = match Json::<RegisterRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "Invalid request body"),
    };
    match auth_svc::register(&state.store, body) {
        Ok(t) => set_auth_cookies(
            ok(201, serde_json::json!({ "user": t.user, "wallet": t.wallet })),
            &t.access_token, &t.refresh_token,
        ),
        Err(e) if e.error.contains("already registered") => ok(409, e),
        Err(e) => ok(400, e),
    }
}

pub async fn login(req: Request) -> Response {
    let state = match state(&req) { Ok(s) => s, Err(e) => return e };
    let Json(body) = match Json::<LoginRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "Invalid request body"),
    };
    match auth_svc::login(&state.store, body) {
        Ok(t) => set_auth_cookies(
            ok(200, serde_json::json!({ "user": t.user, "wallet": t.wallet })),
            &t.access_token,
            &t.refresh_token,
        ),
        Err(e) => ok(401, e),
    }
}

pub async fn logout(req: Request) -> Response {
    if let Ok(state) = state(&req) {
        // Invalidate access token JTI so it can't be used even within its 15-min window
        if let Some(token) = req.headers.get("cookie")
            .and_then(|c| parse_cookie(c, "access_token"))
        {
            if let Some(jti) = auth_svc::extract_jti(&token) {
                state.store.denied_jtis.lock().unwrap().insert(jti);
            }
        }
        // Invalidate refresh token
        if let Some(rt) = req.headers.get("cookie")
            .and_then(|c| parse_cookie(c, "refresh_token"))
        {
            let _ = auth_svc::rotate_refresh_token(&state.store, &rt);
        }
    }
    clear_auth_cookies(ok(200, serde_json::json!({ "status": "logged out" })))
}

pub async fn refresh_token(req: Request) -> Response {
    let state = match state(&req) { Ok(s) => s, Err(e) => return e };

    // Read refresh token from httpOnly cookie
    let rt = req.headers.get("cookie")
        .and_then(|c| parse_cookie(c, "refresh_token"));

    let rt = match rt {
        Some(t) => t,
        None => return err(401, "No refresh token"),
    };

    match auth_svc::rotate_refresh_token(&state.store, &rt) {
        Ok((access, refresh)) => set_auth_cookies(
            ok(200, serde_json::json!({ "status": "refreshed" })),
            &access,
            &refresh,
        ),
        Err(e) => ok(401, e),
    }
}

// ── Wallet ────────────────────────────────────────────────────────────────────

pub async fn get_wallet(req: Request) -> Response {
    let s       = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    match crate::services::wallet::get_wallet(&s.store, user_id) {
        Some(w) => ok(200, w),
        None    => err(404, "Wallet not found"),
    }
}

pub async fn get_transactions(req: Request) -> Response {
    let s       = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    let (page, per_page) = parse_pagination(&req);
    ok(200, crate::services::wallet::get_transactions(&s.store, user_id, page, per_page))
}

pub async fn fund_wallet(req: Request) -> Response {
    let state   = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    let Json(body) = match Json::<FundWalletRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "Invalid request body"),
    };

    // Postgres-backed idempotency (survives restarts)
    let idem_key = req.headers.get("x-idempotency-key")
        .map(|k| format!("fund-{}-{}", user_id, k));

    if let Some(ref key) = idem_key {
        if let Some((status, body)) = db::get_idempotency(&state.db, key).await {
            return Response { status, body, headers: vec![] };
        }
    }

    let reference = format!("fund-{}-{}", user_id, uuid::Uuid::new_v4());
    let resp = match crate::services::wallet::init_paystack_payment(
        body.amount_kobo, &body.email, &reference,
    ).await {
        Ok(res) => ok(200, res),
        Err(e)  => ok(502, e),
    };

    if let Some(key) = idem_key {
        db::set_idempotency(&state.db, &key, resp.status, &resp.body).await;
    }

    resp
}

/// Verifies Paystack HMAC-SHA512 signature before processing any event
pub async fn paystack_webhook(req: Request) -> Response {
    let secret_key = match std::env::var("PAYSTACK_SECRET_KEY") {
        Ok(k) => k,
        Err(_) => return err(500, "Internal server error"),
    };

    // Verify x-paystack-signature
    let signature = req.headers.get("x-paystack-signature").cloned().unwrap_or_default();
    let mut mac = Hmac::<Sha512>::new_from_slice(secret_key.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(&req.body);
    let expected = hex::encode(mac.finalize().into_bytes());

    if signature != expected {
        return err(401, "Invalid signature");
    }

    let body: serde_json::Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v, Err(_) => return err(400, "Invalid body"),
    };

    if body["event"] == "charge.success" {
        let state = match state(&req) { Ok(s) => s, Err(e) => return e };

        let reference   = body["data"]["reference"].as_str().unwrap_or("");
        let amount_kobo = body["data"]["amount"].as_i64().unwrap_or(0);

        // Webhook idempotency — check if this reference was already credited in the ledger
        if db::webhook_already_processed(&state.db, reference).await {
            return ok(200, serde_json::json!({ "status": "already_processed" }));
        }

        let phone = body["data"]["metadata"]["phone"].as_str()
            .or_else(|| body["data"]["customer"]["phone"].as_str());
        let email = body["data"]["customer"]["email"].as_str();

        let user_id = {
            let users  = state.store.users.lock().unwrap();
            let phones = state.store.phone_index.lock().unwrap();
            phone
                .and_then(|p| phones.get(p).copied())
                .or_else(|| email.and_then(|e| {
                    users.values().find(|u| u.email.as_deref() == Some(e)).map(|u| u.id)
                }))
        };

        if let Some(uid) = user_id {
            if amount_kobo > 0 && !reference.is_empty() {
                // Persist to DB (atomic: ledger + outbox in one transaction)
                let wallet_id = state.store.wallets.lock().unwrap()
                    .get(&uid).map(|w| w.id);
                if let Some(wid) = wallet_id {
                    let _ = db::credit(&state.db, wid, amount_kobo, reference, "Wallet top-up via Paystack").await;
                }
                // Also update in-memory cache
                crate::services::wallet::credit_wallet(
                    &state.store, uid, amount_kobo, reference, "Wallet top-up via Paystack",
                );
            }
        }
    }

    ok(200, serde_json::json!({ "status": "ok" }))
}

// ── Ajo ───────────────────────────────────────────────────────────────────────

pub async fn create_ajo(req: Request) -> Response {
    let state   = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    let Json(body) = match Json::<CreateAjoRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "Invalid request body"),
    };
    match crate::services::ajo::create_group(&state.store, user_id, body) {
        Ok(g)  => ok(201, g),
        Err(e) => ok(400, e),
    }
}

pub async fn list_ajo(req: Request) -> Response {
    let state   = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    ok(200, crate::services::ajo::list_groups(&state.store, user_id))
}

pub async fn join_ajo(req: Request) -> Response {
    let state   = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    let group_id = match req.params.get("id").and_then(|s| uuid::Uuid::parse_str(s).ok()) {
        Some(id) => id, None => return err(400, "Invalid group ID"),
    };
    match crate::services::ajo::join_group(&state.store, group_id, user_id) {
        Ok(m)  => ok(200, m),
        Err(e) if e.error.contains("full") || e.error.contains("Already") => ok(409, e),
        Err(e) => ok(400, e),
    }
}

pub async fn contribute_ajo(req: Request) -> Response {
    let state   = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    let group_id = match req.params.get("id").and_then(|s| uuid::Uuid::parse_str(s).ok()) {
        Some(id) => id, None => return err(400, "Invalid group ID"),
    };
    match crate::services::ajo::contribute(&state.store, group_id, user_id) {
        Ok(_)  => ok(200, serde_json::json!({ "status": "contributed" })),
        Err(e) if e.error.contains("Insufficient") => ok(402, e),
        Err(e) if e.error.contains("Already contributed") => ok(409, e),
        Err(e) => ok(400, e),
    }
}

// ── Bills ─────────────────────────────────────────────────────────────────────

pub async fn create_bill(req: Request) -> Response {
    let state   = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    let Json(body) = match Json::<CreateBillRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "Invalid request body"),
    };
    match crate::services::bills::create_bill(&state.store, user_id, body) {
        Ok(b)  => ok(201, b),
        Err(e) => ok(400, e),
    }
}

pub async fn list_bills(req: Request) -> Response {
    let state   = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    let (page, per_page) = parse_pagination(&req);
    ok(200, crate::services::bills::list_bills(&state.store, user_id, page, per_page))
}

pub async fn pay_bill(req: Request) -> Response {
    let state   = match state(&req) { Ok(s) => s, Err(e) => return e };
    let user_id = match auth(&req)  { Ok(id) => id, Err(e) => return e };
    let bill_id = match req.params.get("id").and_then(|s| uuid::Uuid::parse_str(s).ok()) {
        Some(id) => id, None => return err(400, "Invalid bill ID"),
    };
    match crate::services::bills::pay_bill_share(&state.store, bill_id, user_id) {
        Ok(_)  => ok(200, serde_json::json!({ "status": "paid" })),
        Err(e) if e.error.contains("Insufficient") => ok(402, e),
        Err(e) if e.error.contains("Already paid") => ok(409, e),
        Err(e) => ok(400, e),
    }
}
