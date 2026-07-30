use glideapi::{FromRequest, Json, Request, Response};
use shared::*;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::require_admin;
use crate::routes::{err_resp, ok_resp};

// ── Dashboard stats ───────────────────────────────────────────────────────────

pub async fn dashboard(req: Request) -> Response {
    if let Err(e) = require_admin(&req) { return e; }
    let state = match glideapi::State::<AppState>::from_request(&req) {
        Ok(glideapi::State(s)) => s, Err(_) => return err_resp(500, "Internal server error"),
    };

    let users_count   = state.store.users.lock().unwrap().len();
    let wallets       = state.store.wallets.lock().unwrap();
    let total_wallets = wallets.len();
    let total_balance_kobo: i64 = wallets.values().map(|w| w.available_kobo).sum();
    drop(wallets);

    let txns          = state.store.transactions.lock().unwrap();
    let total_txns    = txns.len();
    let total_volume_kobo: i64 = txns.iter()
        .filter(|t| t.kind == TransactionKind::Credit)
        .map(|t| t.amount_kobo).sum();
    drop(txns);

    let ajo_groups    = state.store.ajo_groups.lock().unwrap().len();
    let active_groups = state.store.ajo_groups.lock().unwrap()
        .values().filter(|g| g.status == AjoStatus::Active).count();
    let bills_count   = state.store.bills.lock().unwrap().len();

    // Fee revenue: 0.5% of all ajo contribution volume
    let contributions = state.store.ajo_contributions.lock().unwrap().len();
    let fee_revenue_kobo: i64 = state.store.ajo_groups.lock().unwrap()
        .values()
        .map(|g| {
            let contribs = state.store.ajo_contributions.lock().unwrap()
                .iter().filter(|(gid, _, _)| *gid == g.id).count() as i64;
            (g.contribution_kobo / 200) * contribs // 0.5% per contribution
        })
        .sum();

    ok_resp(200, serde_json::json!({
        "users":              users_count,
        "wallets":            total_wallets,
        "total_balance_kobo": total_balance_kobo,
        "transactions":       total_txns,
        "volume_kobo":        total_volume_kobo,
        "ajo_groups":         ajo_groups,
        "active_groups":      active_groups,
        "bills":              bills_count,
        "contributions":      contributions,
        "fee_revenue_kobo":   fee_revenue_kobo,
    }))
}

// ── Users ─────────────────────────────────────────────────────────────────────

pub async fn list_users(req: Request) -> Response {
    if let Err(e) = require_admin(&req) { return e; }
    let state = match glideapi::State::<AppState>::from_request(&req) {
        Ok(glideapi::State(s)) => s, Err(_) => return err_resp(500, "Internal server error"),
    };

    let users = state.store.users.lock().unwrap();
    let wallets = state.store.wallets.lock().unwrap();

    let list: Vec<_> = users.values().map(|u| {
        let balance = wallets.values()
            .find(|w| w.user_id == u.id)
            .map(|w| w.available_kobo)
            .unwrap_or(0);
        serde_json::json!({
            "id":             u.id,
            "name":           u.name,
            "phone":          u.phone,
            "role":           u.role,
            "balance_kobo":   balance,
            "created_at":     u.created_at,
        })
    }).collect();

    ok_resp(200, serde_json::json!({ "users": list, "total": list.len() }))
}

pub async fn get_user(req: Request) -> Response {
    if let Err(e) = require_admin(&req) { return e; }
    let state = match glideapi::State::<AppState>::from_request(&req) {
        Ok(glideapi::State(s)) => s, Err(_) => return err_resp(500, "Internal server error"),
    };

    let user_id = match req.params.get("id").and_then(|s| Uuid::parse_str(s).ok()) {
        Some(id) => id, None => return err_resp(400, "Invalid user ID"),
    };

    let user = match state.store.users.lock().unwrap().get(&user_id).cloned() {
        Some(u) => u, None => return err_resp(404, "User not found"),
    };

    let wallet = state.store.wallets.lock().unwrap().get(&user_id).cloned();

    let txns: Vec<_> = {
        let wallet_id = wallet.as_ref().map(|w| w.id);
        state.store.transactions.lock().unwrap()
            .iter()
            .filter(|t| Some(t.wallet_id) == wallet_id)
            .rev().take(20).cloned().collect()
    };

    ok_resp(200, serde_json::json!({
        "user":         user,
        "wallet":       wallet,
        "transactions": txns,
    }))
}

#[derive(serde::Deserialize)]
struct SetRoleRequest { role: String }

pub async fn set_user_role(req: Request) -> Response {
    if let Err(e) = require_admin(&req) { return e; }
    let state = match glideapi::State::<AppState>::from_request(&req) {
        Ok(glideapi::State(s)) => s, Err(_) => return err_resp(500, "Internal server error"),
    };

    let user_id = match req.params.get("id").and_then(|s| Uuid::parse_str(s).ok()) {
        Some(id) => id, None => return err_resp(400, "Invalid user ID"),
    };

    let Json(body) = match Json::<SetRoleRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err_resp(400, "Invalid request body"),
    };

    let new_role = match body.role.as_str() {
        "admin" => UserRole::Admin,
        "user"  => UserRole::User,
        _       => return err_resp(400, "Role must be 'user' or 'admin'"),
    };

    let mut users = state.store.users.lock().unwrap();
    match users.get_mut(&user_id) {
        Some(u) => { u.role = new_role; ok_resp(200, serde_json::json!({ "status": "updated" })) }
        None    => err_resp(404, "User not found"),
    }
}

// ── Transactions ──────────────────────────────────────────────────────────────

pub async fn list_transactions(req: Request) -> Response {
    if let Err(e) = require_admin(&req) { return e; }
    let state = match glideapi::State::<AppState>::from_request(&req) {
        Ok(glideapi::State(s)) => s, Err(_) => return err_resp(500, "Internal server error"),
    };

    // Parse pagination from path query string
    let qs = req.path.splitn(2, '?').nth(1).unwrap_or("");
    let mut page = 0usize; let mut per_page = 50usize;
    for pair in qs.split('&') {
        let mut kv = pair.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("page"),     Some(v)) => page     = v.parse().unwrap_or(0),
            (Some("per_page"), Some(v)) => per_page = v.parse().unwrap_or(50).min(200),
            _ => {}
        }
    }

    let txns = state.store.transactions.lock().unwrap();
    let total = txns.len();
    let page_data: Vec<_> = txns.iter().rev()
        .skip(page * per_page).take(per_page).cloned().collect();

    ok_resp(200, serde_json::json!({
        "transactions": page_data,
        "total":        total,
        "page":         page,
        "per_page":     per_page,
    }))
}

// ── Ajo oversight ─────────────────────────────────────────────────────────────

pub async fn list_all_ajo(req: Request) -> Response {
    if let Err(e) = require_admin(&req) { return e; }
    let state = match glideapi::State::<AppState>::from_request(&req) {
        Ok(glideapi::State(s)) => s, Err(_) => return err_resp(500, "Internal server error"),
    };

    let groups: Vec<_> = state.store.ajo_groups.lock().unwrap().values().map(|g| {
        let member_count = state.store.ajo_members.lock().unwrap()
            .keys().filter(|(gid, _)| *gid == g.id).count();
        let contributions = state.store.ajo_contributions.lock().unwrap()
            .iter().filter(|(gid, _, _)| *gid == g.id).count();
        let fee_collected_kobo = (g.contribution_kobo / 200) * contributions as i64;
        serde_json::json!({
            "group":              g,
            "member_count":       member_count,
            "total_contributions": contributions,
            "fee_collected_kobo": fee_collected_kobo,
        })
    }).collect();

    ok_resp(200, serde_json::json!({ "groups": groups, "total": groups.len() }))
}

// ── Outbox status ─────────────────────────────────────────────────────────────

pub async fn outbox_status(req: Request) -> Response {
    if let Err(e) = require_admin(&req) { return e; }
    let state = match glideapi::State::<AppState>::from_request(&req) {
        Ok(glideapi::State(s)) => s, Err(_) => return err_resp(500, "Internal server error"),
    };

    let outbox = state.store.outbox.lock().unwrap();
    let pending   = outbox.iter().filter(|e| e.status == OutboxStatus::Pending).count();
    let delivered = outbox.iter().filter(|e| e.status == OutboxStatus::Delivered).count();
    let failed    = outbox.iter().filter(|e| e.status == OutboxStatus::Failed).count();

    ok_resp(200, serde_json::json!({
        "pending":   pending,
        "delivered": delivered,
        "failed":    failed,
        "total":     outbox.len(),
    }))
}

// ── Bootstrap first admin (disabled after first use) ─────────────────────────

pub async fn bootstrap_admin(req: Request) -> Response {
    let state = match glideapi::State::<AppState>::from_request(&req) {
        Ok(glideapi::State(s)) => s, Err(_) => return err_resp(500, "Internal server error"),
    };

    // Only works if zero admins exist — self-disabling
    let has_admin = state.store.users.lock().unwrap()
        .values().any(|u| u.role == UserRole::Admin);
    if has_admin {
        return err_resp(403, "Admin already exists");
    }

    // Require bootstrap secret from env
    let secret = std::env::var("ADMIN_BOOTSTRAP_SECRET")
        .unwrap_or_default();
    let provided = req.headers.get("x-bootstrap-secret").cloned().unwrap_or_default();
    if secret.is_empty() || provided != secret {
        return err_resp(403, "Invalid bootstrap secret");
    }

    let Json(body) = match Json::<shared::RegisterRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err_resp(400, "Invalid request body"),
    };

    match crate::services::auth::register(&state.store, body) {
        Ok(mut t) => {
            // Upgrade to admin
            let mut users = state.store.users.lock().unwrap();
            if let Some(u) = users.get_mut(&t.user.id) {
                u.role = UserRole::Admin;
                t.user.role = UserRole::Admin;
            }
            ok_resp(201, serde_json::json!({ "user": t.user, "wallet": t.wallet }))
        }
        Err(e) => ok_resp(400, e),
    }
}
