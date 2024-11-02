use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thaw::*;

#[component]
pub fn InfoCard(
    #[prop(optional, into)]
    icon: String,
    #[prop(into)]
    title: String,
    #[prop(into)]
    link_text: String,
    #[prop(into)]
    link_href: String,
    children: Children,
) -> impl IntoView {
    let title = store_value(title);

    let icon_view = if icon.is_empty() { view! {}.into_view() } else { view! {
        <img style="height: 2em;" class="inline-block mr-4" src={icon} alt={move || title.get_value()} />
    }.into_view() };

    view! {
        <div class="w-xs shadow-lg bg-white border-2 border-gray-200 rounded-2xl p-6 text-left flex flex-col justify-between">
            <div>
                <div class="font-bold text-xl inline-flex flex-row items-center">
                    {icon_view}
                    <span>{move || title.get_value()}</span>
                </div>
                
                <div class="text-gray-700 text-base my-4">{children()}</div>
            </div>
            <div class="text-right">
                <A href={link_href} class="fk-button">
                    {link_text}
                </A>
            </div>
        </div>
    }
}
