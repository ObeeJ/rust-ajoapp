use glideapi::Request;
use uuid::Uuid;
use crate::services::auth::verify_token;
use crate::AppState;
use glideapi::FromRequest;

pub fn extract_user(req: &Request) -> Option<Uuid> {
    let state = glideapi::State::<AppState>::from_request(req).ok()?.0;

    let from_cookie = req.headers.get("cookie").and_then(|c| {
        c.split(';').find_map(|part| {
            let part = part.trim();
            let (k, v) = part.split_once('=')?;
            if k.trim() == "access_token" { Some(v.trim().to_string()) } else { None }
        })
    });

    let token = from_cookie.or_else(|| {
        req.headers.get("authorization")
            .and_then(|v| v.strip_prefix("Bearer ").map(str::to_string))
    })?;

    verify_token(&state.store, &token)
}
