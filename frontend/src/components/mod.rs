use leptos::prelude::*;

#[component]
pub fn Button(
    #[prop(into)] label: String,
    #[prop(optional, into)] variant: Option<String>,
    #[prop(optional)] disabled: bool,
    on_click: impl Fn() + 'static,
) -> impl IntoView {
    let base = "inline-flex items-center justify-center rounded-xl px-5 py-2.5 text-sm font-semibold transition-all duration-150 focus:outline-none focus:ring-2 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed";
    let style = match variant.as_deref() {
        Some("outline") => format!("{base} border border-green-600 text-green-700 hover:bg-green-50 focus:ring-green-500"),
        Some("ghost")   => format!("{base} text-gray-600 hover:bg-gray-100 focus:ring-gray-400"),
        Some("danger")  => format!("{base} bg-red-600 text-white hover:bg-red-700 focus:ring-red-500"),
        _               => format!("{base} bg-green-600 text-white hover:bg-green-700 focus:ring-green-500"),
    };
    view! {
        <button class=style disabled=disabled on:click=move |_| on_click()>
            {label}
        </button>
    }
}

#[component]
pub fn Input(
    #[prop(into)] placeholder: String,
    #[prop(into)] value: String,
    #[prop(optional, into)] input_type: Option<String>,
    on_input: impl Fn(String) + 'static,
) -> impl IntoView {
    let t = input_type.unwrap_or_else(|| "text".into());
    view! {
        <input
            type=t
            placeholder=placeholder
            value=value
            class="w-full rounded-xl border border-gray-200 bg-white px-4 py-2.5 text-sm text-gray-900 placeholder-gray-400 shadow-sm focus:border-green-500 focus:outline-none focus:ring-2 focus:ring-green-200 transition"
            on:input=move |e| on_input(event_target_value(&e))
        />
    }
}

#[component]
pub fn Card(children: Children) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-gray-100 bg-white p-5 shadow-sm">
            {children()}
        </div>
    }
}

#[component]
pub fn Badge(#[prop(into)] label: String, #[prop(into)] color: String) -> impl IntoView {
    let cls = format!("inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {color}");
    view! { <span class=cls>{label}</span> }
}

#[component]
pub fn Spinner() -> impl IntoView {
    view! {
        <div class="flex justify-center py-8">
            <div class="h-8 w-8 animate-spin rounded-full border-4 border-green-200 border-t-green-600"></div>
        </div>
    }
}
