#[allow(unused_imports)]
use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use common::sounds;
use common::state::{action::ConfigAction, Action, State};
use dioxus::prelude::*;
#[allow(unused_imports)]
use kit::elements::{button::Button, switch::Switch};

use crate::components::settings::SettingSection;

#[allow(non_snake_case)]
pub fn NotificationSettings() -> Element {
    let mut state = use_context::<Signal<State>>();

    rsx!(
        div {
            id: "settings-notifications",
            aria_label: "settings-notifications",
            /*SettingSection {
                section_label: get_local_text("settings-notifications.grant-permissions"),
                section_description: get_local_text("settings-notifications.grant-permissions-description"),
                Button {
                    aria_label: "grant-permissions-button".to_string(),
                    text: get_local_text("settings-notifications.grant-permissions"),
                    icon: Icon::Shield,
                    onpress: move |_| {
                        // TODO: Grant permissions this should prompt the user to grant permissions for their system
                    }
                }
            },*/
            SettingSection {
                aria_label: "enabled-notifications-section".to_string(),
                section_label: get_local_text("settings-notifications.enabled"),
                section_description: get_local_text("settings-notifications.enabled-description"),
                Switch {
                    active: state.read().configuration.notifications.enabled,
                    onflipped: move |e| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            sounds::Play(sounds::Sounds::Flip);
                        }
                        state.write().mutate(Action::Config(ConfigAction::SetNotificationsEnabled(e)));
                    }
                }
            },
            div {
                class: format_args!("{}", if state.read().configuration.notifications.enabled { "enabled" } else { "disabled" }),
                SettingSection {
                    aria_label: "friends-notifications-section".to_string(),
                    section_label: get_local_text("friends"),
                    section_description: get_local_text("settings-notifications.friends-description"),
                    Switch {
                        active: state.read().configuration.notifications.enabled && state.read().configuration.notifications.friends_notifications,
                        disabled: !state.read().configuration.notifications.enabled,
                        onflipped: move |e| {
                            if state.read().configuration.audiovideo.interface_sounds {
                               sounds::Play(sounds::Sounds::Flip);
                            }
                            state.write().mutate(Action::Config(ConfigAction::SetFriendsNotificationsEnabled(e)));
                        }
                    }
                },
                SettingSection {
                    aria_label: "messages-notifications-section".to_string(),
                    section_label: get_local_text("messages"),
                    section_description: get_local_text("settings-notifications.messages-description"),
                    Switch {
                        active: state.read().configuration.notifications.enabled && state.read().configuration.notifications.messages_notifications,
                        disabled: !state.read().configuration.notifications.enabled,
                        onflipped: move |e| {
                            if state.read().configuration.audiovideo.interface_sounds {
                                sounds::Play(sounds::Sounds::Flip);
                            }
                            state.write().mutate(Action::Config(ConfigAction::SetMessagesNotificationsEnabled(e)));
                        }
                    }
                },
                SettingSection {
                    aria_label: "settings-notifications-section".to_string(),
                    section_label: get_local_text("settings"),
                    section_description: get_local_text("settings-notifications.settings-description"),
                    Switch {
                        active: state.read().configuration.notifications.enabled && state.read().configuration.notifications.settings_notifications,
                        disabled: !state.read().configuration.notifications.enabled,
                        onflipped: move |e| {
                            if state.read().configuration.audiovideo.interface_sounds {
                                sounds::Play(sounds::Sounds::Flip);
                            }
                            state.write().mutate(Action::Config(ConfigAction::SetSettingsNotificationsEnabled(e)));
                        }
                    }
                },
            }
        }
    )
}
