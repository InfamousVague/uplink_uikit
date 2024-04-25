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

#[allow(non_snake_case)]
pub fn Topbar(props: Props) -> Element {
    log::trace!("rendering topbar");
    rsx!(
        div {
            class: "topbar",
            aria_label: "Topbar",
            {
                (props.with_back_button.unwrap_or(false)).then(|| rsx!(
                Button {
                    aria_label: "back-button".to_string(),
                    icon: icons::outline::Shape::Sidebar,
                    onpress: move |_| match &props.onback {
                        Some(f) => f.call(()),
                        None => {}
                    },
                    appearance: Appearance::Secondary
                }))
            },
            div {
                class: "children",
                onclick: move |_| {
                    if let Some(f) = &props.onclick {
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
