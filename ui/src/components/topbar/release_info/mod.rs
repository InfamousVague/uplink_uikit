use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;
use common::language::get_local_text;
use dioxus::prelude::*;

#[allow(non_snake_case)]
pub fn Release_Info() -> Element {
    let alpha_text = get_local_text("uplink.alpha");

    rsx!(
        div {
            id: "alpha",
            class : if cfg!(target_os = "macos") {"topbar-item mac-spacer"}  else {"topbar-item"},
            aria_label: "alpha",
            IconElement {
                icon: Icon::Beaker,
            },
            p {
                div {
                    onclick: move |_| {
                        let _ = open::that("https://issues.satellite.im");
                    },
                    "{alpha_text}"
                }

            }
        },
    )
}
