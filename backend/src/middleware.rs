use glideapi::Request;
use uuid::Uuid;
use crate::services::auth::verify_token;

pub fn extract_user(req: &Request) -> Option<Uuid> {
    // 1. Prefer httpOnly cookie (primary after migration)
    let from_cookie = req.headers.get("cookie")
        .and_then(|c| {
            c.split(';').find_map(|part| {
                let part = part.trim();
                let (k, v) = part.split_once('=')?;
                if k.trim() == "access_token" { Some(v.trim().to_string()) } else { None }
            })
        });

    // 2. Fall back to Authorization header (for API clients / mobile)
    let token = from_cookie.or_else(|| {
        req.headers.get("authorization")
            .and_then(|v| v.strip_prefix("Bearer ").map(str::to_string))
    })?;

    verify_token(&token)
}
