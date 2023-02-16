use dioxus::prelude::*;
use kit::elements::switch::Switch;
use shared::language::get_local_text;
use warp::logging::tracing::log;

use crate::{
    components::settings::SettingSection,
    state::{action::ConfigAction, Action, State},
};

#[allow(non_snake_case)]
pub fn AudioSettings(cx: Scope) -> Element {
    log::debug!("Audio settings page rendered.");
    let state = use_shared_state::<State>(cx)?;

    cx.render(rsx!(
        div {
            id: "settings-audio",
            aria_label: "settings-audio",
            SettingSection {
                section_label: get_local_text("settings-audio.interface-sounds"),
                section_description: get_local_text("settings-audio.interface-sounds-description"),
                Switch {
                    active: state.read().configuration.audiovideo.interface_sounds,
                    onflipped: move |e| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            crate::utils::sounds::Play(crate::utils::sounds::Sounds::Flip);
                        }
                        state.write().mutate(Action::Config(ConfigAction::InterfaceSoundsEnabled(e)));
                    }
                }
            },
            SettingSection {
                section_label: get_local_text("settings-audio.media-sounds"),
                section_description: get_local_text("settings-audio.media-sounds-description"),
                Switch {
                    active: state.read().configuration.audiovideo.media_sounds,
                    onflipped: move |e| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            crate::utils::sounds::Play(crate::utils::sounds::Sounds::Flip);
                        }
                        state.write().mutate(Action::Config(ConfigAction::MediaSoundsEnabled(e)));
                    }
                }
            },
            SettingSection {
                section_label: get_local_text("settings-audio.message-sounds"),
                section_description: get_local_text("settings-audio.message-sounds-description"),
                Switch {
                    active: state.read().configuration.audiovideo.message_sounds,
                    onflipped: move |e| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            crate::utils::sounds::Play(crate::utils::sounds::Sounds::Flip);
                        }
                        state.write().mutate(Action::Config(ConfigAction::MessageSoundsEnabled(e)));
                    }
                }
            },
            SettingSection {
                section_label: get_local_text("settings-audio.call-timer"),
                section_description: get_local_text("settings-audio.call-timer-description"),
                Switch {}
            }
        }
    ))
}
