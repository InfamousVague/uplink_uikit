use common::{
    language::get_local_text,
    state::{Action, State},
};
use dioxus::prelude::*;
use kit::elements::switch::Switch;
use tracing::log;

use crate::components::settings::SettingSection;

#[allow(non_snake_case)]
pub fn Messages() -> Element {
    log::trace!("Messages settings page rendered.");
    let mut state = use_context::<Signal<State>>();
    rsx!(
        div {
            id: "settings-messages",
            aria_label: "settings-messages",
             SettingSection {
                 aria_label: "emoji-conversion-section".to_string(),
                 section_label: get_local_text("settings-messages.emoji-conversion"),
                 section_description: get_local_text("settings-messages.emoji-conversion-description"),
                 Switch {
                    active: state.read().ui.should_transform_ascii_emojis(),
                    onflipped: move|flag| {
                        state.write().mutate(Action::SetTransformAsciiEmojis(flag));
                    }
                 }
             },
            SettingSection {
                aria_label: "markdown-support-section".to_string(),
                section_label: get_local_text("settings-messages.markdown-support"),
                section_description: get_local_text("settings-messages.markdown-support-description"),
                Switch {
                    active: state.read().ui.should_transform_markdown_text(),
                    onflipped: move|flag| {
                        state.write().mutate(Action::SetTransformMarkdownText(flag));
                    }
                }
            }
        }
    )
}
