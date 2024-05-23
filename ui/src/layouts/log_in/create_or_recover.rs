use common::{language::get_local_text, state::State};
use dioxus::prelude::*;
use dioxus_desktop::use_window;
use kit::elements::{button::Button, label::Label};

use crate::{get_app_style, layouts::log_in::update_window_size};

use super::AuthPages;

// styles for this layout are in layouts/style.scss
#[component]
pub fn Layout(cx: Scope, page: UseState<AuthPages>) -> Element {
    let state = use_ref(cx, State::load);
    let window = use_window(cx);

    if !matches!(&*page.current(), AuthPages::Success(_)) {
        update_window_size(window, 500.0, 250.0);
    }
    cx.render(rsx!(
        style {get_app_style(&state.read())},
        div {
            id: "create-or-recover-layout",
            aria_label: "create-or-recover-layout",
            Label {
                aria_label: "create-or-recover".into(),
                text: get_local_text("create-or-recover")
            }
            div {
                class: "instructions",
                aria_label: "create-or-recover-instructions",
                get_local_text("create-or-recover.instructions")
            },
            div {
                class: "button-container",
                Button {
                    aria_label: "create-button".into(),
                    text: get_local_text("create-or-recover.create"),
                    onpress: move |_| {
                        page.set(AuthPages::EnterUserName);
                    }
                },
                Button {
                    aria_label: "recover-button".into(),
                    text: get_local_text("create-or-recover.recover"),
                    onpress: move |_| {
                        page.set(AuthPages::EnterSeedWords);
                    }
                },
            }
        }
    ))
}
