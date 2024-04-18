use crate::layouts::storage::functions::{self, download_file, ChanCmd};
use crate::layouts::storage::send_files_layout::send_files_components::{
    toggle_selected_file, FileCheckbox,
};

use super::files_layout::controller::StorageController;
use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;
use common::is_file_available_to_preview;
use common::state::{State, ToastNotification};
use common::warp_runner::thumbnail_to_base64;
use common::{language::get_local_text, ROOT_DIR_NAME};

use dioxus::html::input_data::keyboard_types::Code;
use dioxus::prelude::*;
use kit::components::context_menu::{ContextItem, ContextMenu};
use kit::elements::file::File;
use kit::elements::folder::Folder;
use warp::constellation::item::Item;
use warp::raygun::Location;

#[derive(Props, Clone, PartialEq)]
pub struct FilesBreadcumbsProps {
    storage_controller: Signal<StorageController>,
    send_files_mode: bool,
}

#[allow(non_snake_case)]
pub fn FilesBreadcumbs(props: FilesBreadcumbsProps) -> Element {
    let send_files_mode = props.send_files_mode;
    let storage_controller = props.storage_controller;
    let ch = use_coroutine_handle();

    rsx!(div {
        id: "files-breadcrumbs",
        class: "files-breadcrumbs",
        aria_label: "files-breadcrumbs",
        margin_top: format_args!("{}", if send_files_mode {"32px"} else {""}),
        margin: format_args!("{}", if send_files_mode {"var(--gap) 0"} else {"var(--gap)"}),
        {storage_controller.read().dirs_opened_ref.iter().enumerate().map(|(index, dir)| {
            let directory = dir.clone();
            let dir_name = dir.name();
            if dir_name == ROOT_DIR_NAME && index == 0 {
                let home_text = get_local_text("uplink.home");
                rsx!(div {
                    class: "crumb",
                    aria_label: "crumb",
                    onclick: move |_| {
                        ch.send(ChanCmd::BackToPreviousDirectory(directory.clone()));
                    },
                    IconElement {
                        icon: Icon::Home,
                    },
                    p {
                        aria_label: "home-dir",
                        "{home_text}",
                    }
                })
            } else {
                let folder_name_formatted = functions::format_item_name(dir_name);
                rsx!(div {
                    class: "crumb",
                    onclick: move |_| {
                        ch.send(ChanCmd::BackToPreviousDirectory(directory.clone()));
                    },
                    aria_label: "crumb",
                    p {
                        aria_label: "{folder_name_formatted}",
                        "{folder_name_formatted}"
                    }
                },)
            }
        })}
    },)
}

#[derive(Props, Clone, PartialEq)]
pub struct FilesAndFoldersProps {
    storage_controller: Signal<StorageController>,
    on_click_share_files: Option<EventHandler<Vec<Location>>>,
    send_files_mode: bool,
}

#[allow(non_snake_case)]
pub fn FilesAndFolders(props: FilesAndFoldersProps) -> Element {
    let mut state = use_context::<Signal<State>>();
    let send_files_mode = props.send_files_mode;
    let mut storage_controller = props.storage_controller;
    let storage_controller_immutable = props.storage_controller;
    let ch = use_coroutine_handle();

    rsx!(span {
        class: "file-parent",
        background: format_args!("{}", if send_files_mode {"var(--secondary)"} else {""}),
        div {
            id: "files-list",
            class: "files-list",
            aria_label: "files-list",
            {storage_controller.read().add_new_folder.then(|| {
                rsx!(
                Folder {
                    with_rename: true,
                    onrename: move |(val, key_code)| {
                        let new_name: String = val;
                        if storage_controller.read().directories_list.iter().any(|dir| dir.name() == new_name) {
                            state
                            .write()
                            .mutate(common::state::Action::AddToastNotification(
                                ToastNotification::init(
                                    "".into(),
                                    get_local_text("files.directory-already-with-name"),
                                    None,
                                    3,
                                ),
                            ));
                            return;
                        }
                        if key_code == Code::Enter {
                            ch.send(ChanCmd::CreateNewDirectory(new_name));
                            ch.send(ChanCmd::GetItemsFromCurrentDirectory);
                        }
                        storage_controller.with_mut(|i| i.add_new_folder = false);
                     }
                })
            })},
            {storage_controller_immutable.read().directories_list.iter().map(|dir| {
                let folder_name = dir.name();
                let folder_name2 = dir.name();
                let folder_name3 = dir.name();
                let key = dir.id();
                let dir2 = dir.clone();
                let deleting = storage_controller.read().deleting.iter().any(|i|{
                    if let Item::Directory(d) = &i {
                        d.id().eq(&dir.id())
                    } else {
                        false
                    }
                });
                rsx!(
                    ContextMenu {
                        key: "{key}-menu",
                        id: dir.id().to_string(),
                        items: rsx!(
                            ContextItem {
                                icon: Icon::Pencil,
                                aria_label: "folder-rename".to_string(),
                                text: get_local_text("files.rename"),
                                onpress: move |e: MouseEvent| {
                                    storage_controller.with_mut(|i| i.is_renaming_map = Some(key));
                                    e.stop_propagation();
                                }
                            },
                            hr {},
                            ContextItem {
                                icon: Icon::Trash,
                                danger: true,
                                aria_label: "folder-delete".to_string(),
                                text: get_local_text("uplink.delete"),
                                onpress: move |_| {
                                    let item = Item::from(dir2.clone());
                                    ch.send(ChanCmd::DeleteItems(item));
                                }
                            },
                        ),
                        Folder {
                            key: "{key}-folder",
                            text: dir.name(),
                            aria_label: dir.name(),
                            with_rename:storage_controller.with(|i| i.is_renaming_map == Some(key)),
                            onrename: move |(val, key_code)| {
                                if val == folder_name3 {
                                    storage_controller.with(|i| i.is_renaming_map.is_none());
                                    storage_controller.write().finish_renaming_item(false);
                                    return;
                                };
                                if storage_controller.read().directories_list.iter().any(|dir| dir.name() == val) {
                                    state
                                    .write()
                                    .mutate(common::state::Action::AddToastNotification(
                                        ToastNotification::init(
                                            "".into(),
                                            get_local_text("files.directory-already-with-name"),
                                            None,
                                            3,
                                        ),
                                    ));
                                    return;
                                }
                                storage_controller.with_mut(|i| i.is_renaming_map = None);
                                storage_controller.write().finish_renaming_item(false);
                                if key_code == Code::Enter {
                                    ch.send(ChanCmd::RenameItem{old_name: folder_name2.clone(), new_name: val});
                                }
                            },
                            onpress: move |_| {
                                if storage_controller.with(|i| i.is_renaming_map == Some(key)) {
                                    return;
                                }
                                storage_controller.with_mut(|i| i.is_renaming_map = None);
                                ch.send(ChanCmd::OpenDirectory(folder_name.clone()));
                            },
                            disabled: deleting,
                        }
                    }
                )
            })},
            {storage_controller_immutable.read().files_list.iter().cloned().map(|file| {
                let file_name = file.name();
                let file_name2 = file.name();
                let file_name3 = file.name();
                let file_path = format!("{}/{}", storage_controller.read().current_dir_path_as_string, file_name3);
                let file_path2 = format!("{}/{}", storage_controller.read().current_dir_path_as_string, file_name3);
                let file_path3 = format!("{}/{}", storage_controller.read().current_dir_path_as_string, file_name3);
                let file2 = file.clone();
                let file3 = file.clone();
                let key = file.id();
                let file_id = file.id();
                let deleting = storage_controller.read().deleting.iter().any(|i|{
                    if let Item::File(f) = &i {
                        f.id().eq(&file.id())
                    } else {
                        false
                    }
                });
                rsx! {
                    ContextMenu {
                        key: "{key}-menu",
                        id: file.id().to_string(),
                        items: rsx!(
                        if !send_files_mode && !state.read().chats_sidebar().is_empty() {
                            {rsx!(
                            ContextItem {
                                icon: Icon::Share,
                                aria_label: "files-share".to_string(),
                                text: get_local_text("files.share-files"),
                                onpress: move |_| {
                                    if let Some(f) = &props.on_click_share_files {
                                        f.call(vec![Location::Constellation { path: file_path3.clone() }]);
                                    }
                                },
                            },
                            hr {},
                        )}}
                            ContextItem {
                                icon: Icon::Pencil,
                                aria_label: "files-rename".to_string(),
                                text: get_local_text("files.rename"),
                                onpress: move |e: MouseEvent| {
                                    storage_controller.with_mut(|i| i.is_renaming_map = Some(key));
                                    e.stop_propagation();
                                }
                            },
                            if !send_files_mode {
                                {rsx!(ContextItem {
                                    icon: Icon::ArrowDownCircle,
                                    aria_label: "files-download".to_string(),
                                    text: get_local_text("files.download"),
                                    onpress: move |_| {
                                        download_file(&file_name2, ch, None);
                                    },
                                },
                                hr {},
                                ContextItem {
                                    icon: Icon::Trash,
                                    danger: true,
                                    aria_label: "files-delete".to_string(),
                                    text: get_local_text("uplink.delete"),
                                    onpress: move |_| {
                                        let item = Item::from(file2.clone());
                                        ch.send(ChanCmd::DeleteItems(item));
                                    }
                                },)}
                            }
                        ),
                        div {
                            class: "file-wrap",
                            FileCheckbox {
                                file_path: file_path.clone(),
                                storage_controller: storage_controller,
                                is_selecting_files: send_files_mode,
                            },
                            File {
                                key: "{key}-file",
                                thumbnail: thumbnail_to_base64(&file),
                                text: file.name(),
                                aria_label: file.name(),
                                with_rename: storage_controller.with(|i| i.is_renaming_map == Some(key)),
                                onpress: move |_| {
                                    if storage_controller.with(|i| i.is_renaming_map == Some(key)) {
                                        return;
                                    }
                                    if send_files_mode {
                                        toggle_selected_file(storage_controller, file_path2.clone());
                                        return;
                                    }
                                    let key = file_id;
                                    if state.read().ui.file_previews.contains_key(&key) {
                                        state
                                        .write()
                                        .mutate(common::state::Action::AddToastNotification(
                                            ToastNotification::init(
                                                "".into(),
                                                get_local_text("files.file-already-opened"),
                                                None,
                                                2,
                                            ),
                                        ));
                                        return;
                                    }
                                    if file3.thumbnail().is_empty() && !is_file_available_to_preview(&file3.name()) {
                                        state
                                        .write()
                                        .mutate(common::state::Action::AddToastNotification(
                                            ToastNotification::init(
                                                "".into(),
                                                get_local_text("files.no-thumbnail-preview"),
                                                None,
                                                3,
                                            ),
                                        ));
                                        return;
                                    }
                                    let file4 = file3.clone();
                                    storage_controller.with_mut(|i| i.show_file_modal = Some(file4));
                                },
                                onrename: move |(val, key_code)| {
                                    let new_name: String = val;
                                    if new_name == file_name3 {
                                        storage_controller.with(|i| i.is_renaming_map.is_none());
                                        storage_controller.write().finish_renaming_item(false);
                                        return;
                                    };
                                    if  storage_controller.read().files_list.iter().any(|file| file.name() == new_name) {
                                        state
                                        .write()
                                        .mutate(common::state::Action::AddToastNotification(
                                            ToastNotification::init(
                                                "".into(),
                                                get_local_text("files.file-already-with-name"),
                                                None,
                                                3,
                                            ),
                                        ));
                                        return;
                                    }
                                    storage_controller.with(|i| i.is_renaming_map.is_none());
                                    storage_controller.write().finish_renaming_item(false);
                                    if key_code == Code::Enter && !new_name.is_empty() && !new_name.chars().all(char::is_whitespace) {
                                        ch.send(ChanCmd::RenameItem{old_name: file_name.clone(), new_name});
                                    }
                                },
                                disabled: deleting,
                            }
                        }
                    }
                }
            })},
        },
    })
}
