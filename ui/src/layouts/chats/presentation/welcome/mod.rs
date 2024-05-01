use common::get_images_dir;
use common::state::{Action, State};
use dioxus::prelude::*;
use dioxus_router::prelude::use_navigator;
use kit::layout::topbar::Topbar;

use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use kit::elements::{button::Button, Appearance};

use crate::UplinkRoute;

#[allow(non_snake_case)]
pub fn Welcome() -> Element {
    let router = use_navigator();
    let mut state = use_context::<Signal<State>>();
    let cta_text = get_local_text("friends.cta-text");
    let image_path = use_image_path();

    rsx! {
        div {
            id: "welcome",
            aria_label: "welcome-screen",
            class: "welcome-screen",
            if state.read().ui.sidebar_hidden && state.read().ui.is_minimal_view() {
                {rsx!(
                    Topbar {
                        with_back_button: true,
                        onback: move |_| {
                            let current = state.read().ui.sidebar_hidden;
                            state.write().mutate(Action::SidebarHidden(!current));
                        },
                    }
                )}
            }
            div {
                class: "welcome-contents",
            img {
                class: "image",
                aria_label: "welcome-image",
                src:"{image_path}"
            },
            p {
                class: "muted",
                "{cta_text}"
            },
            Button {
                icon: Icon::Plus,
                aria_label: "add-friends-button".to_string(),
                text: if state.peek().friends(|f|f.all.is_empty()) {
                        get_local_text("friends.add")
                    } else {
                        get_local_text("friends.go-to-friends")
                    },
                appearance: Appearance::Secondary,
                onpress: move |_| {
                    router.replace(UplinkRoute::FriendsLayout {  });
                }
            },

        }
        }
    }
}

fn use_image_path() -> String {
    let hook_result = use_hook(|| {
        get_images_dir()
            .unwrap_or_default()
            .join("mascot")
            .join("better_with_friends.webp")
            .to_str()
            .map(|x| x.to_string())
            .unwrap_or_default()
    });
    hook_result
}
