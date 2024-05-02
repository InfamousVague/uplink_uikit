use std::path::PathBuf;

use common::notifications::{push_notification, NotificationAction};
use common::warp_runner::{OtherCmd, WarpCmd};
use common::WARP_CMD_CH;
use dioxus::prelude::*;

use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use common::{
    sounds::{self, Sounds},
    state::{action::ConfigAction, notifications::NotificationKind, Action, State},
    STATIC_ARGS,
};
use futures::channel::oneshot;
use futures::StreamExt;
use kit::elements::{button::Button, switch::Switch, Appearance};
use rfd::FileDialog;
use tracing::log;

use crate::{components::settings::SettingSection, logger};

#[allow(non_snake_case)]
pub fn DeveloperSettings() -> Element {
    log::trace!("Developer settings page rendered.");
    let mut state = use_context::<Signal<State>>();

    let ch = use_coroutine(|mut rx: UnboundedReceiver<PathBuf>| {
        //to_owned![];
        async move {
            while let Some(cmd) = rx.next().await {
                let dest = cmd.join("uplink.zip");
                let warp_cmd_tx = WARP_CMD_CH.tx.clone();
                let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                let compress_cmd = OtherCmd::CompressFolder {
                    src: STATIC_ARGS.uplink_path.clone(),
                    dest,
                    rsp: tx,
                };

                if let Err(e) = warp_cmd_tx.send(WarpCmd::Other(compress_cmd)) {
                    log::error!("failed to send warp command: {}", e);
                    continue;
                }

                let res = rx.await.expect("command canceled");
                match res {
                    Ok(_) => {
                        log::debug!("cache export complete");
                    }
                    Err(e) => {
                        log::error!("failed to download cache: {e}");
                    }
                };
            }
        }
    });

    rsx!(
        div {
            id: "settings-developer",
            aria_label: "settings-developer",
            SettingSection {
                aria_label: "experimental-features-section".to_string(),
                section_label: get_local_text("settings-developer.experimental-features"),
                section_description: get_local_text("settings-developer.experimental-features-description"),
                Switch {
                    active: state.peek().configuration.read().developer.experimental_features,
                    onflipped: move |value| {
                        state.write().mutate(Action::Config(ConfigAction::SetExperimentalFeaturesEnabled(value)));
                    },
                }
            },
            SettingSection {
                aria_label: "developer-mode-section".to_string(),
                section_label: get_local_text("settings-developer.developer-mode"),
                section_description: get_local_text("settings-developer.developer-mode-description"),
                Switch {
                    active: state.peek().configuration.read().developer.developer_mode,
                    onflipped: move |value| {
                        if state.peek().configuration.peek().audiovideo.interface_sounds {
                            sounds::Play(sounds::Sounds::Flip);
                        }

                        state.write().mutate(Action::Config(ConfigAction::SetDevModeEnabled(value)));
                    },
                }
            },
            SettingSection {
                aria_label: "test-notification-section".to_string(),
                section_label: get_local_text("settings-developer.test-notification"),
                section_description: get_local_text("settings-developer.test-notification-description"),
                Button {
                    text: get_local_text("settings-developer.test-notification"),
                    aria_label: "test-notification-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::BellAlert,
                    onpress: move |_| {
                        push_notification(
                            get_local_text("settings-developer.test-popup"),
                            get_local_text("settings-developer.test-popup"),
                            Some(Sounds::General),
                            notify_rust::Timeout::Milliseconds(4),
                            NotificationAction::Dummy
                        );
                        state
                            .write()
                            .mutate(Action::AddNotification(NotificationKind::Settings, 1, false));
                    }
                }
            },
            SettingSection {
                aria_label: "open-cache-section".to_string(),
                section_label: get_local_text("settings-developer.open-cache"),
                section_description: get_local_text("settings-developer.open-cache-description"),
                Button {
                    text: get_local_text("settings-developer.open-cache-folder"),
                    aria_label: "open-cache-folder-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::FolderOpen,
                    onpress: move |_| {
                        let _ = opener::open(&STATIC_ARGS.uplink_path);
                    }
                }
            },
            SettingSection {
                aria_label: "compress-download-cache-section".to_string(),
                section_label: get_local_text("settings-developer.compress-download-cache"),
                section_description: get_local_text("settings-developer.compress-download-cache-description"),
                Button {
                    text: get_local_text("settings-developer.compress"),
                    aria_label: "compress-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::ArchiveBoxArrowDown,
                    onpress: move |_| {
                        if let Some(path) =  FileDialog::new().set_directory(dirs::home_dir().unwrap_or(".".into())).pick_folder() {
                            ch.send(path);
                        };
                    }
                }
            },
            SettingSection {
                aria_label: "print-state-section".to_string(),
                section_label: get_local_text("settings-developer.print-state"),
                section_description: get_local_text("settings-developer.print-state-description"),
                Button {
                    text: get_local_text("settings-developer.print-state"),
                    aria_label: "print-state-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::DocumentChartBar,
                    onpress: move |_| {
                        log::debug!("{:#?}", &*state.read());
                    }
                }
            },
            SettingSection {
                aria_label: "clear-cache-section".to_string(),
                section_label: get_local_text("settings-developer.clear-cache"),
                section_description: get_local_text("settings-developer.clear-cache-description"),
                Button {
                    text: get_local_text("settings-developer.clear"),
                    aria_label: "clear-button".to_string(),
                    appearance: Appearance::Danger,
                    icon: Icon::Trash,
                    onpress: move |_| {
                        state.write().clear();
                    }
                }
            }
            SettingSection {
                aria_label: "save-logs-section".to_string(),
                section_label: get_local_text("settings-developer.save-logs-to-file"),
                section_description: get_local_text("settings-developer.save-logs-to-file-description"),
                Switch {
                    active: logger::get_save_to_file(),
                    onflipped: move |value| {
                        if state.peek().configuration.peek().audiovideo.interface_sounds {
                            sounds::Play(sounds::Sounds::Flip);
                        }
                        logger::set_save_to_file(value);
                    },
                }
            }
        }
    )
}
