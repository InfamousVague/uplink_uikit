use common::language::get_local_text;
use common::state::{action::ConfigAction, Action, State};
use dioxus::prelude::*;
use kit::elements::switch::Switch;
use warp::logging::tracing::log;

use crate::components::settings::SettingSection;

#[allow(non_snake_case)]
pub fn AccessibilitySettings(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;

    log::debug!("Accessibility settings page rendered.");

    cx.render(rsx!(
        div {
            id: "settings-general",
            aria_label: "settings-general",
            div {
                class: "open-dyslexic",
                SettingSection {
                    section_label: get_local_text("settings-general.dyslexia"),
                    section_description: get_local_text("settings-general.dyslexia-description"),
                    Switch {
                        active: state.read().configuration.general.dyslexia_support,
                        onflipped: move |e| {
                            state.write().mutate(Action::Config(ConfigAction::SetDyslexicEnabled(e)));
                        }
                    }
                },
            },
        }
    ))
}
