use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use common::{
    icons::{self},
    language::get_local_text,
    state::{Action, Identity, State},
    warp_runner::{RayGunCmd, WarpCmd},
    MAX_FILES_PER_MESSAGE, STATIC_ARGS, WARP_CMD_CH,
};
use dioxus::prelude::*;
use futures::{channel::oneshot, StreamExt};
use kit::layout::modal::Modal;
use kit::{
    components::{
        indicator::{Platform, Status},
        user_image::UserImage,
    },
    elements::{
        button::Button,
        tooltip::{ArrowPosition, Tooltip},
        Appearance,
    },
    layout::chatbar::{Chatbar, Reply},
};
use rfd::FileDialog;
use uuid::Uuid;
use warp::{
    crypto::DID,
    logging::tracing::log,
    raygun::{self, Location},
};

const MAX_CHARS_LIMIT: usize = 1024;

use crate::{
    components::{
        chat::compose::context_file_location::FileLocationContext, files::attachments::Attachments,
        paste_files_with_shortcut,
    },
    layouts::storage::FilesLayout,
    utils::{
        build_user_from_identity,
        clipboard::clipboard_data::{
            check_if_there_is_file_or_string_in_clipboard, ClipboardDataType,
        },
    },
};

type ChatInput = (Vec<String>, Uuid, Option<Uuid>, Option<Uuid>);

#[derive(Eq, PartialEq)]
enum TypingIndicator {
    // reset the typing indicator timer
    Typing(Uuid),
    // clears the typing indicator, ensuring the indicator
    // will not be refreshed
    NotTyping,
    // resend the typing indicator
    Refresh(Uuid),
}

#[derive(Clone)]
struct TypingInfo {
    pub chat_id: Uuid,
    pub last_update: Instant,
}

// todo: display loading indicator if sending a message that takes a long time to upload attachments
pub fn get_chatbar<'a>(cx: &'a Scoped<'a, super::ComposeProps>) -> Element<'a> {
    log::trace!("get_chatbar");
    let state = use_shared_state::<State>(cx)?;
    state.write_silent().scope_ids.chatbar = Some(cx.scope_id().0);
    let data = &cx.props.data;
    let is_loading = data.is_none();
    let active_chat_id = data.as_ref().map(|d| d.active_chat.id);
    let chat_id = data
        .as_ref()
        .map(|data| data.active_chat.id)
        .unwrap_or(Uuid::nil());
    let can_send = use_state(cx, || state.read().active_chat_has_draft());
    let update_script = use_state(cx, String::new);
    let upload_button_menu_uuid = &*cx.use_hook(|| Uuid::new_v4().to_string());
    let show_storage_modal = use_state(cx, || false);

    let update_send = move || {
        let valid = state.read().active_chat_has_draft()
            || !state
                .read()
                .get_active_chat()
                .map(|f| f.files_attached_to_send)
                .unwrap_or_default()
                .is_empty();
        if !can_send.get().eq(&valid) {
            can_send.set(valid);
        }
    };
    update_send();

    let mut files_attached = state
        .read()
        .get_active_chat()
        .map(|f| f.files_attached_to_send)
        .unwrap_or_default();

    if files_attached.len() > MAX_FILES_PER_MESSAGE {
        files_attached.truncate(MAX_FILES_PER_MESSAGE);
        state
            .write()
            .mutate(Action::SetChatAttachments(chat_id, files_attached));
    }

    // used to render the typing indicator
    // for now it doesn't quite work for group messages
    let my_id = state.read().did_key();
    let users_typing: Vec<DID> = data
        .as_ref()
        .map(|data| {
            data.active_chat
                .typing_indicator
                .iter()
                .filter(|(did, _)| *did != &my_id)
                .map(|(did, _)| did.clone())
                .collect()
        })
        .unwrap_or_default();
    let users_typing = state.read().get_identities(&users_typing);

    let msg_ch = use_coroutine(cx, |mut rx: UnboundedReceiver<ChatInput>| {
        to_owned![state];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some((msg, conv_id, ui_msg_id, reply)) = rx.next().await {
                let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                let attachments = state
                    .read()
                    .get_active_chat()
                    .map(|f| f.files_attached_to_send)
                    .unwrap_or_default();
                let msg_clone = msg.clone();
                let cmd = match reply {
                    Some(reply_to) => RayGunCmd::Reply {
                        conv_id,
                        reply_to,
                        msg,
                        attachments,
                        rsp: tx,
                    },
                    None => RayGunCmd::SendMessage {
                        conv_id,
                        msg,
                        attachments,
                        ui_msg_id,
                        rsp: tx,
                    },
                };
                let attachments = state
                    .read()
                    .get_active_chat()
                    .map(|f| f.files_attached_to_send)
                    .unwrap_or_default();
                state
                    .write_silent()
                    .mutate(Action::ClearChatAttachments(conv_id));
                let attachment_files: Vec<String> = attachments
                    .iter()
                    .map(|p| match p {
                        Location::Disk { path } => {
                            if let Some(name) = path
                                .file_name()
                                .map(|ostr| ostr.to_str().unwrap_or_default())
                            {
                                return name.to_string();
                            }
                            String::new()
                        }
                        Location::Constellation { path } => {
                            if let Some(name) = PathBuf::from(path)
                                .file_name()
                                .map(|ostr| ostr.to_str().unwrap_or_default())
                            {
                                return name.to_string();
                            }
                            String::new()
                        }
                    })
                    .collect();
                if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(cmd)) {
                    log::error!("failed to send warp command: {}", e);
                    state.write().decrement_outgoing_messages(
                        conv_id,
                        msg_clone,
                        attachment_files,
                        ui_msg_id,
                    );
                    continue;
                }

                let rsp = rx.await.expect("command canceled");
                if let Err(e) = rsp {
                    log::error!("failed to send message: {}", e);
                    state.write().decrement_outgoing_messages(
                        conv_id,
                        msg_clone,
                        attachment_files,
                        ui_msg_id,
                    );
                }
            }
        }
    });

    // typing indicator notes
    // consider side A, the local side, and side B, the remote side
    // side A -> (typing indicator) -> side B
    // side B removes the typing indicator after a timeout
    // side A doesn't want to send too many typing indicators, say once every 4-5 seconds
    // should we consider matching the timeout with the send frequency so we can closely match if a person is straight up typing for 5 mins straight.

    // tracks if the local participant is typing
    // re-sends typing indicator in response to the Refresh command
    let local_typing_ch = use_coroutine(cx, |mut rx: UnboundedReceiver<TypingIndicator>| {
        // to_owned![];
        async move {
            let mut typing_info: Option<TypingInfo> = None;
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();

            let send_typing_indicator = |conv_id| async move {
                let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                let event = raygun::MessageEvent::Typing;
                if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::SendEvent {
                    conv_id,
                    event,
                    rsp: tx,
                })) {
                    log::error!("failed to send warp command: {}", e);
                    // return from the closure
                    return;
                }
                let rsp = rx.await.expect("command canceled");
                if let Err(e) = rsp {
                    log::error!("failed to send typing indicator: {}", e);
                }
            };

            while let Some(indicator) = rx.next().await {
                match indicator {
                    TypingIndicator::Typing(chat_id) => {
                        // if typing_info was none or the chat id changed, send the indicator immediately
                        let should_send_indicator = match typing_info {
                            None => true,
                            Some(info) => info.chat_id != chat_id,
                        };
                        if should_send_indicator {
                            send_typing_indicator.clone()(chat_id).await;
                        }
                        typing_info = Some(TypingInfo {
                            chat_id,
                            last_update: Instant::now(),
                        });
                    }
                    TypingIndicator::NotTyping => {
                        typing_info = None;
                    }
                    TypingIndicator::Refresh(conv_id) => {
                        let info = match &typing_info {
                            Some(i) => i.clone(),
                            None => continue,
                        };
                        if info.chat_id != conv_id {
                            typing_info = None;
                            continue;
                        }
                        // todo: verify duration for timeout
                        let now = Instant::now();
                        if now - info.last_update
                            <= (Duration::from_secs(STATIC_ARGS.typing_indicator_timeout)
                                - Duration::from_millis(500))
                        {
                            send_typing_indicator.clone()(conv_id).await;
                        }
                    }
                }
            }
        }
    });

    // drives the sending of TypingIndicator
    let local_typing_ch1 = local_typing_ch.clone();
    let enable_paste_shortcut = use_ref(cx, || true);

    use_future(cx, (), |_| {
        to_owned![enable_paste_shortcut];
        async move {
            loop {
                let clipboard_data_type = tokio::task::spawn_blocking(|| {
                    check_if_there_is_file_or_string_in_clipboard()
                        .unwrap_or(ClipboardDataType::String)
                })
                .await
                .expect("Should succeed");
                match clipboard_data_type {
                    ClipboardDataType::File => {
                        if !*enable_paste_shortcut.read() {
                            enable_paste_shortcut.with_mut(|i| *i = true);
                        }
                    }
                    _ => {
                        if *enable_paste_shortcut.read() {
                            enable_paste_shortcut.with_mut(|i| *i = false);
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    });

    use_future(cx, &active_chat_id, |current_chat| async move {
        loop {
            tokio::time::sleep(Duration::from_secs(STATIC_ARGS.typing_indicator_refresh)).await;
            if let Some(c) = current_chat {
                local_typing_ch1.send(TypingIndicator::Refresh(c));
            }
        }
    });

    let msg_valid = |msg: &[String]| {
        (!msg.is_empty() && msg.iter().any(|line| !line.trim().is_empty()))
            || !state
                .read()
                .get_active_chat()
                .map(|f| f.files_attached_to_send)
                .unwrap_or_default()
                .is_empty()
    };

    let submit_fn = move || {
        local_typing_ch.send(TypingIndicator::NotTyping);

        let files_to_upload = state
            .read()
            .get_active_chat()
            .as_ref()
            .map(|d| d.files_attached_to_send.clone())
            .unwrap_or_default();

        let msg = state
            .read()
            .get_active_chat()
            .as_ref()
            .and_then(|d| d.draft.clone())
            .unwrap_or_default()
            .lines()
            .map(|x| x.trim_end().to_string())
            .collect::<Vec<String>>();

        if let Some(id) = active_chat_id {
            state
                .write()
                .mutate(Action::SetChatDraft(id, String::new()));
        }

        if !msg_valid(&msg) {
            return;
        }
        let id = match active_chat_id {
            Some(i) => i,
            None => return,
        };
        can_send.set(false);
        if STATIC_ARGS.use_mock {
            state.write().mutate(Action::MockSend(id, msg));
        } else {
            let replying_to = state.read().chats().get_replying_to();
            if replying_to.is_some() {
                state.write().mutate(Action::CancelReply(id));
            }
            let ui_id = state
                .write()
                .increment_outgoing_messages(msg.clone(), &files_to_upload);
            msg_ch.send((msg, id, ui_id, replying_to));
        }
    };
    let id = match active_chat_id {
        Some(i) => i,
        None => uuid::Uuid::new_v4(),
    };

    let extensions = &state.read().ui.extensions;
    let ext_renders = extensions
        .values()
        .filter(|(_, ext)| ext.details().location == extensions::Location::Chatbar)
        .map(|(_, ext)| ext.render(cx.scope))
        .collect::<Vec<_>>();

    let disabled = !state.read().can_use_active_chat();
    let error = use_state(cx, || (false, id));
    let value_chatbar = state
        .read()
        .get_active_chat()
        .as_ref()
        .and_then(|d| d.draft.clone())
        .unwrap_or_default();

    if value_chatbar.len() >= MAX_CHARS_LIMIT && !error.0 {
        error.set((true, id));
    } else if value_chatbar.len() < MAX_CHARS_LIMIT && error.0 {
        error.set((false, id));
    }

    let validate_max = move || {
        let value_chatbar = state
            .read()
            .get_active_chat()
            .as_ref()
            .and_then(|d| d.draft.clone())
            .unwrap_or_default();
        if value_chatbar.len() >= MAX_CHARS_LIMIT {
            error.set((true, id));
        } else if value_chatbar.len() < MAX_CHARS_LIMIT && error.0 {
            error.set((false, id));
        }
    };

    let typing_users: Vec<String> = users_typing.iter().map(|id| (*id).username()).collect();

    let chatbar = cx.render(rsx!(
        Chatbar {
            key: "{id}",
            id: id.to_string(),
            loading: is_loading,
            placeholder: get_local_text("messages.say-something-placeholder"),
            typing_users: typing_users,
            is_disabled: disabled,
            ignore_focus: cx.props.ignore_focus,
            onchange: move |v: String| {
                if let Some(id) = &active_chat_id {
                    state.write_silent().mutate(Action::SetChatDraft(*id, v));
                    validate_max();
                    update_send();
                    local_typing_ch.send(TypingIndicator::Typing(*id));
                }
            },
            value: state.read().get_active_chat().as_ref().and_then(|d| d.draft.clone()).unwrap_or_default(),
            onreturn: move |_| submit_fn(),
            extensions: cx.render(rsx!(for node in ext_renders { rsx!(node) })),
            controls: cx.render(
                rsx!(
                    Button {
                        icon: icons::outline::Shape::ChevronDoubleRight,
                        disabled: is_loading || disabled,
                        appearance: if * can_send.get() { Appearance::Primary } else { Appearance::Secondary },
                        aria_label: "send-message-button".into(),
                        onpress: move |_| submit_fn(),
                        tooltip: cx.render(rsx!(Tooltip {
                            arrow_position: ArrowPosition::Bottom,
                            text :get_local_text("uplink.send"),
                        })),
                    }
                ),
            ),
            with_replying_to: data.as_ref().filter(|_| !disabled).map(|data| {
                let active_chat = &data.active_chat;
                cx.render(
                    rsx!(
                        active_chat.replying_to.as_ref().map(|msg| {
                            let our_did = state.read().did_key();
                            let msg_owner = if data.my_id.did_key() == msg.sender() {
                                Some(&data.my_id)
                            } else {
                                data.other_participants.iter().find(|x| x.did_key() == msg.sender())
                            };

                            let (platform, status, profile_picture) = get_platform_and_status(msg_owner);

                            rsx!(
                                Reply {
                                    label: get_local_text("messages.replying"),
                                    remote: our_did != msg.sender(),
                                    onclose: move |_| {
                                        state.write().mutate(Action::CancelReply(active_chat.id))
                                    },
                                    attachments: msg.attachments(),
                                    message: msg.value().join("\n"),
                                    UserImage {
                                        image: profile_picture,
                                        platform: platform,
                                        status: status,
                                    },
                                }
                            )
                        })
                    ),
                )
            }).unwrap_or(None),
            with_file_upload: cx.render(
                rsx!(
                    Button {
                        icon: icons::outline::Shape::Plus,
                        disabled: is_loading || disabled,
                        aria_label: "upload-button".into(),
                        appearance: Appearance::Primary,
                        onpress: move |e: Event<MouseData>| {
                            let mouse_data = e;
                            let script = include_str!("../show_context.js")
                                .replace("UUID", upload_button_menu_uuid)
                                .replace("$PAGE_X", &mouse_data.page_coordinates().x.to_string())
                                .replace("$PAGE_Y", &mouse_data.page_coordinates().y.to_string());
                            update_script.set(script);
                        },
                        tooltip: cx.render(rsx!(
                            Tooltip {
                                arrow_position: ArrowPosition::Bottom,
                                text: get_local_text("files.upload"),
                            }
                        )),
                    }
                    FileLocationContext {
                        id: upload_button_menu_uuid,
                        update_script: update_script,
                        on_press_storage: move |_| {
                            show_storage_modal.set(true);
                        },
                        on_press_local_disk: move |_| {
                            if disabled {
                                return;
                            }
                            if let Some(new_files) = FileDialog::new()
                                .set_directory(dirs::home_dir().unwrap_or_default())
                                .pick_files()
                            {
                                let mut new_files_to_upload: Vec<_> = state.read().get_active_chat().map(|f| f.files_attached_to_send)
                                    .unwrap_or_default()
                                    .iter()
                                    .filter(|file_location| {
                                        match file_location {
                                            Location::Disk { path } => {
                                                !new_files.contains(path)
                                            },
                                            Location::Constellation { .. } => {
                                                true
                                            }
                                        }
                                    })
                                    .cloned()
                                    .collect();
                                let local_disk_files: Vec<Location> = new_files
                                    .iter()
                                    .map(|path| Location::Disk { path: path.clone() })
                                    .collect();
                                new_files_to_upload.extend(local_disk_files);
                                state.write().mutate(Action::SetChatAttachments(chat_id, new_files_to_upload));
                                update_send();
                            }
                        },
                    }
                    if *show_storage_modal.get() {
                        rsx!(
                            Modal {
                                open: *show_storage_modal.clone(),
                                onclose: move |_| show_storage_modal.set(false),
                                div {
                                    class: "modal-div-files-layout",
                                    FilesLayout {
                                        storage_files_to_chat_mode_is_active: show_storage_modal.clone(),
                                        on_files_selected_to_send: move |files_location: Vec<Location>| {
                                            let mut new_files_to_upload: Vec<_> = state.read().get_active_chat().map(|f| f.files_attached_to_send)
                                            .unwrap_or_default()
                                            .iter()
                                            .filter(|file_location| {
                                                !files_location.contains(file_location)
                                            })
                                            .cloned()
                                            .collect();
                                            new_files_to_upload.extend(files_location);
                                            state.write().mutate(Action::SetChatAttachments(chat_id, new_files_to_upload));
                                            update_send();
                                        },
                                    }
                                }
                            }
                        )
                    }
                ),
            )
        }
        error.0.then(|| rsx!(
            p {
                class: "chatbar-error-input-message",
                aria_label: "chatbar-input-error",
                format!(
                    "{} {} {} {}.",
                    get_local_text("warning-messages.maximum-of"),
                    MAX_CHARS_LIMIT,
                    get_local_text("uplink.characters"),
                    get_local_text("uplink.reached")
                )
            }
        ))
    ));

    cx.render(rsx!(
        if state.read().ui.metadata.focused && *enable_paste_shortcut.read() {
            rsx!(paste_files_with_shortcut::PasteFilesShortcut {
                on_paste: move |files_local_path: Vec<PathBuf>| {
                    if !files_local_path.is_empty() {
                        let mut new_files_to_upload: Vec<_> = state.read().get_active_chat().map(|f| f.files_attached_to_send)
                            .unwrap_or_default()
                            .iter()
                            .filter(|file_location| {
                                match file_location {
                                    Location::Disk { path } => {
                                        !files_local_path.contains(path)
                                    },
                                    Location::Constellation { .. } => {
                                        true
                                    }
                                }
                            })
                            .cloned()
                            .collect();
                        let local_disk_files: Vec<Location> = files_local_path
                            .iter()
                            .map(|path| Location::Disk { path: path.clone() })
                            .collect();
                    new_files_to_upload.extend(local_disk_files);
                    state.write().mutate(Action::SetChatAttachments(chat_id, new_files_to_upload));
                    }
                },
            })
        }
        Attachments {
            chat_id: chat_id,
            files_to_attach: state
            .read()
            .get_active_chat()
            .map(|f| f.files_attached_to_send)
            .unwrap_or_default(),
            on_remove: move |files_selected| {
                state.write().mutate(Action::SetChatAttachments(chat_id, files_selected));
                update_send();
            }
        }
        chatbar
    ))
}

fn get_platform_and_status(msg_sender: Option<&Identity>) -> (Platform, Status, String) {
    let sender = match msg_sender {
        Some(identity) => identity,
        None => return (Platform::Desktop, Status::Offline, String::new()),
    };
    let user_sender = build_user_from_identity(sender.clone());
    (user_sender.platform, user_sender.status, user_sender.photo)
}
