// This is a dioxus component which will allow browsing of the extension "marketplace".

use common::state::action::ConfigAction;
use common::state::Action;
use common::{icons::outline::Shape as Icon, language::get_local_text, state::State, STATIC_ARGS};
use kit::elements::label::Label;

use crate::components::settings::{ExtensionSetting, SettingSection};
use common::sounds;
use dioxus::prelude::*;
use kit::elements::input::{Input, Options};
use kit::{
    components::nav::{Nav, Route},
    elements::{button::Button, switch::Switch},
};

#[allow(non_snake_case)]
pub fn Settings() -> Element {
    let mut state = use_context::<Signal<State>>();

    rsx! (
        div {
            class: "extensions-settings",
            SettingSection {
                aria_label: "open-extensions-section".to_string(),
                section_label: get_local_text("settings-extensions.open-extensions-folder"),
                section_description: get_local_text("settings-extensions.open-folder-description"),
                Button {
                    icon: Icon::FolderOpen,
                    text: get_local_text("settings-extensions.open-extensions-folder"),
                    aria_label: "open-extensions-folder-button".to_string(),
                    onpress: move |_| {
                        let _ = opener::open(&STATIC_ARGS.extensions_path);
                    }
                },
            },
            SettingSection {
                aria_label: "auto-enable-section".to_string(),
                section_label: get_local_text("settings-extensions.auto-enable"),
                section_description: get_local_text("settings-extensions.auto-enable-description"),
                Switch {
                    active: state.read().configuration.extensions.enable_automatically,
                    onflipped: move |value| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            sounds::Play(sounds::Sounds::Flip);
                        }
                        state.write().mutate(Action::Config(ConfigAction::SetAutoEnableExtensions(value)));
                    },
                }
            },
        }
    )
}

#[allow(non_snake_case)]
pub fn Explore() -> Element {
    rsx! (
        div {
            class: "extensions-explore",
            aria_label: "extensions-explore",
            span {
                class: "banner",
                aria_label: "extensions-explore-banner",
                {get_local_text("settings-extensions.banner")}
            },
            Input {
                placeholder: "Extension name or description.".to_string(),
                // TODO: Pending implementation
                disabled: true,
                aria_label: "extensions-search-input".to_string(),
                icon: Icon::MagnifyingGlass,
                options: Options {
                    with_label: String::from("Search Extensions").into(),
                    with_clear_btn: true,
                    ..Default::default()
                }
            }
        }
    )
}

#[allow(non_snake_case)]
pub fn Installed() -> Element {
    let mut state = use_context::<Signal<State>>();

    let metas: Vec<_> = state
        .read()
        .ui
        .extensions
        .values()
        .map(|(enabled, ext)| (enabled, ext.details().meta.clone()))
        .collect();

    rsx!(if metas.is_empty() {
        {
            rsx!(
                div {
                    class: "extensions-not-installed",
                    aria_label: "extensions-not-installed",
                    Label {
                        text: get_local_text("settings.no-extensions-installed"),
                        aria_label: String::from("extensions-installed-label"),
                    }
                }
            )
        }
    } else {
        {
            rsx!({
                metas.iter().cloned().map(|(enabled, meta)| {
                rsx!(
                    ExtensionSetting {
                        title: meta.pretty_name.to_owned(),
                        author: meta.author.to_owned(),
                        description: meta.description.to_owned(),
                        Switch {
                            active: enabled,
                            onflipped: move |value| {
                                if state.read().configuration.audiovideo.interface_sounds {
                                    sounds::Play(sounds::Sounds::Flip);
                                }

                                state.write().mutate(Action::SetExtensionEnabled(meta.name.to_owned(), value));
                            }
                        }
                    }
                )
            })
            })
        }
    })
}

#[allow(non_snake_case)]
pub fn ExtensionsBrowser() -> Element {
    let routes = vec![
        Route {
            name: get_local_text("settings-extensions.installed"),
            icon: Icon::Check,
            to: "installed",
            ..Default::default()
        },
        Route {
            name: get_local_text("settings-extensions.explore"),
            icon: Icon::Sparkles,
            to: "explore",
            ..Default::default()
        },
        Route {
            name: get_local_text("settings-extensions.settings"),
            icon: Icon::Cog6Tooth,
            to: "settings",
            ..Default::default()
        },
    ];

    let active_route = use_signal(|| "installed");

    rsx!(
        div {
            id: "extensions-browser",
            aria_label: "extensions-browser",
            Nav {
                active: routes[0].to,
                bubble: true,
                routes: routes,
                onnavigate: move |r| {
                    active_route.set(r);
                }
            },
            {(active_route() == "installed").then(|| rsx!(
                Installed {}
            ))},
            {(active_route() == "explore").then(|| rsx!(
                Explore {}
            ))},
            {(active_route() == "settings").then(|| rsx!(
                Settings {}
            ))}
        }
    )
}
