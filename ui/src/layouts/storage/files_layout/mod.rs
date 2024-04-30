#[allow(unused_imports)]
use std::path::Path;

use std::path::PathBuf;
use std::time::Duration;

use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use common::state::data_transfer::TransferTracker;
use common::state::{ui, Action, State};
use common::warp_runner::{RayGunCmd, WarpCmd};
use common::WARP_CMD_CH;
use dioxus::prelude::*;
use dioxus_desktop::use_window;
use dioxus_desktop::wry::FileDropEvent;
use dioxus_router::prelude::use_navigator;
use futures::{channel::oneshot, StreamExt};
use kit::elements::label::Label;
use kit::{
    elements::{
        button::Button,
        tooltip::{ArrowPosition, Tooltip},
        Appearance,
    },
    layout::topbar::Topbar,
};
use rfd::FileDialog;
use uuid::Uuid;
use warp::raygun::Location;

pub mod controller;
pub mod file_preview;

use crate::components::files::upload_progress_bar::FileHoverHandler;
use crate::layouts::chats::ChatSidebar;
use crate::layouts::slimbar::SlimbarLayout;
use crate::layouts::storage::files_layout::file_preview::open_file_preview_modal;
use crate::layouts::storage::send_files_layout::modal::SendFilesLayoutModal;
use crate::layouts::storage::send_files_layout::SendFilesStartLocation;
use crate::layouts::storage::shared_component::{FilesAndFolders, FilesBreadcumbs};
use crate::utils::async_task_queue::use_chat_upload_stream_handler;
use crate::utils::clipboard::clipboard_data::get_files_path_from_clipboard;
use crate::utils::get_drag_event::get_drag_event;
use dioxus_html::input_data::keyboard_types::Code;
use dioxus_html::input_data::keyboard_types::Modifiers;

use self::controller::{StorageController, UploadFileController};

use super::functions::{self, ChanCmd};

#[allow(non_snake_case)]
pub fn FilesLayout() -> Element {
    let mut state = use_context::<Signal<State>>();
    state.write_silent().ui.current_layout = ui::Layout::Storage;
    let mut storage_controller = StorageController::new(state);
    let mut upload_file_controller = UploadFileController::new(state);
    let _window = use_window();
    let files_in_queue_to_upload = upload_file_controller.files_in_queue_to_upload;
    let files_been_uploaded = upload_file_controller.files_been_uploaded;
    let files_in_queue_to_upload2 = files_in_queue_to_upload;
    let files_been_uploaded2 = files_been_uploaded;
    let mut send_files_from_storage = use_signal(|| false);
    let files_pre_selected_to_send: Signal<Vec<Location>> = use_signal(Vec::new);
    let _router = use_navigator();
    let show_slimbar = state.read().show_slimbar() & !state.read().ui.is_minimal_view();
    let file_tracker = use_context::<Signal<TransferTracker>>();

    functions::use_allow_block_folder_nav(files_in_queue_to_upload);

    let ch: Coroutine<ChanCmd> =
        functions::use_init_coroutine(storage_controller, state, file_tracker);

    let _ = use_future(move || {
        to_owned![files_been_uploaded, files_in_queue_to_upload];
        async move {
            // Remove load progress bar if anythings goes wrong
            loop {
                if files_in_queue_to_upload.read().is_empty() && *files_been_uploaded.read() {
                    *files_been_uploaded.write() = false;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    });

    functions::run_verifications_and_update_storage(
        state,
        storage_controller,
        upload_file_controller
            .files_in_queue_to_upload
            .read()
            .clone(),
    );

    functions::use_get_items_from_current_directory(ch);

    #[cfg(not(target_os = "macos"))]
    functions::use_allow_drag_event_for_non_macos_systems(
        upload_file_controller.are_files_hovering_app,
    );
    functions::use_start_upload_file_listener(
        state,
        storage_controller,
        upload_file_controller.clone(),
        file_tracker,
    );

    let upload_streams = use_chat_upload_stream_handler();
    let send_ch = use_coroutine(|mut rx: UnboundedReceiver<(Vec<Location>, Vec<Uuid>)>| {
        to_owned![state, upload_streams, send_files_from_storage];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some((files_location, convs_id)) = rx.next().await {
                let (tx, rx) = oneshot::channel();
                if let Err(e) =
                    warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::SendMessageForSeveralChats {
                        convs_id,
                        msg: vec!["".to_owned()],
                        attachments: files_location,
                        rsp: tx,
                    }))
                {
                    log::error!("Failed to send warp command: {}", e);
                    return;
                }
                if let Ok(Ok(streams)) = rx.await {
                    let mut to_append = upload_streams.write();
                    for (chat, (id, stream)) in streams {
                        state
                            .write()
                            .increment_outgoing_messages(id, vec!["".to_owned()]);
                        if let Some(stream) = stream {
                            to_append.append((chat, id, stream))
                        }
                    }
                }
                send_files_from_storage.set(false);
            }
        }
    });

    rsx!(
        if let Some(file) = storage_controller.read().show_file_modal.as_ref() {
            {let file2 = file.clone();
            rsx!(open_file_preview_modal {
                    on_dismiss: move |_| {
                        storage_controller.with_mut(|i| i.show_file_modal = None);
                    },
                    on_download: move |temp_path| {
                        let file_name = file2.clone().name();
                        functions::download_file(&file_name, ch, temp_path);
                    },
                    file: file.clone()
                }
            )}
        }
        div {
            id: "files-layout",
            aria_label: "files-layout",
            tabindex: "0",
            onkeydown: move |e: Event<KeyboardData>| {
                    let keyboard_data = e;
                    if keyboard_data.code() == Code::KeyV
                        && (keyboard_data.modifiers() == Modifiers::CONTROL || keyboard_data.modifiers() == Modifiers::META)
                    {
                        spawn({
                            to_owned![files_been_uploaded2, files_in_queue_to_upload2];
                            async move {
                                let files_local_path = tokio::task::spawn_blocking(|| {
                                    get_files_path_from_clipboard().unwrap_or_default()
                                })
                                .await
                                .expect("Should succeed");
                            if !files_local_path.is_empty() {
                                functions::add_files_in_queue_to_upload(files_in_queue_to_upload2, files_local_path);
                                files_been_uploaded2.with_mut(|i| *i = true);
                            }
                        }});
                }
            },
            ondragover: move |_| async move {
                if upload_file_controller.are_files_hovering_app.with(|i| !(i)) {
                    let file_drop_event = get_drag_event();
                    if let FileDropEvent::Hovered { .. } = file_drop_event {
                            upload_file_controller.are_files_hovering_app.with_mut(|i| *i = true);

                    }
                }

                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            },
            onclick: move |e| {
                storage_controller.write().finish_renaming_item(false);
                e.stop_propagation();
            },
            if show_slimbar {
                {rsx!(
                    SlimbarLayout {
                        active: crate::UplinkRoute::FilesLayout {}
                    },
                )}
            }
            ChatSidebar {
                active_route: crate::UplinkRoute::FilesLayout {},
            },
            div {
                class: "files-body disable-select",
                aria_label: "files-body",
                    Topbar {
                        with_back_button: state.read().ui.is_minimal_view() && state.read().ui.sidebar_hidden,
                        onback: move |_| {
                            let current = state.read().ui.sidebar_hidden;
                            state.write().mutate(Action::SidebarHidden(!current));
                        },
                        controls:
                            rsx! (Button {
                                    icon: Icon::FolderPlus,
                                    disabled: *upload_file_controller.files_been_uploaded.read(),
                                    appearance: Appearance::Secondary,
                                    aria_label: "add-folder".to_string(),
                                    tooltip: rsx!(
                                        Tooltip {
                                            arrow_position: ArrowPosition::Top,
                                            text: get_local_text("files.new-folder"),
                                        }
                                    ),
                                    onpress: move |e: MouseEvent| {
                                        if !*upload_file_controller.files_been_uploaded.read() {
                                            storage_controller.with_mut(|f| f.finish_renaming_item(true));
                                            e.stop_propagation();
                                        }
                                    },
                                },
                                Button {
                                    icon: Icon::Plus,
                                    appearance: Appearance::Secondary,
                                    aria_label: "upload-file".to_string(),
                                    tooltip: rsx!(
                                        Tooltip {
                                            arrow_position: ArrowPosition::TopRight,
                                            text: get_local_text("files.upload"),
                                        }
                                    ),
                                    onpress: move |_| {
                                        storage_controller.with_mut(|i|  i.is_renaming_map = None);
                                        let files_local_path = match FileDialog::new().set_directory(".").pick_files() {
                                            Some(path) => path,
                                            None => return
                                        };
                                        functions::add_files_in_queue_to_upload(upload_file_controller.files_in_queue_to_upload, files_local_path);
                                        upload_file_controller.files_been_uploaded.with_mut(|i| *i = true);
                                    },
                                }
                            )
                        ,
                        div {
                            class: "files-info",
                            aria_label: "files-info",
                            if storage_controller.read().storage_size.0.is_empty() {
                                {rsx!(div {
                                    class: "skeletal-texts",
                                    div {
                                        class: "skeletal-text",
                                        div {
                                            class: "skeletal-text-content skeletal",
                                        }
                                    },
                                },
                                div {
                                    class: "skeletal-texts",
                                    div {
                                        class: "skeletal-text",
                                        div {
                                            class: "skeletal-text-content skeletal",
                                        }
                                    },
                                })}
                            } else {
                                {rsx!(
                                    p {
                                        class: "free-space",
                                        aria_label: "free-space-max-size",
                                        {get_local_text("files.storage-max-size")},
                                        span {
                                            class: "count",
                                            {storage_controller.read().storage_size.0.to_string()},
                                        }
                                    },
                                    p {
                                        class: "free-space",
                                        aria_label: "free-space-current-size",
                                        {get_local_text("files.storage-current-size")},
                                        span {
                                            class: "count",
                                            {storage_controller.read().storage_size.1.to_string()},
                                        }
                                    },
                                )}
                            }
                        }
                    },
            FileHoverHandler {
                are_files_hovering_app: upload_file_controller.are_files_hovering_app,
                files_been_uploaded: upload_file_controller.files_been_uploaded,
                on_update: move |files_to_upload: Vec<PathBuf>|  {
                    functions::add_files_in_queue_to_upload(upload_file_controller.files_in_queue_to_upload, files_to_upload);
                },
            },
            SendFilesLayoutModal {
                send_files_from_storage: send_files_from_storage,
                send_files_start_location: SendFilesStartLocation::Storage,
                files_pre_selected_to_send: files_pre_selected_to_send.read().clone(),
                on_send: move |(files_location, convs_id): (Vec<Location>, Vec<Uuid>)| {
                    send_ch.send((files_location, convs_id));
                }
            },
            FilesBreadcumbs {
                storage_controller: storage_controller,
                send_files_mode: false,
            },
            if storage_controller.read().files_list.is_empty()
                && storage_controller.read().directories_list.is_empty()
                && !storage_controller.read().add_new_folder {
                    {rsx!(
                        div {
                            class: "no-files-div",
                            Label {
                                text: get_local_text("files.no-files-available"),
                            }
                        }
                    )}
               } else {
                {rsx!(FilesAndFolders {
                    storage_controller: storage_controller,
                    on_click_share_files: move |files_pre_selected: Vec<Location>| {
                        *files_pre_selected_to_send.write_silent() = files_pre_selected;
                        send_files_from_storage.set(true);
                    },
                    send_files_mode: false,
                })}
               }
                {(state.read().ui.sidebar_hidden && state.read().ui.metadata.minimal_view).then(|| rsx!(
                    crate::AppNav {
                        active: crate::UplinkRoute::FilesLayout{},
                    }
                ))}
            }
        }
    )
}
