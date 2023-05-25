use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;
use common::language::get_local_text;
use dioxus::prelude::*;

#[allow(non_snake_case)]
pub fn Release_Info(cx: Scope) -> Element {
    let pre_release_text = get_local_text("uplink.pre-release");

    cx.render(rsx!(
        div {
            id: "pre-release",
            class : if cfg!(target_os = "macos") {"topbar-item mac-spacer topbar-item-background"}  else {"topbar-item topbar-item-background"},
            aria_label: "pre-release",
            IconElement {
                icon: Icon::Beaker,
            },
            p {
                div {
                    onclick: move |_| {
                        let _ = open::that("https://issues.satellite.im");
                    },
                    "{pre_release_text}"
                }

            }
        },
    ))
}
