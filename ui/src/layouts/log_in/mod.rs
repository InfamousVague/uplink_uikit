mod copy_seed_words;
mod create_or_recover;
mod enter_seed_words;
mod enter_username;
mod entry_point;
mod recover_account;

use dioxus::prelude::*;
use dioxus_desktop::{use_window, LogicalSize};
use kit::components::topbar_controls::TopbarControls;
use kit::STYLE as UIKIT_STYLES;
use warp::multipass;
pub const APP_STYLE: &str = include_str!("../../compiled_styles.css");

// flows:
// EntryPoint -> login
// EntryPoint -> CreateOrRecover -> EnterSeedWords -> login or fail
// EntryPoint -> CreateOrRecover -> CopySeedWords -> EnterUserName -> login
// serve as a sort of router while the user logs in]
#[allow(clippy::large_enum_variant)]
#[derive(PartialEq, Eq, Clone)]
pub enum AuthPages {
    EntryPoint,
    CreateOrRecover,
    EnterUserName,
    EnterSeedWords,
    CopySeedWords,
    Success(multipass::identity::Identity),
}

/// Guard the app's router with the login flow
#[component]
pub fn AuthGuard(page: Signal<AuthPages>) -> Element {
    log::trace!("rendering auth guard");

    let pin = use_signal(String::new);
    let user_name = use_signal(String::new);

    let desktop = use_window();
    let theme = "";

    // make the window smaller while the user authenticates
    let window = use_window();

    if !matches!(*page.read(), AuthPages::Success(_)) {
        window.set_inner_size(LogicalSize {
            width: 500.0,
            height: 350.0,
        });
    }

    rsx! (
        style { "{UIKIT_STYLES} {APP_STYLE} {theme}" },
        div {
            id: "app-wrap",
            div {
                class: "titlebar disable-select",
                id: "lockscreen-controls",
                div {
                    class: "draggable-topbar",
                    onmousedown: move |_| { desktop.drag(); },
                },
                TopbarControls {},
            },

            match *page.read() {
                AuthPages::EntryPoint => rsx!(entry_point::Layout { page: page.clone(), pin: pin.clone() }),
                AuthPages::EnterUserName => rsx!(enter_username::Layout { page: page.clone(), user_name: user_name.clone() }),
                AuthPages::CreateOrRecover => rsx!(create_or_recover::Layout { page: page.clone() }),
                AuthPages::EnterSeedWords => rsx!(enter_seed_words::Layout { page: page.clone(), pin: pin.clone(), }),
                AuthPages::CopySeedWords => rsx!(copy_seed_words::Layout { page: page.clone(), username: user_name(), pin: pin() }),
                _ => unreachable!("this view should disappear when an account is unlocked or created"),
            }
        }
    )
}
