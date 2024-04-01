use common::language::get_local_text;
use common::state::{action::ConfigAction, Action, State};
use dioxus::prelude::*;
use kit::elements::switch::Switch;
use tracing::log;

use crate::components::settings::SettingSection;

#[allow(non_snake_case)]
pub fn AccessibilitySettings() -> Element {
    let mut state = use_context::<Signal<State>>();

    log::trace!("Accessibility settings page rendered.");

    rsx!(
        div {
            id: "settings-general",
            aria_label: "settings-accessibility",
            div {
                class: format_args!("{}", if state.read().configuration.general.dyslexia_support {"open-dyslexic-activated"} else {"open-dyslexic"}),
                SettingSection {
                    aria_label: "open-dyslexic-section".to_string(),
                    section_label: get_local_text("settings-accessibility.dyslexia"),
                    section_description: get_local_text("settings-accessibility.dyslexia-description"),
                    Switch {
                        active: state.read().configuration.general.dyslexia_support,
                        onflipped: move |e| {
                            state.write().mutate(Action::Config(ConfigAction::SetDyslexicEnabled(e)));
                        }
                    }
                },
            },
        }
    )
}
