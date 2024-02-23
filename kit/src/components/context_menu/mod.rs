use common::{icons, language::get_local_text, state::State};
use dioxus::{
    core::Event,
    events::{MouseData, MouseEvent},
    prelude::*,
};
use dioxus_desktop::use_window;
use warp::crypto::DID;

use crate::components::indicator::Indicator;

#[derive(Props)]
pub struct ItemProps<'a> {
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
pub fn emit(cx: &Scope<ItemProps>, e: Event<MouseData>) {
    if let Some(f) = props.onpress.as_ref() {
        f.call(e)
    }
}

#[allow(non_snake_case)]
pub fn ContextItem<'a>(props: 'a, ItemProps<'a>) -> Element {
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

    let tooltip_visible = use_state(cx, || false);

    if let Some(children) = &props.children {
        rsx!(
            div {
                onmouseenter: move |_| {
                    if props.tooltip.is_some() {
                         tooltip_visible.set(true);
                    }
                },
                onmouseleave: move |_| {
                    if props.tooltip.is_some() {
                         tooltip_visible.set(false);
                    }
                },
                class: "context-item simple-context-item",
                if *tooltip_visible.current() {
                    props.tooltip.as_ref().map(|tooltip| {
                        rsx!(
                           tooltip
                        )
                    })
                }
                children
            }
        ))
    } else {
        rsx!(
            div {
                onmouseenter: move |_| {
                    if props.tooltip.is_some() {
                         tooltip_visible.set(true);
                    }
                },
                onmouseleave: move |_| {
                    if props.tooltip.is_some() {
                         tooltip_visible.set(false);
                    }
                },
                button {
                    class: format_args!("{class} {}", if disabled {"context-item-disabled"} else {""}),
                    aria_label: "{aria_label}",
                    onclick: move |e| {
                        if !disabled {
                            emit(&cx, e);
                        }
                    },
                    (props.icon.is_some()).then(|| {
                        let icon = props.icon.unwrap_or(icons::outline::Shape::Cog6Tooth);
                        rsx! {
                            icons::Icon { icon: icon }
                        }
                    }),
                    div {"{props.text}"},
                }
                if *tooltip_visible.current() {
                    props.tooltip.as_ref().map(|tooltip| {
                        rsx!(
                           tooltip
                        )
                    })
                }
            }
        ))
    }
}

#[derive(PartialEq, Props)]
pub struct IdentityProps {
    sender_did: DID,
    with_status: Option<bool>,
}

#[allow(non_snake_case)]
pub fn IdentityHeader(props: IdentityProps) -> Element {
    let state = use_shared_state::<State>(cx)?;
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
                    with_status.then(||{
                        rsx!(Indicator {
                            status: sender.identity_status().into(),
                            platform: sender.platform().into(),
                        })
                    })
                }
            }
        }
    ))
}

#[derive(Props)]
pub struct Props<'a> {
    id: String,
    items: Element,
    children: Element,
    #[props(optional)]
    devmode: Option<bool>,
    on_mouseenter: Option<EventHandler<MouseEvent>>,
    left_click_trigger: Option<bool>,
}

#[allow(non_snake_case)]
pub fn ContextMenu<'a>(props: 'a, Props<'a>) -> Element {
    let id = &props.id;
    let window = use_window(cx);

    let devmode = props.devmode.unwrap_or(false);
    let with_click = props.left_click_trigger.unwrap_or_default();

    // Handles the hiding and showing of the context menu
    let eval = use_eval(cx);
    use_effect(cx, (id,), |(id,)| {
        to_owned![eval, with_click];
        async move {
            let script = include_str!("./context.js")
                .replace("UUID", &id)
                .replace("ON_CLICK", &format!("{}", with_click));
            let _ = eval(&script);
        }
    });

    rsx! {
        div {
            class: "context-wrap",
            onmouseenter: |e| {
                if let Some(f) = props.on_mouseenter.as_ref() { f.call(e) }
            },
            div {
                id: "{id}",
                class: "context-inner",
                &props.children,
            },
            div {
                id: "{id}-context-menu",
                class: "context-menu hidden",
                aria_label: "Context Menu",
                &props.items,
                devmode.then(|| rsx!(
                    br {},
                    hr {},
                    br {},
                    ContextItem {
                        icon: icons::outline::Shape::CommandLine,
                        text: get_local_text("uplink.open-devtools"),
                        onpress: move |_| window.webview.open_devtools(),
                        aria_label: "open-devtools-context".into(),
                    }
                ))
            },
        },
    })
}
