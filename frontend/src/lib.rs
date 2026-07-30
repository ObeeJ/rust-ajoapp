mod state;
mod components;
mod pages;

use leptos::prelude::*;
use pages::{auth::AuthPage, dashboard::DashboardPage};
use state::API_BASE;

#[component]
fn App() -> impl IntoView {
    let (logged_in, set_logged_in) = signal(false);
    let (user_name, set_user_name) = signal(String::new());
    let (balance, set_balance)     = signal(0i64);

    // Check session on load — cookie is sent automatically by the browser
    leptos::task::spawn_local(async move {
        if let Ok(r) = gloo_net::http::Request::get(&format!("{API_BASE}/wallet"))
            .credentials(web_sys::RequestCredentials::Include) // send cookies
            .send()
            .await
        {
            if r.ok() {
                let data: serde_json::Value = r.json().await.unwrap_or_default();
                set_balance.set(data["available_kobo"].as_i64().unwrap_or(0));
                set_logged_in.set(true);
            }
        }
    });

    view! {
        {move || if logged_in.get() {
            view! {
                <DashboardPage
                    user_name=user_name.get()
                    balance_kobo=balance.get()
                    on_logout=move || {
                        // Tell backend to clear cookies
                        leptos::task::spawn_local(async move {
                            let _ = gloo_net::http::Request::post(&format!("{API_BASE}/auth/logout"))
                                .credentials(web_sys::RequestCredentials::Include)
                                .send()
                                .await;
                        });
                        set_logged_in.set(false);
                    }
                />
            }.into_any()
        } else {
            view! {
                <AuthPage on_login=move |_, name, bal| {
                    set_logged_in.set(true);
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
