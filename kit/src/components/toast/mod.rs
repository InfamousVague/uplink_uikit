use crate::elements::{button::Button, label::Label, Appearance};

use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;

use dioxus::prelude::*;
use uuid::Uuid;

#[allow(dead_code)]
//TODO: Remove for appearance when it is used
#[derive(Props, Clone, PartialEq)]
pub struct Props {
    id: Uuid,
    on_hover: EventHandler<Uuid>,
    on_close: EventHandler<Uuid>,
    #[props(!optional)]
    icon: Option<Icon>,
    #[props(!optional)]
    with_title: Option<String>,
    #[props(!optional)]
    with_content: Option<String>,
    #[props(!optional)]
    appearance: Option<Appearance>,
    #[props(optional)]
    aria_label: Option<String>,
}

/// Generates the optional icon providing a fallback.
/// If there is no icon provided, the toast should not call this.
pub fn get_icon(props: Props) -> Icon {
    match &props.icon {
        Some(icon) => icon.to_owned(),
        None => Icon::QuestionMarkCircle,
    }
}

#[allow(non_snake_case)]
pub fn Toast(props: Props) -> Element {
    let props_signal = use_signal(|| props.clone());
    let content = props.with_content.clone().unwrap_or_default();
    let title = props.with_title.clone().unwrap_or_default();

    rsx!(
        div {
            class: "toast",
            aria_label: "Toast Notification",
            onmouseover: move |_| props_signal().on_hover.call(props.id),
            {(props.icon.is_some()).then(|| rsx!(
                span {
                    class: "toast-icon",
                    IconElement {
                        icon: get_icon(props_signal())
                    }
                }
            ))},
            div {
                class: "toast-content",
                aria_label: "toast-content",
                Label {
                    text: title,
                    aria_label: "toast-title".to_string(),
                },
                p {
                    "{content}",
                }
            },
            Button {
                icon: Icon::XMark,
                appearance: Appearance::Secondary,
                onpress: move |_| props.on_close.call(props.id),
                aria_label: "close-toast".to_string(),
            }
        }
    )
}
