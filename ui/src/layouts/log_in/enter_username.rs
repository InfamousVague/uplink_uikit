use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use dioxus::prelude::*;
use dioxus_desktop::{use_window, LogicalSize};
use kit::elements::label::Label;
use kit::elements::{
    button::Button,
    input::{Input, Options, Validation},
};
use tracing::log;

use crate::AuthPages;

pub const MIN_USERNAME_LEN: i32 = 4;
pub const MAX_USERNAME_LEN: i32 = 32;

#[component]
pub fn Layout(cx: Scope, page: UseState<AuthPages>, user_name: UseRef<String>) -> Element {
    log::trace!("rendering enter username layout");
    let window = use_window(cx);

    if !matches!(&*page.current(), AuthPages::Success(_)) {
        window.set_inner_size(LogicalSize {
            width: 500.0,
            height: 250.0,
        });
    }

    //let error = use_state(cx, String::new);
    let button_disabled = use_state(cx, || true);

    let username_validation = Validation {
        // The input should have a maximum length of 32
        max_length: Some(MAX_USERNAME_LEN),
        // The input should have a minimum length of 4
        min_length: Some(MIN_USERNAME_LEN),
        // The input should only contain alphanumeric characters
        alpha_numeric_only: true,
        // The input should not contain any whitespace
        no_whitespace: true,
        // The input component validation is shared - if you need to allow just colons in, set this to true
        ignore_colons: false,
        // The input should allow any special characters
        // if you need special chars, select action to allow or block and pass a vec! with each char necessary, mainly if alpha_numeric_only is true
        special_chars: None,
    };

    cx.render(rsx!(
        div {
            id: "unlock-layout",
            aria_label: "unlock-layout",
            Label {
                text: get_local_text("auth.enter-username")
            },
            div {
                class: "instructions",
                aria_label: "instructions",
                get_local_text("auth.enter-username-subtext")
            },
            Input {
                id: "username-input".to_owned(),
                focus: true,
                is_password: false,
                icon: Icon::Identification,
                aria_label: "username-input".into(),
                disable_onblur: true,
                placeholder: get_local_text("auth.enter-username"),
                options: Options {
                    with_validation: Some(username_validation),
                    with_clear_btn: true,
                    clear_on_submit: false,
                    ..Default::default()
                },
                onchange: |(val, is_valid): (String, bool)| {
                    let should_disable = !is_valid;
                    if *button_disabled.get() != should_disable {
                        button_disabled.set(should_disable);
                    }
                    user_name.set(val);
                },
                onreturn: move |_| {
                    if !*button_disabled.get() {
                        page.set(AuthPages::CopySeedWords);
                    }
                }
            },
            Button {
                text:  get_local_text("unlock.create-account"),
                aria_label: "create-account-button".into(),
                appearance: kit::elements::Appearance::Primary,
                disabled: *button_disabled.get(),
                onpress: move |_| {
                    page.set(AuthPages::CopySeedWords);
                }
            }
        }
    ))
}
