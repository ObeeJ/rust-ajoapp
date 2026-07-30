mod state;
mod components;
mod pages;

use leptos::prelude::*;
use pages::{auth::AuthPage, dashboard::DashboardPage};
use state::{clear_tokens, get_token, API_BASE};

#[component]
fn App() -> impl IntoView {
    let (token, set_token)       = signal(get_token());
    let (user_name, set_user_name) = signal(String::new());
    let (balance, set_balance)   = signal(0i64);

    if token.get().is_some() {
        leptos::task::spawn_local(async move {
            let t = get_token().unwrap_or_default();
            if let Ok(r) = gloo_net::http::Request::get(&format!("{API_BASE}/wallet"))
                .header("Authorization", &format!("Bearer {t}"))
                .send()
                .await
            {
                if r.ok() {
                    let data: serde_json::Value = r.json().await.unwrap_or_default();
                    set_balance.set(data["balance_kobo"].as_i64().unwrap_or(0));
                } else {
                    clear_tokens();
                    set_token.set(None);
                }
            }
        });
    }

    view! {
        {move || if token.get().is_some() {
            view! {
                <DashboardPage
                    user_name=user_name.get()
                    balance_kobo=balance.get()
                    on_logout=move || {
                        clear_tokens();
                        set_token.set(None);
                    }
                />
            }.into_any()
        } else {
            view! {
                <AuthPage on_login=move |t, name, bal| {
                    set_token.set(Some(t));
                    set_user_name.set(name);
                    set_balance.set(bal);
                } />
            }.into_any()
        }}
    }
}

fn main() {
    leptos::mount::mount_to_body(App);
}
