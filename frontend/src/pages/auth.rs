use leptos::prelude::*;
use gloo_net::http::Request;
use serde_json::json;

use crate::state::{save_token, API_BASE};
use crate::components::{Button, Input};

#[component]
pub fn AuthPage(on_login: impl Fn(String, String, i64) + 'static + Clone) -> impl IntoView {
    let (is_register, set_register) = signal(false);
    let (name, set_name) = signal(String::new());
    let (phone, set_phone) = signal(String::new());
    let (pin, set_pin) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);

    let on_login_clone = on_login.clone();

    let submit = move || {
        let phone = phone.get();
        let pin = pin.get();
        let name = name.get();
        let is_reg = is_register.get();
        let on_login = on_login_clone.clone();

        if phone.is_empty() || pin.len() != 4 {
            set_error.set(Some("Enter phone and 4-digit PIN".into()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            let url = if is_reg { format!("{API_BASE}/auth/register") } else { format!("{API_BASE}/auth/login") };
            let body = if is_reg {
                json!({ "name": name, "phone": phone, "pin": pin })
            } else {
                json!({ "phone": phone, "pin": pin })
            };

            let res = Request::post(&url)
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .unwrap()
                .send()
                .await;

            set_loading.set(false);

            match res {
                Ok(r) if r.ok() => {
                    let data: serde_json::Value = r.json().await.unwrap_or_default();
                    let token = data["token"].as_str().unwrap_or("").to_string();
                    let uname = data["user"]["name"].as_str().unwrap_or("").to_string();
                    let balance = data["wallet"]["balance_kobo"].as_i64().unwrap_or(0);
                    save_token(&token);
                    on_login(token, uname, balance);
                }
                Ok(r) => {
                    let data: serde_json::Value = r.json().await.unwrap_or_default();
                    set_error.set(Some(data["error"].as_str().unwrap_or("Something went wrong").to_string()));
                }
                Err(_) => set_error.set(Some("Network error. Check your connection.".into())),
            }
        });
    };

    view! {
        <div class="min-h-screen bg-gradient-to-br from-green-50 to-emerald-100 flex items-center justify-center p-4">
            <div class="w-full max-w-sm">
                // Logo / Brand
                <div class="text-center mb-8">
                    <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-green-600 text-white text-2xl font-bold mb-3 shadow-lg">
                        "₦"
                    </div>
                    <h1 class="text-2xl font-bold text-gray-900">"Cowri"</h1>
                    <p class="text-sm text-gray-500 mt-1">"Save together. Split easy. Pay fast."</p>
                </div>

                <div class="bg-white rounded-2xl shadow-xl p-6 border border-gray-100">
                    // Tab toggle
                    <div class="flex rounded-xl bg-gray-100 p-1 mb-6">
                        <button
                            class=move || if !is_register.get() { "flex-1 rounded-lg py-2 text-sm font-semibold bg-white shadow text-green-700 transition" } else { "flex-1 rounded-lg py-2 text-sm font-medium text-gray-500 transition" }
                            on:click=move |_| set_register.set(false)
                        >"Sign In"</button>
                        <button
                            class=move || if is_register.get() { "flex-1 rounded-lg py-2 text-sm font-semibold bg-white shadow text-green-700 transition" } else { "flex-1 rounded-lg py-2 text-sm font-medium text-gray-500 transition" }
                            on:click=move |_| set_register.set(true)
                        >"Create Account"</button>
                    </div>

                    <div class="space-y-3">
                        {move || is_register.get().then(|| view! {
                            <Input placeholder="Full name" value=name.get() on_input=move |v| set_name.set(v) />
                        })}
                        <Input placeholder="Phone (e.g. 08012345678)" value=phone.get() on_input=move |v| set_phone.set(v) />
                        <Input placeholder="4-digit PIN" value=pin.get() input_type="password" on_input=move |v| set_pin.set(v) />
                    </div>

                    {move || error.get().map(|e| view! {
                        <p class="mt-3 text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2">{e}</p>
                    })}

                    <div class="mt-5">
                        <Button
                            label=move || if is_register.get() { "Create Account".to_string() } else { "Sign In".to_string() }
                            disabled=loading.get()
                            on_click=submit.clone()
                        />
                    </div>

                    <p class="mt-4 text-center text-xs text-gray-400">
                        "No hidden charges. No bank wahala. 🇳🇬"
                    </p>
                </div>
            </div>
        </div>
    }
}
