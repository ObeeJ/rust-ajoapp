use glideapi::Request;
use uuid::Uuid;
use crate::services::auth::verify_token;

pub fn extract_user(req: &Request) -> Option<Uuid> {
    req.headers.get("authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(verify_token)
}
