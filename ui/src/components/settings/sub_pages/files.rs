use common::language::get_local_text;
use dioxus::prelude::*;
#[allow(unused_imports)]
use kit::elements::{button::Button, switch::Switch};
use tracing::log;

use crate::components::settings::SettingSection;

#[allow(dead_code)]
#[allow(non_snake_case)]
pub fn FilesSettings() -> Element {
    log::trace!("Files settings page rendered.");
    rsx!(
        div {
            id: "settings-files",
            aria_label: "settings-files",
            SettingSection {
                aria_label: "local-sync-section".to_string(),
                section_label: get_local_text("settings-files.local-sync"),
                section_description: get_local_text("settings-files.local-sync-description"),
                Switch {

                }
            },
            /*SettingSection {
                section_label: get_local_text("settings-files.open-sync-folder"),
                section_description: get_local_text("settings-files.open-sync-folder-description"),
                Button {
                    text: get_local_text("settings-files.open-sync-folder"),
                    aria_label: "open-sync-folder-button".to_string(),
                    appearance: kit::elements::Appearance::Secondary,
                    icon: common::icons::outline::Shape::FolderOpen,
                    onpress: move |_| {
                    }
                }
            },*/
        }
    )
}
