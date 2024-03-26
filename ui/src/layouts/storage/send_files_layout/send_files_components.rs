use common::language::get_local_text_with_args;
use common::MAX_FILES_PER_MESSAGE;
use common::{icons::outline::Shape as Icon, language::get_local_text};
use dioxus::prelude::*;
use dioxus_router::prelude::use_navigator;
use kit::elements::{button::Button, checkbox::Checkbox, Appearance};
use warp::raygun::Location;

use crate::{layouts::storage::files_layout::controller::StorageController, UplinkRoute};

use super::SendFilesStartLocation;

#[component]
pub fn FileCheckbox(
    file_path: String,
    storage_controller: Signal<StorageController>,
    is_selecting_files: bool,
) -> Element {
    if *is_selecting_files {
        let files_selected_to_send = storage_controller.with(|f| f.files_selected_to_send.clone());
        return rsx!( div {
            class: "checkbox-position",
            Checkbox {
                disabled: files_selected_to_send.len() >= MAX_FILES_PER_MESSAGE,
                is_checked: files_selected_to_send.iter()
                .any(|location| {
                    match location {
                        Location::Constellation { path } => path.clone() == file_path,
                        Location::Disk { .. } => false,
                    }
                }),
                on_click: move |_| {}
            }
        });
    }
    None
}

#[component]
pub fn SendFilesTopbar(
    send_files_from_storage_state: Signal<bool>,
    send_files_start_location: SendFilesStartLocation,
    storage_controller: Signal<StorageController>,
    on_send: EventHandler<Vec<Location>>,
    in_files: bool,
) -> Element {
    let router = use_navigator();

    return rsx! (
        div {
            class: "send-files-button",
            Button {
                text: get_local_text("files.go-to-files"),
                icon: Icon::FolderPlus,
                aria_label: "go_to_files_btn".into(),
                appearance: Appearance::Secondary,
                onpress: move |_| {
                    if send_files_start_location.eq(&SendFilesStartLocation::Storage) {
                        send_files_from_storage_state.set(false);
                    } else {
                        router.replace(UplinkRoute::FilesLayout {});
                    }
                },
            },
            Button {
                text: get_local_text_with_args("files.send-files-text-amount", vec![("amount", format!("{}/{}", storage_controller.with(|f| f.files_selected_to_send.clone()).len(), MAX_FILES_PER_MESSAGE))]),
                aria_label: "send_files_modal_send_button".into(),
                disabled: storage_controller.with(|f| f.files_selected_to_send.is_empty() || (in_files && f.chats_selected_to_send.is_empty())),
                appearance: Appearance::Primary,
                icon: Icon::ChevronRight,
                onpress: move |_| {
                    on_send.call(storage_controller.with(|f| f.files_selected_to_send.clone()));
                }
            },
        }
    );
}

pub fn toggle_selected_file(mut storage_controller: Signal<StorageController>, file_path: String) {
    if let Some(index) = storage_controller.with(|f| {
        f.files_selected_to_send
            .iter()
            .position(|location| match location {
                Location::Constellation { path } => path.eq(&file_path),
                _ => false,
            })
    }) {
        storage_controller.with_mut(|f| f.files_selected_to_send.remove(index));
    } else if storage_controller.with(|f| f.files_selected_to_send.len() < MAX_FILES_PER_MESSAGE) {
        storage_controller.with_mut(|f| {
            f.files_selected_to_send.push(Location::Constellation {
                path: file_path.clone(),
            })
        });
    }
}
