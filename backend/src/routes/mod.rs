use glideapi::{FromRequest, Json, Request, Response};
use shared::*;

use crate::AppState;
use crate::services::auth as auth_svc;
use crate::middleware::extract_user;

fn ok<T: serde::Serialize>(status: u16, v: T) -> Response {
    Response { status, body: serde_json::to_string(&v).unwrap() }
}

fn err(status: u16, msg: &str) -> Response {
    ok(status, ApiError { error: msg.into() })
}

pub async fn register(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let Json(body) = match Json::<RegisterRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "invalid body"),
    };
    match auth_svc::register(&state.store, body) {
        Ok(res) => ok(201, res),
        Err(e) => ok(400, e),
    }
}

pub async fn login(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let Json(body) = match Json::<LoginRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "invalid body"),
    };
    match auth_svc::login(&state.store, body) {
        Ok(res) => ok(200, res),
        Err(e) => ok(401, e),
    }
}

pub async fn get_wallet(req: Request) -> Response {
    let glideapi::State(_state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    match crate::services::wallet::get_wallet(&_state.store, user_id) {
        Some(w) => ok(200, w),
        None => err(404, "wallet not found"),
    }
}

pub async fn get_transactions(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    ok(200, crate::services::wallet::get_transactions(&state.store, user_id))
}

pub async fn fund_wallet(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    let Json(body) = match Json::<FundWalletRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "invalid body"),
    };

    let reference = format!("fund-{}-{}", user_id, uuid::Uuid::new_v4());
    match crate::services::wallet::init_paystack_payment(body.amount_kobo, &body.email, &reference).await {
        Ok(res) => ok(200, res),
        Err(e) => ok(502, e),
    }
}

pub async fn paystack_webhook(req: Request) -> Response {
    // In production: verify x-paystack-signature header
    let body: serde_json::Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v, Err(_) => return err(400, "invalid body"),
    };

    if body["event"] == "charge.success" {
        let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
            Ok(s) => s, Err(_) => return err(500, "state error"),
        };
        let reference = body["data"]["reference"].as_str().unwrap_or("");
        let amount_kobo = body["data"]["amount"].as_i64().unwrap_or(0);
        let email = body["data"]["customer"]["email"].as_str().unwrap_or("");

        // Find user by email
        let user_id = {
            let users = state.store.users.lock().unwrap();
            users.values().find(|u| u.email.as_deref() == Some(email)).map(|u| u.id)
        };

        if let Some(uid) = user_id {
            crate::services::wallet::credit_wallet(&state.store, uid, amount_kobo, reference, "Wallet top-up via Paystack");
        }
    }

    ok(200, serde_json::json!({ "status": "ok" }))
}

pub async fn create_ajo(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    let Json(body) = match Json::<CreateAjoRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "invalid body"),
    };
    ok(201, crate::services::ajo::create_group(&state.store, user_id, body))
}

pub async fn list_ajo(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    ok(200, crate::services::ajo::list_groups(&state.store, user_id))
}

pub async fn join_ajo(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    let group_id = match req.params.get("id").and_then(|s| uuid::Uuid::parse_str(s).ok()) {
        Some(id) => id, None => return err(400, "invalid group id"),
    };
    match crate::services::ajo::join_group(&state.store, group_id, user_id) {
        Ok(m) => ok(200, m),
        Err(e) => ok(400, e),
    }
}

pub async fn contribute_ajo(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    let group_id = match req.params.get("id").and_then(|s| uuid::Uuid::parse_str(s).ok()) {
        Some(id) => id, None => return err(400, "invalid group id"),
    };
    match crate::services::ajo::contribute(&state.store, group_id, user_id) {
        Ok(_) => ok(200, serde_json::json!({ "status": "contributed" })),
        Err(e) => ok(400, e),
    }
}

pub async fn create_bill(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    let Json(body) = match Json::<CreateBillRequest>::from_request(&req) {
        Ok(b) => b, Err(_) => return err(400, "invalid body"),
    };
    match crate::services::bills::create_bill(&state.store, user_id, body) {
        Ok(b) => ok(201, b),
        Err(e) => ok(400, e),
    }
}

pub async fn list_bills(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    ok(200, crate::services::bills::list_bills(&state.store, user_id))
}

pub async fn pay_bill(req: Request) -> Response {
    let glideapi::State(state) = match glideapi::State::<AppState>::from_request(&req) {
        Ok(s) => s, Err(_) => return err(500, "state error"),
    };
    let user_id = match extract_user(&req) {
        Some(id) => id, None => return err(401, "unauthorized"),
    };
    let bill_id = match req.params.get("id").and_then(|s| uuid::Uuid::parse_str(s).ok()) {
        Some(id) => id, None => return err(400, "invalid bill id"),
    };
    match crate::services::bills::pay_bill_share(&state.store, bill_id, user_id) {
        Ok(_) => ok(200, serde_json::json!({ "status": "paid" })),
        Err(e) => ok(400, e),
    }
}
