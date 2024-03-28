use common::{
    language::{get_local_text, get_local_text_with_args},
    state::{self, data_transfer::TransferTracker, State},
};
use dioxus::prelude::*;
use kit::{
    components::{
        indicator::Status, message::format_text, user::User, user_image::UserImage,
        user_image_group::UserImageGroup,
    },
    elements::{checkbox::Checkbox, label::Label},
};
use uuid::Uuid;
use warp::raygun::{self, ConversationType, Location};

pub mod modal;
pub mod send_files_components;

use crate::{
    layouts::storage::{
        send_files_layout::send_files_components::SendFilesTopbar,
        shared_component::{FilesAndFolders, FilesBreadcumbs},
    },
    utils::build_participants,
};

use super::{
    files_layout::controller::StorageController,
    functions::{self, ChanCmd},
};

#[derive(PartialEq, Clone)]
pub enum SendFilesStartLocation {
    Chats,
    Storage,
}

#[derive(Props, Clone, PartialEq)]
pub struct SendFilesProps {
    send_files_from_storage_state: Signal<bool>,
    send_files_start_location: SendFilesStartLocation,
    on_files_attached: EventHandler<(Vec<Location>, Vec<Uuid>)>,
    files_pre_selected_to_send: Vec<Location>,
}

#[allow(non_snake_case)]
pub fn SendFilesLayout(props: SendFilesProps) -> Element {
    let mut state = use_context::<Signal<State>>();
    let send_files_start_location = props.send_files_start_location.clone();
    let send_files_from_storage_state = props.send_files_from_storage_state.clone();
    let storage_controller = StorageController::new(state);
    let first_render = use_signal(|| true);
    let file_tracker = use_context::<Signal<TransferTracker>>();
    let ch: Coroutine<ChanCmd> =
        functions::init_coroutine(storage_controller.clone(), state, file_tracker);
    let in_files = send_files_start_location.eq(&SendFilesStartLocation::Storage);
    functions::get_items_from_current_directory(ch);

    functions::run_verifications_and_update_storage(state, storage_controller, vec![]);

    if *first_render.read() {
        *first_render.write_silent() = false;
        storage_controller.write_silent().files_selected_to_send =
            props.files_pre_selected_to_send.clone();
    }

    storage_controller
        .write_silent()
        .update_current_dir_path(state.clone());

    rsx!(div {
        id: "send-files-layout",
        aria_label: "send-files-layout",
        div {
            class: "files-body disable-select",
            aria_label: "send-files-body",
            SendFilesTopbar {
                send_files_start_location: send_files_start_location.clone(),
                send_files_from_storage_state: send_files_from_storage_state.clone(),
                storage_controller: storage_controller.clone(),
                on_send: move |files_location_path| {
                    props.on_files_attached.call((files_location_path, storage_controller.with(|f| f.chats_selected_to_send.clone())));
                },
                in_files: in_files
            }
            if in_files {
                ChatsToSelect {
                    storage_controller: storage_controller.clone(),
                }
            }
            FilesBreadcumbs {
                storage_controller: storage_controller.clone(),
                send_files_mode: true,
            },
            if storage_controller.read().files_list.is_empty()
                && storage_controller.read().directories_list.is_empty() {
                        div {
                            padding: "48px",
                            Label {
                                text: get_local_text("files.no-files-available"),
                            }
                        }
               } else {
                FilesAndFolders {
                    storage_controller: storage_controller.clone(),
                    send_files_mode: true,
                }
               }
        }
    })
}

#[derive(Props, Clone, PartialEq)]
struct ChatsToSelectProps {
    storage_controller: Signal<StorageController>,
}

#[allow(non_snake_case)]
fn ChatsToSelect(props: ChatsToSelectProps) -> Element {
    let mut state = use_context::<Signal<State>>();
    let mut storage_controller = props.storage_controller.clone();

    rsx!(div {
        id: "all_chats",
        div {
            padding_top: "16px",
            padding_left: "16px",
            Label {
                text: get_local_text("files.select-chats"),
            }
        }
        {state.read().chats_sidebar().iter().cloned().map(|chat| {
            let participants = state.read().chat_participants(&chat);
            let other_participants =  state.read().remove_self(&participants);
            let user: state::Identity = other_participants.first().cloned().unwrap_or_default();
            let platform = user.platform().into();
            // todo: how to tell who is participating in a group chat if the chat has a conversation_name?
            let participants_name = match chat.conversation_name {
                Some(name) => name,
                None => State::join_usernames(&other_participants)
            };
            let is_checked = storage_controller.read().chats_selected_to_send.iter().any(|uuid| {uuid.eq(&chat.id)});
            let unwrapped_message = match chat.messages.iter().last() {Some(m) => m.inner.clone(),None => raygun::Message::default()};
            let subtext_val = match unwrapped_message.lines().iter().map(|x| x.trim()).find(|x| !x.is_empty()) {
                Some(v) => format_text(v, state.read().ui.should_transform_markdown_text(), state.read().ui.should_transform_ascii_emojis(), Some((&state.read(), &chat.id, true))),
                _ => match &unwrapped_message.attachments()[..] {
                    [] => get_local_text("sidebar.chat-new"),
                    [ file ] => file.name(),
                    _ => match participants.iter().find(|p| p.did_key()  == unwrapped_message.sender()).map(|x| x.username()) {
                        Some(name) => get_local_text_with_args("sidebar.subtext", vec![("user", name)]),
                        None => {
                            log::error!("error calculating subtext for sidebar chat");
                            // Still return default message
                            get_local_text("sidebar.chat-new")
                        }
                    }
                }
            };

            rsx!(div {
                    id: "chat-selector-to-send-files",
                    height: "80px",
                    padding: "16px",
                    display: "inline-flex",
                    Checkbox {
                        disabled: false,
                        width: "1em".to_string(),
                        height: "1em".to_string(),
                        is_checked: is_checked,
                        on_click: move |_| {
                            if is_checked {
                                storage_controller.with_mut(|f| f.chats_selected_to_send.retain(|uuid| chat.id != *uuid));
                            } else {
                                storage_controller.with_mut(|f| f.chats_selected_to_send.push(chat.id));
                            }
                        }
                    }
                    User {
                        username: participants_name,
                        subtext: subtext_val,
                        timestamp: raygun::Message::default().date(),
                        active: false,
                        user_image: rsx!(
                            div {
                                class: "chat-selector-to-send-image-group",
                                Checkbox {
                                    disabled: false,
                                    width: "1em".to_string(),
                                    height: "1em".to_string(),
                                    is_checked: is_checked,
                                    on_click: move |_| {
                                        if is_checked {
                                            storage_controller.with_mut(|f| f.chats_selected_to_send.retain(|uuid| chat.id != *uuid));
                                        } else {
                                            storage_controller.with_mut(|f| f.chats_selected_to_send.push(chat.id));
                                        }
                                    }
                                }
                                if chat.conversation_type == ConversationType::Direct {{rsx! (
                                    UserImage {
                                        platform: platform,
                                        status:  Status::from(user.identity_status()),
                                        image: user.profile_picture(),
                                        typing: false,
                                    }
                                )}} else {{rsx! (
                                    UserImageGroup {
                                        participants: build_participants(&participants),
                                        typing: false,
                                    }
                                )}}
                            }
                        ),
                        with_badge: "".to_string(),
                        onpress: move |_| {
                            if is_checked {
                                storage_controller.with_mut(|f| f.chats_selected_to_send.retain(|uuid| chat.id != *uuid));
                            } else {
                                storage_controller.with_mut(|f| f.chats_selected_to_send.push(chat.id));
                            }
                        }
                    }
                }
            )
        })},
    })
}
