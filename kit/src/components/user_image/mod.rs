use dioxus::{
    events::{MouseData, MouseEvent},
    prelude::*,
};

use crate::{
    components::indicator::{Indicator, Platform, Status},
    elements::label::Label,
};

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    #[props(optional)]
    loading: Option<bool>,
    #[props(optional)]
    image: Option<String>,
    #[props(optional)]
    typing: Option<bool>,
    #[props(optional)]
    with_username: Option<String>,
    #[props(optional)]
    on_press: Option<EventHandler<MouseEvent>>,
    #[props(optional)]
    oncontextmenu: Option<EventHandler<MouseEvent>>,
    status: Option<Status>,
    platform: Platform,
}

pub fn get_image(props: Props) -> String {
    props.image.clone().unwrap_or_default()
}

/// Tells the parent the user_image was interacted with.
pub fn emit(props: Props, e: Event<MouseData>) {
    match &props.on_press {
        Some(f) => f.call(e),
        None => {}
    }
}

pub fn emit_context(props: Props, e: Event<MouseData>) {
    match &props.oncontextmenu {
        Some(f) => f.call(e),
        None => {}
    }
}

#[allow(non_snake_case)]
pub fn UserImage(props: Props) -> Element {
    let props_signal = use_signal(|| props.clone());
    let image_data: String = get_image(props.clone());
    let status = props.status;
    let platform = props.platform;
    let typing = props.typing.unwrap_or_default();
    let username = props.with_username.clone().unwrap_or_default();
    let pressable = props.on_press.is_some();

    let loading = props.loading.unwrap_or_default();

    rsx!(if loading {
        {
            rsx!(UserImageLoading {})
        }
    } else {
        {
            rsx!(
                div {
                    class: {
                        format_args!("user-image-wrap {} {}", if pressable { "pressable" } else { "" },
                        if props.oncontextmenu.is_some() {"has-context-handler"} else {""})
                    },
                    aria_label: "user-image-wrap",
                    onclick: move |e| emit(props_signal(), e),
                    oncontextmenu: move |e| emit_context(props_signal(), e),
                    div {
                        class: "user-image",
                        aria_label: "User Image",
                        div {
                            class: "image",
                            aria_label: "user-image-profile",
                            style: "background-image: url('{image_data}');"
                        },
                        {typing.then(|| rsx!(
                            div {
                                class: "profile-typing",
                                aria_label: "profile-typing",
                                div { class: "dot dot-1" },
                                div { class: "dot dot-2" },
                                div { class: "dot dot-3" }
                            }
                        ))}
                        {status.map(|s| {
                            rsx!(Indicator {
                                status: s,
                                platform: platform,
                            })
                        })}
                    },
                    {(props.with_username.is_some()).then(|| rsx!(
                        Label {
                            text: username
                        }
                    ))}
                }
            )
        }
    })
}

#[allow(non_snake_case)]
pub fn UserImageLoading() -> Element {
    rsx!(div {
        class: "skeletal user-image-loading"
    })
}
