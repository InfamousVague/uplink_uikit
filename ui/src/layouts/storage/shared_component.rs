use std::str::FromStr;

use crate::layouts::storage::functions::{self, download_file, ChanCmd};
use crate::layouts::storage::send_files_layout::send_files_components::{
    toggle_selected_file, FileCheckbox,
};

use super::files_layout::controller::StorageController;
use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;
use common::is_file_available_to_preview;
use common::language::get_local_text_with_args;
use common::state::{State, ToastNotification};
use common::warp_runner::thumbnail_to_base64;
use common::{language::get_local_text, ROOT_DIR_NAME};

use dioxus::html::input_data::keyboard_types::Code;
use dioxus::prelude::*;
use kit::components::context_menu::{ContextItem, ContextMenu};
use kit::elements::file::File;
use kit::elements::folder::Folder;
use uuid::Uuid;
use warp::constellation::directory::Directory;
use warp::constellation::item::Item;
use warp::raygun::{ConversationSettings, Location};

#[derive(Props)]
pub struct FilesBreadcumbsProps<'a> {
    storage_controller: &'a UseRef<StorageController>,
    ch: &'a Coroutine<ChanCmd>,
    send_files_mode: bool,
}

#[allow(non_snake_case)]
pub fn FilesBreadcumbs<'a>(cx: Scope<'a, FilesBreadcumbsProps<'a>>) -> Element<'a> {
    let state = use_shared_state::<State>(cx)?;
    let send_files_mode = cx.props.send_files_mode;
    let storage_controller = cx.props.storage_controller;
    let ch = cx.props.ch;

    cx.render(rsx!(div {
        id: "files-breadcrumbs",
        class: "files-breadcrumbs",
        aria_label: "files-breadcrumbs",
        margin_top: format_args!("{}", if send_files_mode {"32px"} else {""}),
        margin: format_args!("{}", if send_files_mode {"var(--gap) 0"} else {"var(--gap)"}),
        storage_controller.read().dirs_opened_ref.iter().enumerate().map(|(index, dir)| {
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
                let folder_name_resolved = resolve_directory_name(dir, &state.read());
                let folder_name_formatted = functions::format_item_name(folder_name_resolved);
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
        })
    },))
}

#[derive(Props)]
pub struct FilesAndFoldersProps<'a> {
    storage_controller: &'a UseRef<StorageController>,
    ch: &'a Coroutine<ChanCmd>,
    on_click_share_files: Option<EventHandler<'a, Vec<Location>>>,
    send_files_mode: bool,
}

#[allow(non_snake_case)]
pub fn FilesAndFolders<'a>(cx: Scope<'a, FilesAndFoldersProps<'a>>) -> Element<'a> {
    let state = use_shared_state::<State>(cx)?;
    let send_files_mode = cx.props.send_files_mode;
    let storage_controller = cx.props.storage_controller;
    let ch = cx.props.ch;
    cx.render(rsx!(span {
        class: "file-parent",
        background: format_args!("{}", if send_files_mode {"var(--secondary)"} else {""}),
        div {
            id: "files-list",
            class: "files-list",
            aria_label: "files-list",
            storage_controller.read().add_new_folder.then(|| {
                rsx!(
                Folder {
                    with_rename: true,
                    onrename: |(val, key_code)| {
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
            }),
            storage_controller.read().directories_list.iter().map(|dir| {
                let folder_name = dir.name();
                let folder_name2 = folder_name.clone();
                let folder_name3 = folder_name.clone();
                let folder_name_resolved = resolve_directory_name(dir, &state.read());
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
                        items: cx.render(rsx!(
                            ContextItem {
                                icon: Icon::Pencil,
                                aria_label: "folder-rename".into(),
                                text: get_local_text("files.rename"),
                                onpress: move |_| {
                                    storage_controller.with_mut(|i| i.is_renaming_map = Some(key));
                                }
                            },
                            hr {},
                            ContextItem {
                                icon: Icon::Trash,
                                danger: true,
                                aria_label: "folder-delete".into(),
                                text: get_local_text("uplink.delete"),
                                onpress: move |_| {
                                    let item = Item::from(dir2.clone());
                                    ch.send(ChanCmd::DeleteItems(item));
                                }
                            },
                        )),
                        Folder {
                            key: "{key}-folder",
                            text: folder_name_resolved.clone(),
                            aria_label: folder_name_resolved,
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
                                storage_controller.with_mut(|i| i.is_renaming_map = None);
                                ch.send(ChanCmd::OpenDirectory(folder_name.clone()));
                            },
                            disabled: deleting,
                        }
                    }
                )
            }),
            storage_controller.read().files_list.iter().map(|file| {
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
                        items: cx.render(rsx!(
                        if !send_files_mode && !state.read().chats_sidebar().is_empty() {
                            rsx!(
                            ContextItem {
                                icon: Icon::Share,
                                aria_label: "files-share".into(),
                                text: get_local_text("files.share-files"),
                                onpress: move |_| {
                                    if let Some(f) = &cx.props.on_click_share_files {
                                        f.call(vec![Location::Constellation { path: file_path3.clone() }]);
                                    }
                                },
                            },
                            hr {},
                        )}
                            ContextItem {
                                icon: Icon::Pencil,
                                aria_label: "files-rename".into(),
                                text: get_local_text("files.rename"),
                                onpress: move |_| {
                                    storage_controller.with_mut(|i| i.is_renaming_map = Some(key));
                                }
                            },
                            if !send_files_mode {
                                rsx!(ContextItem {
                                    icon: Icon::ArrowDownCircle,
                                    aria_label: "files-download".into(),
                                    text: get_local_text("files.download"),
                                    onpress: move |_| {
                                        download_file(&file_name2, ch, None);
                                    },
                                },
                                hr {},
                                ContextItem {
                                    icon: Icon::Trash,
                                    danger: true,
                                    aria_label: "files-delete".into(),
                                    text: get_local_text("uplink.delete"),
                                    onpress: move |_| {
                                        let item = Item::from(file2.clone());
                                        ch.send(ChanCmd::DeleteItems(item));
                                    }
                                },)
                            }
                        )),
                        div {
                            class: "file-wrap",
                            FileCheckbox {
                                file_path: file_path.clone(),
                                storage_controller: storage_controller.clone(),
                                is_selecting_files: send_files_mode,
                            },
                            File {
                                key: "{key}-file",
                                thumbnail: thumbnail_to_base64(file),
                                text: file.name(),
                                aria_label: file.name(),
                                with_rename: storage_controller.with(|i| i.is_renaming_map == Some(key)),
                                onpress: move |_| {
                                    if send_files_mode {
                                        toggle_selected_file(storage_controller.clone(), file_path2.clone());
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
            }),
        },
    }))
}

fn resolve_directory_name(dir: &Directory, state: &State) -> String {
    let folder_name = dir.name();
    // Try to check and resolve the foldername for chats
    match Uuid::from_str(&folder_name) {
        Ok(id) => {
            log::debug!("with id {id}");
            state
                .get_chat_by_id(id)
                .and_then(|c| {
                    log::debug!("chat type {:?}", c.settings);
                    c.conversation_name.or(match c.settings {
                        ConversationSettings::Direct(_) => {
                            // If DM try use the other users name
                            let own = state.did_key();
                            let other = c.participants.iter().find(|id| !own.eq(id));
                            log::debug!("other {:?}", other.and_then(|o| state.get_identity(o)));
                            other.and_then(|o| state.get_identity(o)).map(|id| {
                                get_local_text_with_args(
                                    "files.direct-message-name",
                                    vec![("with", id.username())],
                                )
                            })
                        }
                        ConversationSettings::Group(_) => None,
                    })
                })
                .unwrap_or(folder_name)
        }
        Err(_) => folder_name,
    }
}
