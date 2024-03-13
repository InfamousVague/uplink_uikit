use dioxus::prelude::*;

use crate::elements::{button::Button, Appearance};
use common::icons;

#[derive(Props)]
pub struct Props<'a> {
    with_back_button: Option<bool>,
    onback: Option<EventHandler<'a>>,
    with_nav: Option<Element>,
    navbar_visible: bool,
    top_children: Option<Element>,
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
pub fn Slimbar<'a>(props: Props<'a>) -> Element {
    rsx!(div {
        class: "slimbar",
        aria_label: "slimbar",
        (show_back_button(&cx)).then(|| rsx!(
            div {
                class: "slimbar-back",
                Button {
                    aria_label: "back-button".into(),
                    icon: icons::outline::Shape::Sidebar,
                    onpress: move |_| emit(&cx),
                    appearance: Appearance::Secondary
                }
            }
        )),
        div {
            class: "slimbar-scroll",
            div {
                class: "slimbar-top",
                props.top_children.clone(),
            }
            div {
                class: "slimbar-inner",
                props.children.clone(),
            }
        }

        props.navbar_visible.then(|| rsx!(div {
            class: "nav-vertical-wrapper",
            props.with_nav.clone(),
        })),
    })
}
