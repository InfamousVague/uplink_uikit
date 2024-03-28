#[cfg(not(target_os = "macos"))]
use crate::utils::get_drag_event;
use common::{
    language::{get_local_text, get_local_text_with_args},
    state::{
        data_transfer::{TrackerType, TransferState, TransferTracker},
        storage::Storage,
        Action, State, ToastNotification,
    },
    upload_file_channel::{UploadFileAction, UPLOAD_FILE_LISTENER},
    warp_runner::{ConstellationCmd, WarpCmd},
    WARP_CMD_CH,
};
use dioxus::{
    events::eval,
    prelude::*,
    signals::{Readable, Signal},
};
#[cfg(not(target_os = "macos"))]
use dioxus_desktop::wry::webview::FileDropEvent;
use dioxus_hooks::{to_owned, use_coroutine, use_resource, Coroutine, UnboundedReceiver};
use futures::{channel::oneshot, StreamExt};
use rfd::FileDialog;
use std::{ffi::OsStr, path::PathBuf, rc::Rc, time::Duration};
use tokio::time::sleep;
use uuid::Uuid;
use warp::constellation::{directory::Directory, item::Item};

use crate::utils::{
    async_task_queue::{download_stream_handler, DownloadStreamData},
    download::get_download_path,
};

use super::files_layout::controller::{StorageController, UploadFileController};

pub type UseEvalFn = Rc<dyn Fn(&str) -> Result<UseEval, EvalError>>;

static ALLOW_FOLDER_NAVIGATION: &str = r#"
    var folders_element = document.getElementById('files-list');
    folders_element.style.pointerEvents = '$POINTER_EVENT';
    folders_element.style.opacity = '$OPACITY';
    var folders_breadcumbs_element = document.getElementById('files-breadcrumbs');
    folders_breadcumbs_element.style.pointerEvents = '$POINTER_EVENT';
    folders_breadcumbs_element.style.opacity = '$OPACITY';
"#;

const MAX_LEN_TO_FORMAT_NAME: usize = 64;

pub fn run_verifications_and_update_storage(
    mut state: Signal<State>,
    mut controller: Signal<StorageController>,
    files_in_queue_to_upload: Vec<PathBuf>,
) {
    let files_in_queue_to_upload_list = files_in_queue_to_upload;

    if controller.read().first_render && state.read().ui.is_minimal_view() {
        state.write_silent().mutate(Action::SidebarHidden(true));
        controller.with_mut(|i| i.first_render = false);
    }

    if state.read().storage.files_in_queue_to_upload.len() != files_in_queue_to_upload_list.len() {
        state.write_silent().storage.files_in_queue_to_upload =
            files_in_queue_to_upload_list.clone();
    }
    if let Some(storage) = controller.write_silent().update_state() {
        state.write().storage = Storage {
            files_in_queue_to_upload: files_in_queue_to_upload_list,
            ..storage
        };
    }
}

pub fn get_items_from_current_directory(ch: Coroutine<ChanCmd>) {
    let _ = use_resource(move || {
        to_owned![ch];
        async move {
            sleep(Duration::from_secs(1)).await;
            ch.send(ChanCmd::GetItemsFromCurrentDirectory);
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn allow_drag_event_for_non_macos_systems(are_files_hovering_app: &Signal<bool>) {
    use_resource(move || {
        async move {
            // ondragover function from div does not work on windows
            loop {
                sleep(Duration::from_millis(100)).await;
                if let FileDropEvent::Hovered { .. } = get_drag_event::get_drag_event() {
                    if are_files_hovering_app.with(|i| !(*i)) {
                        are_files_hovering_app.with_mut(|i| *i = true);
                    };
                }
            }
        }
    });
}

pub fn format_item_name(file_name: String) -> String {
    let item = PathBuf::from(&file_name);

    let file_stem = item
        .file_stem()
        .and_then(OsStr::to_str)
        .map(str::to_string)
        .unwrap_or_default();

    file_name
        .get(0..64)
        .map(|x| x.to_string())
        .map(|x| {
            if file_stem.len() > MAX_LEN_TO_FORMAT_NAME {
                format!("{x}...")
            } else {
                x
            }
        })
        .unwrap_or_else(|| file_name.clone())
}

pub fn format_item_size(item_size: usize) -> String {
    if item_size == 0 {
        return String::from("0 bytes");
    }
    let base_1024: f64 = 1024.0;
    let size_f64: f64 = item_size as f64;

    let i = (size_f64.log10() / base_1024.log10()).floor();
    let size_formatted = size_f64 / base_1024.powf(i);

    let item_size_suffix = ["bytes", "KB", "MB", "GB", "TB"][i as usize];
    let mut size_formatted_string = format!(
        "{size:.*} {size_suffix}",
        1,
        size = size_formatted,
        size_suffix = item_size_suffix
    );
    if size_formatted_string.contains(".0") {
        size_formatted_string = size_formatted_string.replace(".0", "");
    }
    size_formatted_string
}

pub fn download_file(
    file_name: &str,
    ch: Coroutine<ChanCmd>,
    temp_path_to_download_file_to_preview: Option<PathBuf>,
) {
    let file_extension = std::path::Path::new(&file_name)
        .extension()
        .and_then(OsStr::to_str)
        .map(|s| s.to_string())
        .unwrap_or_default();
    let file_stem = PathBuf::from(&file_name)
        .file_stem()
        .and_then(OsStr::to_str)
        .map(str::to_string)
        .unwrap_or_default();
    let file_path_buf = if temp_path_to_download_file_to_preview.is_none() {
        match FileDialog::new()
            .set_directory(".")
            .set_file_name(file_stem)
            .add_filter("", &[&file_extension])
            .save_file()
        {
            Some(path) => path,
            None => return,
        }
    } else {
        temp_path_to_download_file_to_preview
            .clone()
            .unwrap_or_default()
    };
    ch.send(ChanCmd::DownloadFile {
        file_name: file_name.to_string(),
        local_path_to_save_file: file_path_buf,
        notification_download_status: temp_path_to_download_file_to_preview.is_none(),
    });
}

pub fn add_files_in_queue_to_upload(
    files_in_queue_to_upload: Signal<Vec<PathBuf>>,
    files_path: Vec<PathBuf>,
) {
    let tx_upload_file = UPLOAD_FILE_LISTENER.tx.clone();
    allow_folder_navigation(false);
    files_in_queue_to_upload
        .write_silent()
        .extend(files_path.clone());
    let _ = tx_upload_file.send(UploadFileAction::UploadFiles(files_path));
}

pub fn use_allow_block_folder_nav(files_in_queue_to_upload: Signal<Vec<PathBuf>>) {
    // Block directories navigation if there is a file been uploaded
    // use_future here to verify before render elements on first render
    let _ = use_resource(move || async move {
        allow_folder_navigation(files_in_queue_to_upload().is_empty());
    });
    // This is to run on all re-renders
    allow_folder_navigation(files_in_queue_to_upload().is_empty());
}

pub fn allow_folder_navigation(allow_navigation: bool) {
    let new_script = if allow_navigation {
        ALLOW_FOLDER_NAVIGATION
            .replace("$POINTER_EVENT", "")
            .replace("$OPACITY", "1")
    } else {
        ALLOW_FOLDER_NAVIGATION
            .replace("$POINTER_EVENT", "none")
            .replace("$OPACITY", "0.5")
    };

    _ = eval(&new_script);
}

pub enum ChanCmd {
    GetItemsFromCurrentDirectory,
    CreateNewDirectory(String),
    OpenDirectory(String),
    BackToPreviousDirectory(Directory),
    DownloadFile {
        file_name: String,
        local_path_to_save_file: PathBuf,
        notification_download_status: bool,
    },
    RenameItem {
        old_name: String,
        new_name: String,
    },
    DeleteItems(Item),
}

pub fn init_coroutine(
    controller: Signal<StorageController>,
    state: Signal<State>,
    file_tracker: Signal<TransferTracker>,
) -> Coroutine<ChanCmd> {
    let download_queue = download_stream_handler();

    use_coroutine(|mut rx: UnboundedReceiver<ChanCmd>| {
        to_owned![controller, download_queue, state, file_tracker];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    ChanCmd::CreateNewDirectory(directory_name) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        let directory_name2 = directory_name.clone();

                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                            ConstellationCmd::CreateNewDirectory {
                                directory_name,
                                rsp: tx,
                            },
                        )) {
                            log::error!("failed to add new directory {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");

                        match rsp {
                            Ok(_) => {
                                log::info!("New directory added: {}", directory_name2);
                            }
                            Err(e) => {
                                log::error!("failed to add new directory: {}", e);
                                continue;
                            }
                        }
                    }
                    ChanCmd::GetItemsFromCurrentDirectory => {
                        let (tx, rx) = oneshot::channel::<Result<Storage, warp::error::Error>>();

                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                            ConstellationCmd::GetItemsFromCurrentDirectory { rsp: tx },
                        )) {
                            log::error!("failed to get items from current directory {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(storage) => {
                                controller.with_mut(|i| i.storage_state = Some(storage));
                            }
                            Err(e) => {
                                log::error!("failed to add new directory: {}", e);
                                continue;
                            }
                        }
                    }
                    ChanCmd::OpenDirectory(directory_name) => {
                        let (tx, rx) = oneshot::channel::<Result<Storage, warp::error::Error>>();
                        let directory_name2 = directory_name.clone();

                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                            ConstellationCmd::OpenDirectory {
                                directory_name,
                                rsp: tx,
                            },
                        )) {
                            log::error!("failed to open {directory_name2} directory {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(storage) => {
                                controller.with_mut(|i| i.storage_state = Some(storage));
                                log::info!("Folder {} opened", directory_name2);
                            }
                            Err(e) => {
                                log::error!("failed to open folder {directory_name2}: {}", e);
                                continue;
                            }
                        }
                    }
                    ChanCmd::BackToPreviousDirectory(directory) => {
                        let (tx, rx) = oneshot::channel::<Result<Storage, warp::error::Error>>();
                        let directory_name = directory.name();

                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                            ConstellationCmd::BackToPreviousDirectory { directory, rsp: tx },
                        )) {
                            log::error!("failed to open directory {}: {}", directory_name, e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(storage) => {
                                controller.with_mut(|i| i.storage_state = Some(storage));
                                log::info!("Folder {} opened", directory_name);
                            }
                            Err(e) => {
                                log::error!("failed to open directory {}: {}", directory_name, e);
                                continue;
                            }
                        }
                    }
                    ChanCmd::DownloadFile {
                        file_name,
                        local_path_to_save_file,
                        notification_download_status,
                    } => {
                        let (local_path_to_save_file, on_finish) =
                            get_download_path(local_path_to_save_file);
                        let (tx, rx) = oneshot::channel();

                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                            ConstellationCmd::DownloadFile {
                                file_name: file_name.clone(),
                                local_path_to_save_file,
                                rsp: tx,
                            },
                        )) {
                            if notification_download_status {
                                state.write().mutate(Action::AddToastNotification(
                                    ToastNotification::init(
                                        "".into(),
                                        get_local_text_with_args(
                                            "files.download-failed",
                                            vec![("file", file_name)],
                                        ),
                                        None,
                                        2,
                                    ),
                                ));
                            }
                            log::error!("failed to download file {}", e);
                            continue;
                        }

                        // Unique id to track this download
                        let file_id = Uuid::new_v4();
                        let file_state = TransferState::new();
                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(stream) => {
                                download_queue.write().append(DownloadStreamData {
                                    stream,
                                    file: file_name.clone(),
                                    id: file_id,
                                    on_finish,
                                    show_toast: notification_download_status,
                                    file_state: file_state.clone(),
                                });
                            }
                            Err(error) => {
                                if notification_download_status {
                                    state.write().mutate(Action::AddToastNotification(
                                        ToastNotification::init(
                                            "".into(),
                                            get_local_text_with_args(
                                                "files.download-failed",
                                                vec![("file", file_name)],
                                            ),
                                            None,
                                            2,
                                        ),
                                    ));
                                }
                                log::error!("failed to download file: {}", error);
                                continue;
                            }
                        }
                        file_tracker.write().start_file_transfer(
                            file_id,
                            file_name,
                            file_state,
                            TrackerType::FileDownload,
                        );
                    }
                    ChanCmd::RenameItem { old_name, new_name } => {
                        let (tx, rx) = oneshot::channel::<Result<Storage, warp::error::Error>>();

                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::Constellation(ConstellationCmd::RenameItem {
                                old_name,
                                new_name,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to rename item {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(storage) => {
                                controller.with_mut(|i| i.storage_state = Some(storage));
                            }
                            Err(e) => {
                                log::error!(
                                    "failed to update uplink storage with renamed item: {}",
                                    e
                                );
                                continue;
                            }
                        }
                    }
                    ChanCmd::DeleteItems(item) => {
                        controller.with_mut(|i| i.deleting.push(item.clone()));
                        let (tx, rx) = oneshot::channel::<Result<Storage, warp::error::Error>>();

                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                            ConstellationCmd::DeleteItems {
                                item: item.clone(),
                                rsp: tx,
                            },
                        )) {
                            log::error!("failed to delete items {}, item {:?}", e, item.name());
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(storage) => {
                                controller.with_mut(|i| {
                                    i.storage_state = Some(storage);
                                    i.deleting.retain(|i| match i {
                                        Item::File(f) => {
                                            if let Item::File(f2) = &item {
                                                !f.id().eq(&f2.id())
                                            } else {
                                                true
                                            }
                                        }
                                        Item::Directory(d) => {
                                            if let Item::File(d2) = &item {
                                                !d.id().eq(&d2.id())
                                            } else {
                                                true
                                            }
                                        }
                                    });
                                });
                            }
                            Err(e) => {
                                log::error!("failed to delete items {}, item {:?}", e, item.name());
                                continue;
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Upload files has many states to manage
/// 1. It is necessary to check if any file is being uploaded, hence the use of `use_future`.
/// 2. It was necessary to use the global channel to send the command to upload the files,
/// to fix a specific behavior (when the user leaves the page, returns,
/// and tries to upload a second file, then leaves and returns again,
/// it was not possible to cancel that upload in the coroutine).
pub fn start_upload_file_listener(
    state: Signal<State>,
    controller: Signal<StorageController>,
    upload_file_controller: UploadFileController,
    file_tracker: Signal<TransferTracker>,
) {
    let files_been_uploaded = upload_file_controller.files_been_uploaded;
    let files_in_queue_to_upload = upload_file_controller.files_in_queue_to_upload;

    use_resource(move || {
        to_owned![
            state,
            controller,
            files_been_uploaded,
            files_in_queue_to_upload,
            file_tracker
        ];
        async move {
            let listener_channel = UPLOAD_FILE_LISTENER.rx.clone();
            log::trace!("starting upload file action listener");
            let mut ch = listener_channel.lock().await;
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = ch.recv().await {
                match cmd {
                    UploadFileAction::UploadFiles(files_path) => {
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                            ConstellationCmd::UploadFiles { files_path },
                        )) {
                            log::error!("failed to upload files {}", e);
                            continue;
                        }
                    }
                    UploadFileAction::SizeNotAvailable(path, file_name) => {
                        files_in_queue_to_upload.with_mut(|i| i.retain(|p| !p.eq(&path)));
                        state
                            .write()
                            .mutate(common::state::Action::AddToastNotification(
                                ToastNotification::init(
                                    "".into(),
                                    get_local_text_with_args(
                                        "files.no-size-available",
                                        vec![("file", file_name)],
                                    ),
                                    None,
                                    3,
                                ),
                            ));
                    }
                    UploadFileAction::Starting(id, file_state, file_name) => {
                        *files_been_uploaded.write_silent() = true;
                        file_tracker.write().start_file_transfer(
                            id,
                            file_name,
                            file_state,
                            TrackerType::FileUpload,
                        );
                    }
                    UploadFileAction::Pausing(id) => {
                        file_tracker
                            .write()
                            .pause_file_upload(id, TrackerType::FileUpload);
                    }
                    UploadFileAction::Cancelling(path, id) => {
                        file_tracker
                            .write()
                            .cancel_file_upload(id, TrackerType::FileUpload);
                        files_in_queue_to_upload.with_mut(|i| i.retain(|p| !p.eq(&path)));
                    }
                    UploadFileAction::Uploading((progress, msg, file)) => {
                        if !*files_been_uploaded.read() && controller.read().first_render {
                            files_been_uploaded.with_mut(|i| *i = true);
                        }
                        if let Some(progress) = progress {
                            file_tracker.write().update_file_upload(
                                file,
                                progress,
                                TrackerType::FileUpload,
                            );
                        }
                        file_tracker.write().update_file_description(
                            file,
                            msg,
                            TrackerType::FileUpload,
                        );
                    }
                    UploadFileAction::Finishing(path, file, finish) => {
                        *files_been_uploaded.write_silent() = true;
                        if finish {
                            if !files_in_queue_to_upload.read().is_empty() {
                                files_in_queue_to_upload.with_mut(|i| i.retain(|p| !p.eq(&path)));
                            }
                            file_tracker
                                .write()
                                .remove_file_upload(file, TrackerType::FileUpload);
                        } else {
                            file_tracker.write().update_file_description(
                                file,
                                get_local_text("files.finishing-upload"),
                                TrackerType::FileUpload,
                            );
                        }
                    }
                    UploadFileAction::Finished(storage) => {
                        if files_in_queue_to_upload.read().is_empty() {
                            *files_been_uploaded.write_silent() = false;
                        }
                        controller.with_mut(|i| i.storage_state = Some(storage));
                    }
                    UploadFileAction::Remove(path, file) => {
                        files_in_queue_to_upload.with_mut(|i| i.retain(|p| !p.eq(&path)));
                        file_tracker
                            .write()
                            .remove_file_upload(file, TrackerType::FileUpload);
                        *files_been_uploaded.write_silent() =
                            file_tracker.read().file_progress_upload.is_empty();
                    }
                    UploadFileAction::Error(path, file) => {
                        match path {
                            Some(path) => {
                                files_in_queue_to_upload.with_mut(|i| i.retain(|p| !p.eq(&path)))
                            }
                            None => files_in_queue_to_upload.with_mut(|i| i.clear()),
                        }
                        if let Some(file) = file {
                            file_tracker
                                .write()
                                .error_file_upload(file, TrackerType::FileUpload);
                            continue;
                        }
                        state
                            .write()
                            .mutate(common::state::Action::AddToastNotification(
                                ToastNotification::init(
                                    "".into(),
                                    get_local_text("files.error-to-upload"),
                                    None,
                                    3,
                                ),
                            ));
                    }
                }
            }
        }
    });
}
