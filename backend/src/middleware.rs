use glideapi::{FromRequest, Request, Response};
use uuid::Uuid;
use crate::services::auth::verify_token;
use crate::AppState;

fn token_from_request(req: &Request) -> Option<String> {
    let from_cookie = req.headers.get("cookie").and_then(|c| {
        c.split(';').find_map(|part| {
            let part = part.trim();
            let (k, v) = part.split_once('=')?;
            if k.trim() == "access_token" { Some(v.trim().to_string()) } else { None }
        })
    });
    from_cookie.or_else(|| {
        req.headers.get("authorization")
            .and_then(|v| v.strip_prefix("Bearer ").map(str::to_string))
    })
}

pub fn extract_user(req: &Request) -> Option<Uuid> {
    let state = glideapi::State::<AppState>::from_request(req).ok()?.0;
    let token = token_from_request(req)?;
    verify_token(&state.store, &token).map(|(uid, _)| uid)
}

/// Returns 401 if not authenticated, 403 if not admin
pub fn require_admin(req: &Request) -> Result<Uuid, Response> {
    let state = glideapi::State::<AppState>::from_request(req)
        .map_err(|_| crate::routes::err_resp(500, "Internal server error"))?.0;
    let token = token_from_request(req)
        .ok_or_else(|| crate::routes::err_resp(401, "Unauthorized"))?;
    let (uid, role) = verify_token(&state.store, &token)
        .ok_or_else(|| crate::routes::err_resp(401, "Unauthorized"))?;
    if role != "admin" {
        return Err(crate::routes::err_resp(403, "Forbidden"));
    }
    Ok(uid)
}
