use leptos::prelude::*;
use gloo_net::http::Request as HttpRequest;

use crate::state::{get_token, API_BASE};
use crate::components::{Button, Card, Badge, Spinner};

#[component]
pub fn DashboardPage(
    user_name: String,
    balance_kobo: i64,
    on_logout: impl Fn() + 'static,
) -> impl IntoView {
    let (tab, set_tab) = signal("wallet");
    let formatted = format!("₦{:,.2}", balance_kobo as f64 / 100.0);

    view! {
        <div class="min-h-screen bg-gray-50">
            // Top nav
            <header class="bg-white border-b border-gray-100 px-4 py-3 flex items-center justify-between sticky top-0 z-10 shadow-sm">
                <div class="flex items-center gap-2">
                    <div class="w-8 h-8 rounded-xl bg-green-600 text-white text-sm font-bold flex items-center justify-center">"₦"</div>
                    <span class="font-bold text-gray-900">"Cowri"</span>
                </div>
                <div class="flex items-center gap-3">
                    <span class="text-sm text-gray-600">{format!("Hi, {}", user_name.split_whitespace().next().unwrap_or(&user_name))}</span>
                    <Button label="Logout" variant="ghost" on_click=on_logout />
                </div>
            </header>

            // Balance hero
            <div class="bg-gradient-to-r from-green-600 to-emerald-500 text-white px-4 py-8 text-center">
                <p class="text-sm opacity-80 mb-1">"Wallet Balance"</p>
                <p class="text-4xl font-bold tracking-tight">{formatted}</p>
                <p class="text-xs opacity-70 mt-1">"Available to spend"</p>
            </div>

            // Quick actions
            <div class="px-4 -mt-4">
                <div class="bg-white rounded-2xl shadow-sm border border-gray-100 p-4 grid grid-cols-4 gap-2">
                    {quick_action(svg_topup(),   "Top Up",  move || set_tab.set("wallet"))}
                    {quick_action(svg_ajo(),     "Ajo",     move || set_tab.set("ajo"))}
                    {quick_action(svg_split(),   "Split",   move || set_tab.set("bills"))}
                    {quick_action(svg_history(), "History", move || set_tab.set("history"))}
                </div>
            </div>

            // Tab content
            <div class="px-4 mt-4 pb-24">
                {move || match tab.get() {
                    "ajo"     => view! { <AjoTab /> }.into_any(),
                    "bills"   => view! { <BillsTab /> }.into_any(),
                    "history" => view! { <HistoryTab /> }.into_any(),
                    _         => view! { <WalletTab /> }.into_any(),
                }}
            </div>

            // Bottom nav
            <nav class="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-100 flex justify-around py-2 z-10" aria-label="Main navigation">
                {nav_item(svg_topup(),   "Home",    "wallet",  tab, set_tab)}
                {nav_item(svg_ajo(),     "Ajo",     "ajo",     tab, set_tab)}
                {nav_item(svg_split(),   "Bills",   "bills",   tab, set_tab)}
                {nav_item(svg_history(), "History", "history", tab, set_tab)}
            </nav>
        </div>
    }
}

fn quick_action(icon: impl IntoView, label: &'static str, on_click: impl Fn() + 'static) -> impl IntoView {
    view! {
        <button
            class="flex flex-col items-center gap-1 p-2 rounded-xl hover:bg-gray-50 transition cursor-pointer"
            aria-label=label
            on:click=move |_| on_click()
        >
            <span class="w-6 h-6 text-green-600" aria-hidden="true">{icon}</span>
            <span class="text-xs text-gray-600 font-medium">{label}</span>
        </button>
    }
}

fn nav_item(icon: impl IntoView, label: &'static str, value: &'static str, tab: ReadSignal<&'static str>, set_tab: WriteSignal<&'static str>) -> impl IntoView {
    view! {
        <button
            class=move || if tab.get() == value {
                "flex flex-col items-center gap-0.5 px-4 text-green-600 cursor-pointer"
            } else {
                "flex flex-col items-center gap-0.5 px-4 text-gray-400 cursor-pointer"
            }
            aria-label=label
            aria-current=move || if tab.get() == value { "page" } else { "false" }
            on:click=move |_| set_tab.set(value)
        >
            <span class="w-5 h-5" aria-hidden="true">{icon}</span>
            <span class="text-xs font-medium">{label}</span>
        </button>
    }
}

// ── SVG Icons ─────────────────────────────────────────────────────────────────

fn svg_topup() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
        </svg>
    }
}

fn svg_ajo() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M18 18.72a9.094 9.094 0 0 0 3.741-.479 3 3 0 0 0-4.682-2.72m.94 3.198.001.031c0 .225-.012.447-.037.666A11.944 11.944 0 0 1 12 21c-2.17 0-4.207-.576-5.963-1.584A6.062 6.062 0 0 1 6 18.719m12 0a5.971 5.971 0 0 0-.941-3.197m0 0A5.995 5.995 0 0 0 12 12.75a5.995 5.995 0 0 0-5.058 2.772m0 0a3 3 0 0 0-4.681 2.72 8.986 8.986 0 0 0 3.74.477m.94-3.197a5.971 5.971 0 0 0-.94 3.197M15 6.75a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm6 3a2.25 2.25 0 1 1-4.5 0 2.25 2.25 0 0 1 4.5 0Zm-13.5 0a2.25 2.25 0 1 1-4.5 0 2.25 2.25 0 0 1 4.5 0Z" />
        </svg>
    }
}

fn svg_split() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 14.25l6-6m4.5-3.493V21.75l-3.75-1.5-3.75 1.5-3.75-1.5-3.75 1.5V4.757c0-1.108.806-2.057 1.907-2.185a48.507 48.507 0 0 1 11.186 0c1.1.128 1.907 1.077 1.907 2.185Z" />
        </svg>
    }
}

fn svg_history() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
        </svg>
    }
}

// ── Wallet Tab ────────────────────────────────────────────────────────────────

#[component]
fn WalletTab() -> impl IntoView {
    let (amount, set_amount) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (msg, set_msg) = signal(Option::<String>::None);

    let fund = move || {
        let amount = amount.get();
        let email = email.get();
        let Ok(kobo) = amount.parse::<f64>().map(|n| (n * 100.0) as i64) else {
            set_msg.set(Some("Enter a valid amount".into())); return;
        };

        leptos::task::spawn_local(async move {
            let token = get_token().unwrap_or_default();
            let res = HttpRequest::post(&format!("{API_BASE}/wallet/fund"))
                .header("Authorization", &format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(serde_json::json!({ "amount_kobo": kobo, "email": email }).to_string())
                .unwrap()
                .send().await;

            if let Ok(r) = res {
                let data: serde_json::Value = r.json().await.unwrap_or_default();
                if let Some(url) = data["authorization_url"].as_str() {
                    let _ = web_sys::window().unwrap().location().set_href(url);
                } else {
                    set_msg.set(Some(data["error"].as_str().unwrap_or("Failed").to_string()));
                }
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h2 class="text-base font-semibold text-gray-800">"Top Up Wallet"</h2>
            <Card>
                <div class="space-y-3">
                    <div>
                        <label class="text-xs font-medium text-gray-500 mb-1 block">"Amount (₦)"</label>
                        <input
                            type="number"
                            placeholder="e.g. 5000"
                            class="w-full rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200"
                            on:input=move |e| set_amount.set(event_target_value(&e))
                        />
                    </div>
                    <div>
                        <label class="text-xs font-medium text-gray-500 mb-1 block">"Email (for Paystack)"</label>
                        <input
                            type="email"
                            placeholder="you@email.com"
                            class="w-full rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200"
                            on:input=move |e| set_email.set(event_target_value(&e))
                        />
                    </div>
                    {move || msg.get().map(|m| view! { <p class="text-sm text-red-600">{m}</p> })}
                    <Button label="Pay with Paystack" on_click=fund />
                </div>
            </Card>
            <div class="flex items-center gap-2 text-xs text-gray-400 justify-center">
                <span>"🔒"</span>
                <span>"Secured by Paystack · No card stored"</span>
            </div>
        </div>
    }
}

// ── Ajo Tab ───────────────────────────────────────────────────────────────────

#[component]
fn AjoTab() -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (amount, set_amount) = signal(String::new());
    let (members, set_members) = signal(String::new());
    let (groups, set_groups) = signal(Vec::<serde_json::Value>::new());
    let (loading, set_loading) = signal(true);

    let fetch_groups = move || {
        leptos::task::spawn_local(async move {
            let token = get_token().unwrap_or_default();
            if let Ok(r) = HttpRequest::get(&format!("{API_BASE}/ajo"))
                .header("Authorization", &format!("Bearer {token}"))
                .send().await
            {
                let data: Vec<serde_json::Value> = r.json().await.unwrap_or_default();
                set_groups.set(data);
            }
            set_loading.set(false);
        });
    };

    fetch_groups();

    let create = move || {
        let name = name.get();
        let Ok(kobo) = amount.get().parse::<f64>().map(|n| (n * 100.0) as i64) else { return };
        let Ok(count) = members.get().parse::<u32>() else { return };

        leptos::task::spawn_local(async move {
            let token = get_token().unwrap_or_default();
            let _ = HttpRequest::post(&format!("{API_BASE}/ajo"))
                .header("Authorization", &format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(serde_json::json!({
                    "name": name,
                    "contribution_kobo": kobo,
                    "frequency": "monthly",
                    "member_count": count
                }).to_string())
                .unwrap()
                .send().await;
            fetch_groups();
        });
    };

    view! {
        <div class="space-y-4">
            <h2 class="text-base font-semibold text-gray-800">"Ajo Groups"</h2>

            // Create form
            <Card>
                <p class="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-3">"Start New Ajo"</p>
                <div class="space-y-2">
                    <input placeholder="Group name" class="w-full rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200" on:input=move |e| set_name.set(event_target_value(&e)) />
                    <input type="number" placeholder="Contribution per cycle (₦)" class="w-full rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200" on:input=move |e| set_amount.set(event_target_value(&e)) />
                    <input type="number" placeholder="Number of members" class="w-full rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200" on:input=move |e| set_members.set(event_target_value(&e)) />
                    <Button label="Create Group" on_click=create />
                </div>
            </Card>

            // Groups list
            {move || if loading.get() {
                view! { <Spinner /> }.into_any()
            } else if groups.get().is_empty() {
                view! {
                    <div class="text-center py-10 text-gray-400">
                        <p class="text-3xl mb-2">"🤝"</p>
                        <p class="text-sm">"No ajo groups yet. Start one above!"</p>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="space-y-3">
                        {groups.get().into_iter().map(|g| {
                            let name = g["name"].as_str().unwrap_or("").to_string();
                            let amount = g["contribution_kobo"].as_i64().unwrap_or(0);
                            let cycle = g["current_cycle"].as_u64().unwrap_or(0);
                            let members = g["member_count"].as_u64().unwrap_or(0);
                            view! {
                                <Card>
                                    <div class="flex items-start justify-between">
                                        <div>
                                            <p class="font-semibold text-gray-900">{name}</p>
                                            <p class="text-sm text-gray-500 mt-0.5">{format!("₦{:,.0} · {} members · Cycle {}", amount as f64 / 100.0, members, cycle)}</p>
                                        </div>
                                        <Badge label="Active" color="bg-green-100 text-green-700" />
                                    </div>
                                </Card>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}

// ── Bills Tab ─────────────────────────────────────────────────────────────────

#[component]
fn BillsTab() -> impl IntoView {
    let (title, set_title) = signal(String::new());
    let (total, set_total) = signal(String::new());
    let (phones, set_phones) = signal(String::new());
    let (bills, set_bills) = signal(Vec::<serde_json::Value>::new());
    let (loading, set_loading) = signal(true);

    let fetch_bills = move || {
        leptos::task::spawn_local(async move {
            let token = get_token().unwrap_or_default();
            if let Ok(r) = HttpRequest::get(&format!("{API_BASE}/bills"))
                .header("Authorization", &format!("Bearer {token}"))
                .send().await
            {
                let data: Vec<serde_json::Value> = r.json().await.unwrap_or_default();
                set_bills.set(data);
            }
            set_loading.set(false);
        });
    };

    fetch_bills();

    let create = move || {
        let title = title.get();
        let Ok(kobo) = total.get().parse::<f64>().map(|n| (n * 100.0) as i64) else { return };
        let participant_phones: Vec<String> = phones.get()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        leptos::task::spawn_local(async move {
            let token = get_token().unwrap_or_default();
            let _ = HttpRequest::post(&format!("{API_BASE}/bills"))
                .header("Authorization", &format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(serde_json::json!({
                    "title": title,
                    "total_kobo": kobo,
                    "participant_phones": participant_phones
                }).to_string())
                .unwrap()
                .send().await;
            fetch_bills();
        });
    };

    view! {
        <div class="space-y-4">
            <h2 class="text-base font-semibold text-gray-800">"Split Bills"</h2>

            <Card>
                <p class="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-3">"New Bill"</p>
                <div class="space-y-2">
                    <input placeholder="What's this for? (e.g. Iya Basira's food)" class="w-full rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200" on:input=move |e| set_title.set(event_target_value(&e)) />
                    <input type="number" placeholder="Total amount (₦)" class="w-full rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200" on:input=move |e| set_total.set(event_target_value(&e)) />
                    <input placeholder="Participants' phones (comma separated)" class="w-full rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200" on:input=move |e| set_phones.set(event_target_value(&e)) />
                    <Button label="Split Bill" on_click=create />
                </div>
            </Card>

            {move || if loading.get() {
                view! { <Spinner /> }.into_any()
            } else if bills.get().is_empty() {
                view! {
                    <div class="text-center py-10 text-gray-400">
                        <p class="text-3xl mb-2">"🧾"</p>
                        <p class="text-sm">"No bills yet. Split one above!"</p>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="space-y-3">
                        {bills.get().into_iter().map(|b| {
                            let title = b["title"].as_str().unwrap_or("").to_string();
                            let total = b["total_kobo"].as_i64().unwrap_or(0);
                            let status = b["status"].as_str().unwrap_or("pending").to_string();
                            let (badge_label, badge_color) = match status.as_str() {
                                "settled"        => ("Settled", "bg-green-100 text-green-700"),
                                "partially_paid" => ("Partial", "bg-yellow-100 text-yellow-700"),
                                _                => ("Pending", "bg-gray-100 text-gray-600"),
                            };
                            view! {
                                <Card>
                                    <div class="flex items-start justify-between">
                                        <div>
                                            <p class="font-semibold text-gray-900">{title}</p>
                                            <p class="text-sm text-gray-500 mt-0.5">{format!("Total: ₦{:,.2}", total as f64 / 100.0)}</p>
                                        </div>
                                        <Badge label=badge_label.to_string() color=badge_color.to_string() />
                                    </div>
                                </Card>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}

// ── History Tab ───────────────────────────────────────────────────────────────

#[component]
fn HistoryTab() -> impl IntoView {
    let (txns, set_txns) = signal(Vec::<serde_json::Value>::new());
    let (loading, set_loading) = signal(true);

    leptos::task::spawn_local(async move {
        let token = get_token().unwrap_or_default();
        if let Ok(r) = HttpRequest::get(&format!("{API_BASE}/wallet/transactions"))
            .header("Authorization", &format!("Bearer {token}"))
            .send().await
        {
            let data: Vec<serde_json::Value> = r.json().await.unwrap_or_default();
            set_txns.set(data);
        }
        set_loading.set(false);
    });

    view! {
        <div class="space-y-4">
            <h2 class="text-base font-semibold text-gray-800">"Transaction History"</h2>
            {move || if loading.get() {
                view! { <Spinner /> }.into_any()
            } else if txns.get().is_empty() {
                view! {
                    <div class="text-center py-10 text-gray-400">
                        <p class="text-3xl mb-2">"📋"</p>
                        <p class="text-sm">"No transactions yet."</p>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="space-y-2">
                        {txns.get().into_iter().map(|t| {
                            let desc = t["description"].as_str().unwrap_or("").to_string();
                            let amount = t["amount_kobo"].as_i64().unwrap_or(0);
                            let kind = t["kind"].as_str().unwrap_or("debit");
                            let is_credit = kind == "credit";
                            let sign = if is_credit { "+" } else { "-" };
                            let color = if is_credit { "text-green-600" } else { "text-red-500" };
                            view! {
                                <div class="flex items-center justify-between bg-white rounded-xl px-4 py-3 border border-gray-100">
                                    <div class="flex items-center gap-3">
                                        <div class=format!("w-8 h-8 rounded-full flex items-center justify-center text-sm {}", if is_credit { "bg-green-100" } else { "bg-red-50" })>
                                            {if is_credit { "↓" } else { "↑" }}
                                        </div>
                                        <p class="text-sm text-gray-700">{desc}</p>
                                    </div>
                                    <p class=format!("text-sm font-semibold {color}")>
                                        {format!("{sign}₦{:,.2}", amount as f64 / 100.0)}
                                    </p>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
