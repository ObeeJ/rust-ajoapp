/// Injected at build time via Trunk: COWRI_API_BASE env var.
pub const API_BASE: &str = std::env!("COWRI_API_BASE", "http://localhost:3000/v1");

/// The frontend never reads or writes tokens — they live in httpOnly cookies
/// set by the backend. This is intentionally empty.
pub fn clear_tokens() {
    // Tokens are httpOnly cookies — cleared by POST /v1/auth/logout
}
