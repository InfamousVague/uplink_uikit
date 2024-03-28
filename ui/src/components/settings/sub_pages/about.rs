use std::process::Command;

use common::get_images_dir;
use common::language::get_local_text;
use common::state::{Action, ToastNotification};
use common::{icons::outline::Shape as Icon, state::State};
use dioxus::prelude::*;
use dioxus_desktop::use_window;
use futures::StreamExt;
use kit::elements::{button::Button, Appearance};

use tracing::log;

use crate::get_download_modal;
use crate::utils::auto_updater::{DownloadProgress, DownloadState, SoftwareDownloadCmd};
use crate::{
    components::settings::SettingSection,
    utils::{self, auto_updater::GitHubRelease},
};

#[allow(non_snake_case)]
pub fn AboutPage() -> Element {
    let version = env!("CARGO_PKG_VERSION");
    let app_name = env!("CARGO_PKG_NAME");
    let mut state = use_context::<Signal<State>>();
    let download_state = use_context::<Signal<DownloadState>>();
    let update_button_loading = use_signal(|| false);
    let download_available: Signal<Option<GitHubRelease>> = use_signal(|| None);
    let desktop = use_window();

    let click_count = use_signal(|| 0);

    let ch = use_coroutine(|mut rx: UnboundedReceiver<()>| {
        to_owned![download_available, update_button_loading, state];
        async move {
            while rx.next().await.is_some() {
                match utils::auto_updater::check_for_release().await {
                    Ok(opt) => {
                        if opt.is_none() {
                            state.write().mutate(Action::AddToastNotification(
                                ToastNotification::init(
                                    "".into(),
                                    get_local_text("settings-about.no-update-available"),
                                    None,
                                    2,
                                ),
                            ))
                        }
                        download_available.set(opt);
                    }
                    Err(e) => {
                        let opt_err: Option<&reqwest::Error> = e.downcast_ref();
                        let msg = match opt_err {
                            Some(e) => {
                                // Most common ones. Else display a generic error message
                                if e.is_timeout() {
                                    "settings-about.update-check-error-timeout"
                                } else if e.is_request() {
                                    "settings-about.update-check-error-request"
                                } else {
                                    "settings-about.update-check-error"
                                }
                            }
                            None => "settings-about.update-check-error",
                        };
                        state.write().mutate(Action::AddToastNotification(
                            ToastNotification::init("".into(), get_local_text(msg), None, 4),
                        ));
                        log::error!("failed to check for updates: {e}");
                    }
                }
                update_button_loading.set(false);
            }
        }
    });

    let _download_ch = use_coroutine_handle::<SoftwareDownloadCmd>();

    let opt = download_available();
    let stage = download_state.read().stage;
    let pending_key = format!("btn-pending{}", download_state.read().progress);

    let image_path_flag_AU = get_flag_image_path("AU");
    let image_path_flag_USA = get_flag_image_path("USA");
    let image_path_flag_MX = get_flag_image_path("MX");
    let image_path_flag_DE = get_flag_image_path("DE");
    let image_path_flag_PT = get_flag_image_path("PT");
    let image_path_flag_BR = get_flag_image_path("BR");
    let image_path_flag_IT = get_flag_image_path("IT");
    let image_path_flag_UR = get_flag_image_path("UR");
    let image_path_flag_BL = get_flag_image_path("BL");
    let image_path_flag_JP = get_flag_image_path("JP");
    let image_path_flag_IN = get_flag_image_path("IN");

    fn get_flag_image_path(flag: &str) -> String {
        get_images_dir()
            .unwrap_or_default()
            .join("flags")
            .join(format!("{}-flag.png", flag))
            .to_str()
            .map(|x| x.to_string())
            .unwrap_or_default()
    }

    let about_button = rsx!(match opt {
        None if stage == DownloadProgress::Idle => {
            {
                let btn_start = "btn-start".to_string();
                rsx!(Button {
                    key: "{btn_start}",
                    text: get_local_text("uplink.check-for-updates"),
                    loading: update_button_loading(),
                    aria_label: "check-for-updates-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::ArrowPath,
                    onpress: move |_| {
                        download_available.set(None);
                        update_button_loading.set(true);
                        ch.send(());
                    }
                })
            }
        }
        _ => match stage {
            DownloadProgress::Idle => {
                let btn_idle = "btn-idle".to_string();
                rsx!(Button {
                    key: "{btn_idle}",
                    text: get_local_text("uplink.download-update"),
                    loading: update_button_loading(),
                    aria_label: "check-for-updates-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::ArrowDown,
                    onpress: move |_| {
                        download_state.write().stage = DownloadProgress::PickFolder;
                    }
                })
            }
            DownloadProgress::PickFolder => rsx!(get_download_modal {
                on_dismiss: move |_| {
                    download_state.write().stage = DownloadProgress::Idle;
                },
                // is never used
                // on_submit: move |dest: PathBuf| {
                //     download_state.write().stage = DownloadProgress::Pending;
                //     download_state.write().destination = Some(dest.clone());
                //     download_ch.send(SoftwareDownloadCmd(dest));
                // }
            }),
            DownloadProgress::_Pending => {
                rsx!(Button {
                    key: "{pending_key}",
                    text: format!("{}%", download_state.read().progress as u32),
                    loading: true,
                    aria_label: "check-for-updates-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::ArrowDown,
                })
            }
            DownloadProgress::Finished => {
                let btn_finished = "btn-finished".to_string();
                rsx!(Button {
                    key: "{btn_finished}",
                    text: get_local_text("uplink.update-menu-install"),
                    loading: update_button_loading(),
                    aria_label: "check-for-updates-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::ArrowDown,
                    onpress: move |_| {
                        // be sure to update this before closing the app
                        state.write().mutate(Action::DismissUpdate);
                        if let Some(dest) = download_state.read().destination.clone() {
                            std::thread::spawn(move || {
                                let cmd = if cfg!(target_os = "windows") {
                                    "explorer"
                                } else if cfg!(target_os = "linux") {
                                    "xdg-open"
                                } else if cfg!(target_os = "macos") {
                                    "open"
                                } else {
                                    eprintln!("unknown OS type. failed to open files browser");
                                    return;
                                };
                                Command::new(cmd).arg(dest).spawn().unwrap();
                            });
                            desktop.close();
                        } else {
                            log::error!("attempted to download update without download location");
                        }
                        download_state.write().destination = None;
                        download_state.write().stage = DownloadProgress::Idle;
                    }
                })
            }
        },
    });

    rsx!(
        div {
            id: "settings-about",
            SettingSection {
                aria_label: "about-info-section".to_string(),
                section_label: get_local_text("settings-about.info"),
                section_description: app_name.to_string(),
            },
            div {
                width: "100%",
                onclick: move |_| {
                    if click_count() < 9 {
                        click_count.with_mut(|x| *x += 1);
                    } else {
                        click_count.set(0);
                        if !state.read().ui.show_dev_settings {
                             state.write().mutate(Action::SetDevSettings(true));
                        }
                    }
                },
                SettingSection {
                    aria_label: "about-version-section".to_string(),
                    section_label:  get_local_text("settings-about.version"),
                    section_description: version.to_string(),
                    div {
                        {about_button}
                    }
                },
            }
            SettingSection {
                aria_label: "open-website-section".to_string(),
                section_label: get_local_text("settings-about.open-website"),
                section_description: get_local_text("settings-about.open-website-description"),
                Button {
                    text: get_local_text("settings-about.open-website"),
                    aria_label: "open-website-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::GlobeAlt,
                    onpress: move |_| {
                        let _ = open::that("https://satellite.im");
                    }
                }
            },
            SettingSection {
                aria_label: "open-codebase-section".to_string(),
                section_label: get_local_text("settings-about.open-codebase"),
                section_description: get_local_text("settings-about.open-codebase-description"),
                Button {
                    text: get_local_text("settings-about.open-codebase"),
                    aria_label: "open-codebase-button".to_string(),
                    appearance: Appearance::Secondary,
                    icon: Icon::CodeBracketSquare,
                    onpress: move |_| {
                        let _ = open::that("https://github.com/Satellite-im/Uplink");
                    }
                }
            },
            SettingSection {
                aria_label: "made-in-section".to_string(),
                section_label: get_local_text("settings-about.made-in"),
                section_description: get_local_text("settings-about.team"),
                div {
                    class: "flags-settings",
                    img {
                        src: "{image_path_flag_USA}",
                        aria_label: "usa-flag",
                        alt: "USA Flag",
                    },
                    img {
                        src: "{image_path_flag_MX}",
                        aria_label: "mx-flag",
                        alt: "Mexico Flag",
                    }
                    img {
                        src: "{image_path_flag_DE}",
                        aria_label: "de-flag",
                        alt: "Germany Flag",
                    }
                    img {
                        src: "{image_path_flag_PT}",
                        aria_label: "pt-flag",
                        alt: "Portugal Flag",
                    }
                    img {
                        src: "{image_path_flag_BR}",
                        aria_label: "br-flag",
                        alt: "Brazil Flag",
                    }
                    img {
                        src: "{image_path_flag_IT}",
                        aria_label: "it-flag",
                        alt: "Italy Flag",
                    }
                    img {
                        src: "{image_path_flag_UR}",
                        aria_label: "ur-flag",
                        alt: "Ukraine Flag",
                    }
                    img {
                        src: "{image_path_flag_BL}",
                        aria_label: "bl-flag",
                        alt: "Belarus Flag",
                    }
                    img {
                        src: "{image_path_flag_JP}",
                        aria_label: "jp-flag",
                        alt: "Japan Flag",
                    }
                    img {
                        src: "{image_path_flag_AU}",
                        aria_label: "au-flag",
                        alt: "Australia Flag",
                    }
                    img {
                        src: "{image_path_flag_IN}",
                        aria_label: "in-flag",
                        alt: "Indonesia Flag",
                    }
                }
            }
        }
    )
}
