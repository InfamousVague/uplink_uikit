use common::icons;
use dioxus::prelude::*;
use tracing::log;

use crate::elements::{button::Button, Appearance};

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    #[props(optional)]
    with_back_button: Option<bool>,
    #[props(optional)]
    onback: Option<EventHandler>,
    #[props(optional)]
    onclick: Option<EventHandler>,
    #[props(optional)]
    controls: Option<Element>,
    #[props(optional)]
    children: Option<Element>,
}

/// If enabled, it will render the bool
pub fn show_back_button(props: Props) -> bool {
    props.with_back_button.unwrap_or(false)
}

/// Emit the back button event
pub fn emit(props: Props) {
    match &props.onback {
        Some(f) => f.call(()),
        None => {}
    }
}

#[allow(non_snake_case)]
pub fn Topbar(props: Props) -> Element {
    let props_signal = use_signal(|| props.clone());
    log::trace!("rendering topbar");
    rsx!(
        div {
            class: "topbar",
            aria_label: "Topbar",
            {(show_back_button(props_signal.read().clone())).then(|| rsx!(
                Button {
                    aria_label: "back-button".to_string(),
                    icon: icons::outline::Shape::Sidebar,
                    onpress: move |_| emit(props_signal.read().clone()),
                    appearance: Appearance::Secondary
                }
            ))},
            div {
                class: "children",
                onclick: move |_| {
                    if let Some(f) = &props_signal.read().clone().onclick {
                        f.call(())
                    }
                },
                {props.children.as_ref()}
            },
            {props.controls.is_some().then(|| rsx!(
                div {
                    class: "controls",
                    {props.controls.as_ref()}
                }
            ))}
        }
    )
}
