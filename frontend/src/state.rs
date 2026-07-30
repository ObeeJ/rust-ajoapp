use serde::{Deserialize, Serialize};

/// Injected at build time via Trunk: COWRI_API_BASE env var.
/// Falls back to localhost for local dev.
pub const API_BASE: &str = std::env!("COWRI_API_BASE", "http://localhost:3000/v1");

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AuthState {
    pub token: Option<String>,
    pub user_name: Option<String>,
    pub balance_kobo: i64,
}

impl AuthState {
    pub fn formatted_balance(&self) -> String {
        format!("₦{:,.2}", self.balance_kobo as f64 / 100.0)
    }
}

pub fn get_token() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("access_token").ok().flatten())
}

pub fn save_tokens(access: &str, refresh: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item("access_token", access);
        let _ = storage.set_item("refresh_token", refresh);
    }
}

pub fn clear_tokens() {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.remove_item("access_token");
        let _ = storage.remove_item("refresh_token");
    }
}
