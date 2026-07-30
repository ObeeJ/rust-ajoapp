use leptos::prelude::*;
use serde::{Deserialize, Serialize};

pub const API_BASE: &str = "http://localhost:3000";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AuthState {
    pub token: Option<String>,
    pub user_name: Option<String>,
    pub balance_kobo: i64,
}

impl AuthState {
    pub fn is_logged_in(&self) -> bool {
        self.token.is_some()
    }

    pub fn formatted_balance(&self) -> String {
        format!("₦{:,.2}", self.balance_kobo as f64 / 100.0)
    }
}

pub fn get_token() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("token").ok().flatten())
}

pub fn save_token(token: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item("token", token);
    }
}

pub fn clear_token() {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.remove_item("token");
    }
}
