use crate::elements::{button::Button, Appearance};
use common::{icons::outline::Shape as Icon, state::State};
use dioxus::prelude::*;
use dioxus_desktop::{use_window, LogicalSize};

#[allow(non_snake_case)]
pub fn TopbarControls() -> Element {
    let state = use_signal(State::load);
    let desktop = use_window();
    let desktop_signal = use_signal(|| desktop.clone());
    let first_resize = use_signal(|| true);
    if cfg!(not(target_os = "macos")) {
        rsx!(
            div {
                class: "controls",
                Button {
                    aria_label: String::from("minimize-button"),
                    icon: Icon::Minus,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop_signal().set_minimized(true);
                    }
                },
                Button {
                    aria_label: String::from("square-button"),
                    icon: Icon::Square2Stack,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop_signal().set_maximized(!desktop.is_maximized());
                        if state.read().ui.window_maximized
                            && *first_resize.read()
                            && cfg!(target_os = "windows")
                        {
                            desktop_signal().set_inner_size(LogicalSize::new(950.0, 600.0));
                            *first_resize.write_silent() = false;
                        }
                    }
                },
                Button {
                    aria_label: String::from("close-button"),
                    icon: Icon::XMark,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop_signal().close();
                    }
                },
            }
        )
    } else {
        rsx!({})
    }
}
