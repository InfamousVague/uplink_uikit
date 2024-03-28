use common::{icons, language::get_local_text, state::State};
use dioxus::{
    events::{MouseData, MouseEvent},
    prelude::*,
};
use dioxus_desktop::use_window;
use warp::crypto::DID;

use crate::components::indicator::Indicator;

#[derive(Props, Clone, PartialEq)]
pub struct ItemProps {
    #[props(optional)]
    onpress: Option<EventHandler<MouseEvent>>,
    text: String,
    disabled: Option<bool>,
    #[props(optional)]
    icon: Option<icons::outline::Shape>,
    #[props(optional)]
    danger: Option<bool>,
    should_render: Option<bool>,
    aria_label: Option<String>,
    #[props(optional)]
    children: Option<Element>,
    #[props(optional)]
    tooltip: Option<Element>,
}

/// Tells the parent the menu was interacted with.
pub fn emit(props: ItemProps, e: Event<MouseData>) {
    if let Some(f) = props.onpress.as_ref() {
        f.call(e)
    }
}

#[allow(non_snake_case)]
pub fn ContextItem(props: ItemProps) -> Element {
    let should_render = props.should_render.unwrap_or(true);

    if !should_render {
        return None;
    }

    let class = if props.danger.unwrap_or_default() {
        "context-item danger"
    } else {
        "context-item"
    };

    let disabled: bool = props.disabled.unwrap_or(false);

    let aria_label = props.aria_label.clone().unwrap_or_default();

    let mut tooltip_visible = use_signal(|| false);

    let tooltip_clone = props.tooltip.clone();
    let tooltip_clone2 = props.tooltip.clone();
    let tooltip_clone3 = props.tooltip.clone();
    let tooltip_clone4 = props.tooltip.clone();
    let tooltip_clone5 = props.tooltip.clone();

    if let Some(children) = &props.children {
        rsx!(
            div {
                onmouseenter: move |_| {
                    if tooltip_clone.is_some() {
                         tooltip_visible.set(true);
                    }
                },
                onmouseleave: move |_| {
                    if tooltip_clone2.is_some() {
                         tooltip_visible.set(false);
                    }
                },
                class: "context-item simple-context-item",
                if *tooltip_visible.read() {
                    {tooltip_clone3.as_ref().map(|tooltip| {
                        rsx!(
                           {tooltip}
                        )
                    })}
                }
                {children}
            }
        )
    } else {
        rsx!(
            div {
                onmouseenter: move |_| {
                    if tooltip_clone4.is_some() {
                         tooltip_visible.set(true);
                    }
                },
                onmouseleave: move |_| {
                    if tooltip_clone5.is_some() {
                         tooltip_visible.set(false);
                    }
                },
                button {
                    class: format_args!("{class} {}", if disabled {"context-item-disabled"} else {""}),
                    aria_label: "{aria_label}",
                    onclick: move |e| {
                        if !disabled {
                            emit(props.clone(), e);
                        }
                    },
                    {(props.icon.is_some()).then(|| {
                        let icon = props.icon.unwrap_or(icons::outline::Shape::Cog6Tooth);
                            rsx!{icons::Icon { icon: icon }}
                    })},
                    div {"{props.text}"},
                }
                if *tooltip_visible.read() {
                    {props.tooltip.as_ref().map(|tooltip| {
                        rsx!(
                           {tooltip}
                        )
                    })}
                }
            }
        )
    }
}

#[derive(PartialEq, Props, Clone)]
pub struct IdentityProps {
    sender_did: DID,
    with_status: Option<bool>,
}

#[allow(non_snake_case)]
pub fn IdentityHeader(props: IdentityProps) -> Element {
    let state = use_context::<Signal<State>>();
    let sender = state
        .read()
        .get_identity(&props.sender_did)
        .unwrap_or_default();
    let image = sender.profile_picture();
    let banner = sender.profile_banner();
    let with_status = props.with_status.unwrap_or(true);
    rsx!(
        div {
            class: "identity-header",
            aria_label: "identity-header",
            div {
                id: "banner-image",
                aria_label: "banner-image",
                style: "background-image: url('{banner}');",
                div {
                    id: "profile-image",
                    aria_label: "profile-image",
                    style: "background-image: url('{image}');",
                    {with_status.then(||{
                        rsx!(Indicator {
                            status: sender.identity_status().into(),
                            platform: sender.platform().into(),
                        })
                    })}
                }
            }
        }
    )
}

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    id: String,
    items: Element,
    children: Element,
    #[props(optional)]
    devmode: Option<bool>,
    on_mouseenter: Option<EventHandler<MouseEvent>>,
    left_click_trigger: Option<bool>,
    fit_parent: Option<bool>,
}

#[allow(non_snake_case)]
pub fn ContextMenu(props: Props) -> Element {
    let id = &props.id;
    let window = use_window();

    let devmode = props.devmode.unwrap_or(false);
    let with_click = use_signal(|| props.left_click_trigger.unwrap_or_default());
    let id_signal = use_signal(|| id.clone());

    // Handles the hiding and showing of the context menu
    use_effect(move || {
        let script = include_str!("./context.js")
            .replace("UUID", &id_signal.read())
            .replace("ON_CLICK", &format!("{}", with_click.read()));
        let _ = eval(&script);
    });

    rsx! {
        div {
            class: format_args!("context-wrap {}", if props.fit_parent.unwrap_or_default() {"context-wrap-fit"} else {""}),
            onmouseenter: move |e| {
                if let Some(f) = props.on_mouseenter.as_ref() { f.call(e) }
            },
            div {
                id: "{id}",
                class: "context-inner",
                {&props.children},
            },
            div {
                id: "{id}-context-menu",
                class: "context-menu hidden",
                aria_label: "Context Menu",
                {&props.items},
                {devmode.then(|| rsx!(
                    br {},
                    hr {},
                    br {},
                    ContextItem {
                        icon: icons::outline::Shape::CommandLine,
                        text: get_local_text("uplink.open-devtools"),
                        onpress: move |_| window.webview.open_devtools(),
                        aria_label: "open-devtools-context".to_string(),
                    }
                ))}
            },
        },
    }
}
