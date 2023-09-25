use std::path::PathBuf;

use common::{
    state::{storage::Storage, State},
    ROOT_DIR_NAME,
};
use dioxus_core::ScopeState;
use dioxus_hooks::{use_ref, UseRef, UseSharedState};
use uuid::Uuid;
use warp::{constellation::directory::Directory, raygun::Location};

use super::functions::{self, format_item_size};

#[derive(Clone)]
pub struct StorageController {
    pub storage_state: Option<Storage>,
    pub directories_list: Vec<Directory>,
    pub files_list: Vec<warp::constellation::file::File>,
    pub current_dir: Directory,
    pub dirs_opened_ref: Vec<Directory>,
    pub storage_size: (String, String),
    pub is_renaming_map: Option<Uuid>,
    pub add_new_folder: bool,
    pub first_render: bool,
    pub show_file_modal: Option<warp::constellation::file::File>,
    pub files_selected_to_send: Vec<Location>,
    pub current_dir_path_as_string: String,
    pub chats_selected_to_send: Vec<Uuid>,
}

impl StorageController {
    pub fn new<'a>(cx: &'a ScopeState, state: &'a UseSharedState<State>) -> &'a UseRef<Self> {
        let controller = Self {
            storage_state: None,
            directories_list: state.read().storage.directories.clone(),
            files_list: state.read().storage.files.clone(),
            current_dir: state.read().storage.current_dir.clone(),
            dirs_opened_ref: state.read().storage.directories_opened.clone(),
            storage_size: (
                functions::format_item_size(state.read().storage.max_size),
                functions::format_item_size(state.read().storage.current_size),
            ),
            is_renaming_map: None,
            add_new_folder: false,
            first_render: true,
            show_file_modal: None,
            files_selected_to_send: state
                .read()
                .get_active_chat()
                .map(|f| f.files_attached_to_send)
                .unwrap_or_default()
                .to_vec(),
            current_dir_path_as_string: state
                .read()
                .storage
                .directories_opened
                .iter()
                .filter(|dir| dir.name() != ROOT_DIR_NAME)
                .map(|dir| dir.name())
                .collect::<Vec<_>>()
                .join("/"),
            chats_selected_to_send: Vec::new(),
        };
        use_ref(cx, || controller)
    }

    pub fn update_current_dir_path(&mut self, state: UseSharedState<State>) {
        self.current_dir_path_as_string = state
            .read()
            .storage
            .directories_opened
            .iter()
            .filter(|dir| dir.name() != ROOT_DIR_NAME)
            .map(|dir| dir.name())
            .collect::<Vec<_>>()
            .join("/");
    }

    pub fn update_state(&mut self) -> Option<Storage> {
        if let Some(storage) = self.storage_state.take() {
            self.directories_list = storage.directories.clone();
            self.files_list = storage.files.clone();
            self.current_dir = storage.current_dir.clone();
            self.dirs_opened_ref = storage.directories_opened.clone();
            self.storage_size = (
                format_item_size(storage.max_size),
                format_item_size(storage.current_size),
            );
            self.storage_state = None;
            Some(storage)
        } else {
            None
        }
    }

    pub fn finish_renaming_item(&mut self, should_toggle: bool) {
        self.is_renaming_map.take();
        if should_toggle {
            self.add_new_folder = !self.add_new_folder;
        } else {
            self.add_new_folder = false;
        }
    }
}

#[derive(PartialEq, Clone)]
pub struct UploadFileController<'a> {
    pub are_files_hovering_app: &'a UseRef<bool>,
    pub files_been_uploaded: &'a UseRef<bool>,
    pub files_in_queue_to_upload: &'a UseRef<Vec<PathBuf>>,
    pub disable_cancel_upload_button: &'a UseRef<bool>,
}

impl<'a> UploadFileController<'a> {
    pub fn new(cx: &'a ScopeState, state: UseSharedState<State>) -> Self {
        Self {
            are_files_hovering_app: use_ref(cx, || false),
            files_been_uploaded: use_ref(cx, || {
                !state.read().storage.files_in_queue_to_upload.is_empty()
            }),
            files_in_queue_to_upload: use_ref(cx, || {
                state.read().storage.files_in_queue_to_upload.clone()
            }),
            disable_cancel_upload_button: use_ref(cx, || false),
        }
    }
}
