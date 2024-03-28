use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::PathBuf,
};

use arboard::Clipboard;
use dioxus::prelude::{EventHandler, *};

mod coroutines;
mod effects;

use common::state::{
    pending_message::{FileLocation, PendingMessage},
    Action, Identity, State,
};
use common::{
    icons::outline::Shape as Icon,
    icons::Icon as IconElement,
    language::get_local_text_with_args,
    state::{ui::EmojiDestination, ToastNotification},
};
use kit::{
    components::{
        context_menu::{ContextItem, ContextMenu},
        indicator::Status,
        message::{Message, Order, ReactionAdapter},
        message_group::MessageGroup,
        message_reply::MessageReply,
        user_image::UserImage,
    },
    elements::{
        loader::Loader,
        tooltip::{ArrowPosition, Tooltip},
    },
};

use common::language::get_local_text;
use rfd::FileDialog;

use uuid::Uuid;
use warp::{
    constellation::file::File,
    crypto::DID,
    multipass::identity::IdentityStatus,
    raygun::{self},
};

use tracing::log;

use crate::{
    components::emoji_group::EmojiGroup,
    layouts::{
        chats::{
            data::{self, ChatData, MessagesToEdit, MessagesToSend, ScrollBtn},
            scripts,
        },
        storage::files_layout::file_preview::open_file_preview_modal,
    },
    utils::format_timestamp::format_timestamp_timeago,
};

#[allow(clippy::large_enum_variant)]
pub enum MessagesCommand {
    React((DID, raygun::Message, String)),
    DeleteMessage {
        conv_id: Uuid,
        msg_id: Uuid,
    },
    DownloadAttachment {
        conv_id: Uuid,
        msg_id: Uuid,
        file: warp::constellation::file::File,
        file_path_to_download: PathBuf,
    },
    EditMessage {
        conv_id: Uuid,
        msg_id: Uuid,
        msg: Vec<String>,
    },
    Pin(raygun::Message),
}

pub type DownloadTracker = HashMap<Uuid, HashSet<warp::constellation::file::File>>;

#[component(no_case_check)]
pub fn get_messages(
    cx: Scope,
    quickprofile_data: UseRef<Option<(f64, f64, Identity, bool)>>,
) -> Element {
    log::trace!("get_messages");
    use_shared_state_provider(cx, || -> DownloadTracker { HashMap::new() });
    let state = use_shared_state::<State>(cx)?;
    let chat_data = use_shared_state::<ChatData>(cx)?;
    let scroll_btn = use_shared_state::<ScrollBtn>(cx)?;
    let pending_downloads = use_shared_state::<DownloadTracker>(cx)?;

    let eval = use_eval(cx);
    let ch = coroutines::handle_msg_scroll(cx, eval, chat_data, scroll_btn);
    let fetch_later_ch = coroutines::fetch_later_ch(cx, chat_data, scroll_btn);
    effects::init_msg_scroll(cx, chat_data, eval, ch);

    // used by child Elements via use_coroutine_handle
    let _ch = coroutines::handle_warp_commands(cx, state, pending_downloads);

    let active_chat_id = chat_data.read().active_chat.id();
    // used by the intersection observer to terminate itself.
    let chat_key = chat_data.read().active_chat.key().to_string();
    let chat_behavior = chat_data.read().get_chat_behavior(active_chat_id);
    let msg_container_end =
        if matches!(chat_behavior.on_scroll_top, data::ScrollBehavior::FetchMore) {
            rsx!(div {
                class: "fetching",
                p {
                    Loader {
                        spinning: true
                    },
                    get_local_text("messages.fetching")
                }
            })
        } else {
            rsx!(
                div {
                    // key: "encrypted-notification-0001",
                    class: "msg-container-end",
                    aria_label: "messages-secured-alert",
                    p {
                        IconElement {
                            icon:  Icon::LockClosed,
                        },
                        get_local_text("messages.msg-banner")
                    }
                }
            )
        };

    cx.render(rsx!(
        div {
            id: "messages",
            // this is a hack to deal with the limitations of the message paging. On the first page, if a message comes in while the page
            // is scrolled up, it won't be displayed when the user scrolls back down. need to trigger a "fetch more" response. 
            onscroll: move |_| {
                to_owned![eval, active_chat_id, chat_data, fetch_later_ch, scroll_btn];
                async move {
                    let behavior = chat_data.read().get_chat_behavior(active_chat_id);
                    if behavior.on_scroll_end != data::ScrollBehavior::DoNothing {
                        return;
                    }

                    if let Ok(val) = eval(scripts::READ_SCROLL) {
                        if let Ok(result) = val.join().await {
                            let scroll = result.as_i64().unwrap_or_default();
                            chat_data.write_silent().set_scroll_value(active_chat_id, scroll);

                            if scroll < -100  && !scroll_btn.read().get(active_chat_id) {
                                log::debug!("triggering scroll button");
                                scroll_btn.write().set(active_chat_id);
                            } else if scroll == 0 && scroll_btn.read().get(active_chat_id) {
                                if !behavior.message_received  {
                                    scroll_btn.write().clear(active_chat_id);
                                } else {
                                     fetch_later_ch.send(active_chat_id);
                                }
                            }
                        }
                    }
                }
            },
            // used by the intersection observer to terminate itself
            div {
                id: "{chat_key}",
                hidden: true,
            },
            span {
                rsx!(
                    msg_container_end,
                    loop_over_message_groups {
                        groups: data::create_message_groups(chat_data.read().active_chat.my_id(), chat_data.read().active_chat.other_participants(), chat_data.read().active_chat.messages()),
                        active_chat_id: chat_data.read().active_chat.id(),
                        on_context_menu_action: move |(e, mut id): (Event<MouseData>, Identity)| {
                            let own = state.read().get_own_identity().did_key().eq(&id.did_key());
                            if own {
                                id.set_identity_status(IdentityStatus::Online);
                            };
                            quickprofile_data.set(Some((e.page_coordinates().x, e.page_coordinates().y, id.clone(), own)));
                        }
                    },
                    render_pending_messages_listener {
                        on_context_menu_action: move |(e, mut id): (Event<MouseData>, Identity)| {
                            let own = state.read().get_own_identity().did_key().eq(&id.did_key());
                            if own {
                                id.set_identity_status(IdentityStatus::Online);
                            };
                            quickprofile_data.set(Some((e.page_coordinates().x, e.page_coordinates().y, id.clone(), own)));
                        }
                    }
                )
            }
        }
    ))
}

#[derive(Props)]
pub struct AllMessageGroupsProps<'a> {
    groups: Vec<data::MessageGroup>,
    active_chat_id: Uuid,
    on_context_menu_action: EventHandler<'a, (Event<MouseData>, Identity)>,
}

// attempting to move the contents of this function into the above rsx! macro causes an error: cannot return vale referencing
// temporary location
pub fn loop_over_message_groups<'a>(cx: Scope<'a, AllMessageGroupsProps<'a>>) -> Element<'a> {
    log::trace!("render message groups");
    cx.render(rsx!(cx.props.groups.iter().map(|_group| {
        rsx!(render_message_group {
            group: _group,
            active_chat_id: cx.props.active_chat_id,
            on_context_menu_action: move |e| cx.props.on_context_menu_action.call(e)
        },)
    })))
}

#[derive(Props)]
struct MessageGroupProps<'a> {
    group: &'a data::MessageGroup,
    active_chat_id: Uuid,
    on_context_menu_action: EventHandler<'a, (Event<MouseData>, Identity)>,
    pending: Option<bool>,
}

fn render_message_group<'a>(cx: Scope<'a, MessageGroupProps<'a>>) -> Element<'a> {
    let state = use_shared_state::<State>(cx)?;

    let MessageGroupProps {
        group,
        active_chat_id: _,
        on_context_menu_action: _,
        pending: _,
    } = cx.props;

    let messages = &group.messages;
    let last_message_date = messages
        .last()
        .as_ref()
        .map(|x| x.message.inner.date())
        .unwrap_or_default();
    let sender = state.read().get_identity(&group.sender).unwrap_or_default();
    let blocked = group.remote && state.read().is_blocked(&sender.did_key());
    let show_blocked = use_state(cx, || false);

    let blocked_element = if blocked {
        if !show_blocked.get() {
            return cx.render(rsx!(
                div {
                    class: "blocked-container",
                    p {
                        get_local_text_with_args("messages.blocked", vec![("amount", messages.len())])
                    },
                    p {
                        style: "white-space: pre",
                        " - "
                    },
                    div {
                        class: "pressable",
                        onclick: move |_| {
                            show_blocked.set(true);
                        },
                        get_local_text("messages.view")
                    }
                }
            ));
        }
        cx.render(rsx!(
            div {
                class: "blocked-container",
                p {
                    get_local_text_with_args("messages.blocked", vec![("amount", messages.len())])
                },
                p {
                    style: "white-space: pre",
                    " - "
                },
                div {
                    class: "pressable",
                    onclick: move |_| {
                        show_blocked.set(false);
                    },
                    get_local_text("messages.hide")
                }
            }
        ))
    } else {
        Option::None
    };
    let sender_clone = sender.clone();
    let sender_name = if sender.username().is_empty() {
        get_local_text("uplink.unknown")
    } else {
        sender.username()
    };
    let active_language = &state.read().settings.language_id();

    let mut sender_status = sender.identity_status().into();
    if !group.remote && sender_status == Status::Offline {
        sender_status = Status::Online;
    }

    cx.render(rsx!(
        blocked_element,
        MessageGroup {
            user_image: render!(UserImage {
                image: sender.profile_picture(),
                platform: sender.platform().into(),
                status: sender_status,
                on_press: move |e| {
                    cx.props.on_context_menu_action.call((e, sender.to_owned()));
                },
                oncontextmenu: move |e| {
                    cx.props
                        .on_context_menu_action
                        .call((e, sender_clone.to_owned()));
                }
            }),
            timestamp: format_timestamp_timeago(last_message_date, active_language),
            sender: sender_name.clone(),
            remote: group.remote,
            children: cx.render(rsx!(wrap_messages_in_context_menu {
                messages: &group.messages,
                active_chat_id: cx.props.active_chat_id,
                is_remote: group.remote,
                pending: cx.props.pending.unwrap_or_default()
            }))
        },
    ))
}

#[derive(Props)]
struct MessagesProps<'a> {
    messages: &'a Vec<data::MessageGroupMsg>,
    active_chat_id: Uuid,
    is_remote: bool,
    pending: bool,
}
fn wrap_messages_in_context_menu<'a>(cx: Scope<'a, MessagesProps<'a>>) -> Element<'a> {
    let state = use_shared_state::<State>(cx)?;
    let edit_msg = use_shared_state::<MessagesToEdit>(cx)?;
    // see comment in ContextMenu about this variable.
    let reacting_to: &UseState<Option<Uuid>> = use_state(cx, || None);

    let emoji_selector_extension = "emoji_selector";

    let has_extension = state
        .read()
        .ui
        .extensions
        .enabled_extension(emoji_selector_extension);

    let ch = use_coroutine_handle::<MessagesCommand>(cx)?;
    cx.render(rsx!(cx.props.messages.iter().map(|grouped_message| {
        let message = &grouped_message.message;
        let sender_is_self = message.inner.sender() == state.read().did_key();

        // WARNING: these keys are required to prevent a bug with the context menu, which manifests when deleting messages.
        let is_editing = edit_msg
            .read().edit
            .map(|id| !cx.props.is_remote && (id == message.inner.id()))
            .unwrap_or(false);
        let message_key = format!("{}-{:?}", &message.key, is_editing);
        let message_id = format!("{}-{:?}", &message.inner.id(), is_editing);
        let context_key = format!("message-{}", &message_id);
        let msg_uuid = message.inner.id();
        let conversation_id = message.inner.conversation_id();

        if cx.props.pending {
            return rsx!(render_message {
                message: grouped_message,
                is_remote: cx.props.is_remote,
                message_key: message_key,
                edit_msg: edit_msg,
                pending: cx.props.pending
            });
        }

        // todo: add onblur event
        rsx!(ContextMenu {
            key: "{context_key}",
            id: msg_uuid.to_string(),
            devmode: state.read().configuration.developer.developer_mode,
            children: cx.render(rsx!(render_message {
                message: grouped_message,
                is_remote: cx.props.is_remote,
                message_key: message_key,
                edit_msg: edit_msg,
                pending: cx.props.pending
            })),
            items: cx.render(rsx!(
                ContextItem {
                    text: "Emoji Group".into(),
                    EmojiGroup {
                        onselect: move |emoji: String| {
                            log::trace!("reacting with emoji: {}", emoji);
                            ch.send(MessagesCommand::React((state.read().did_key(), message.inner.clone(), emoji)));
                        },
                        apply_to: EmojiDestination::Message(conversation_id, msg_uuid),
                    }
                },
                ContextItem {
                    icon: Icon::Pin,
                    aria_label: "messages-pin".into(),
                    text: if message.inner.pinned() {get_local_text("messages.unpin")} else {get_local_text("messages.pin")},
                    onpress: move |_| {
                        log::trace!("pinning message: {}", message.inner.id());
                        if state.read().reached_max_pinned(&message.inner.conversation_id()) {
                            state.write().mutate(Action::AddToastNotification(ToastNotification::init(
                                "".into(),
                                get_local_text("messages.pinned-max"),
                                None,
                                3,
                            )));
                        } else {
                            ch.send(MessagesCommand::Pin(message.inner.clone()));
                        }
                    }
                },
                ContextItem {
                    icon: Icon::ArrowLongLeft,
                    aria_label: "messages-reply".into(),
                    text: get_local_text("messages.reply"),
                    onpress: move |_| {
                        state
                            .write()
                            .mutate(Action::StartReplying(&cx.props.active_chat_id, message));
                    }
                },
                ContextItem {
                    icon: Icon::FaceSmile,
                    aria_label: "messages-react".into(),
                    text: get_local_text("messages.react"),
                    disabled: !has_extension,
                    tooltip:  if has_extension {
                        cx.render(rsx!(()))
                    } else {
                        cx.render(rsx!(Tooltip {
                            arrow_position: ArrowPosition::Top,
                            text: get_local_text("messages.missing-emoji-picker")
                        }))
                    },
                    onpress: move |_| {
                        if has_extension {
                            state.write().ui.ignore_focus = true;
                            state.write().mutate(Action::SetEmojiDestination(
                                // Tells the default emojipicker where to place the next emoji
                                Some(
                                    common::state::ui::EmojiDestination::Message(conversation_id, msg_uuid)
                                )
                            ));
                            reacting_to.set(Some(msg_uuid));
                            state.write().mutate(Action::SetEmojiPickerVisible(true));
                        }
                    }
                },
                ContextItem {
                    icon: Icon::ClipboardDocument,
                    aria_label: "messages-copy".into(),
                    text: get_local_text("uplink.copy-text"),
                    onpress: move |_| {
                        let text = message.inner.lines().join("\n");
                        match Clipboard::new() {
                            Ok(mut c) => {
                                if let Err(e) = c.set_text(text) {
                                    log::warn!("Unable to set text to clipboard: {e}");
                                }
                            }
                            Err(e) => {
                                log::warn!("Unable to create clipboard reference: {e}");
                            }
                        };
                    }
                },
                ContextItem {
                    icon: Icon::Pencil,
                    aria_label: "messages-edit".into(),
                    text: get_local_text("messages.edit"),
                    should_render: !cx.props.is_remote
                        && edit_msg.read().edit.map(|id| id != msg_uuid).unwrap_or(true),
                    onpress: move |_| {
                        edit_msg.write().edit = Some(msg_uuid);
                        state.write().ui.ignore_focus = true;
                    }
                },
                ContextItem {
                    icon: Icon::Pencil,
                    aria_label: "messages-cancel-edit".into(),
                    text: get_local_text("messages.cancel-edit"),
                    should_render: !cx.props.is_remote
                        && edit_msg.read().edit.map(|id| id == msg_uuid).unwrap_or(false),
                    onpress: move |_| {
                        edit_msg.write().edit = None;
                        state.write().ui.ignore_focus = false;
                    }
                },
                ContextItem {
                    icon: Icon::Trash,
                    danger: true,
                    aria_label: "messages-delete".into(),
                    text: get_local_text("uplink.delete"),
                    should_render: sender_is_self,
                    onpress: move |_| {
                        ch.send(MessagesCommand::DeleteMessage {
                            conv_id: message.inner.conversation_id(),
                            msg_id: message.inner.id(),
                        });
                    }
                },
            )) // end of context menu items
        }) // end context menu
    }))) // end outer cx.render
}

#[derive(Props)]
struct MessageProps<'a> {
    message: &'a data::MessageGroupMsg,
    is_remote: bool,
    message_key: String,
    edit_msg: &'a UseSharedState<MessagesToEdit>,
    pending: bool,
}
fn render_message<'a>(cx: Scope<'a, MessageProps<'a>>) -> Element<'a> {
    //log::trace!("render message {}", &cx.props.message.message.key);
    let state = use_shared_state::<State>(cx)?;
    let chat_data = use_shared_state::<ChatData>(cx)?;

    let pending_downloads = use_shared_state::<DownloadTracker>(cx)?;
    let user_did = state.read().did_key();

    // todo: why?
    #[cfg(not(target_os = "macos"))]
    let _eval = use_eval(cx);

    let ch = use_coroutine_handle::<MessagesCommand>(cx)?;

    let MessageProps {
        message: grouped_message,
        is_remote: _,
        message_key,
        edit_msg,
        pending: _,
    } = cx.props;
    let message = &grouped_message.message;
    let is_editing = edit_msg
        .read()
        .edit
        .map(|id| !cx.props.is_remote && (id == message.inner.id()))
        .unwrap_or(false);

    let reactions_list: Vec<ReactionAdapter> = message
        .inner
        .reactions()
        .iter()
        .map(|(emoji, users)| {
            let user_names: Vec<String> = users
                .iter()
                .filter_map(|id| state.read().get_identity(id).map(|x| x.username()))
                .collect();
            ReactionAdapter {
                emoji: emoji.into(),
                reaction_count: users.len(),
                self_reacted: users.iter().any(|x| x == &user_did),
                alt: user_names.join(", "),
            }
        })
        .collect();

    let user_did_2 = user_did.clone();

    let pending_uploads = grouped_message.file_progress.as_ref();
    let render_markdown = state.read().ui.should_transform_markdown_text();
    let should_transform_ascii_emojis = state.read().ui.should_transform_ascii_emojis();
    let msg_lines = message.inner.lines().join("\n");

    let is_mention = message.clone().is_mention_self(&user_did);
    let preview_file_in_the_message: &UseState<(bool, Option<File>)> =
        use_state(cx, || (false, None));

    let mut reply_user = Identity::default();
    if let Some(info) = &message.in_reply_to {
        reply_user = state.read().get_identity(&info.2).unwrap_or_default();
    }
    let to_send = use_shared_state::<MessagesToSend>(cx)?;

    cx.render(rsx!(
        div {
            class: "msg-wrapper",
            preview_file_in_the_message.0.then(|| {
                if preview_file_in_the_message.1.is_none() {
                    preview_file_in_the_message.set((false, None));
                }
                let file = preview_file_in_the_message.1.clone().unwrap();
                let file2 = file.clone();
                rsx!(open_file_preview_modal {
                    on_dismiss: |_| {
                        preview_file_in_the_message.set((false, None));
                    },
                    on_download: move |temp_path: Option<PathBuf>| {
                        let conv_id = message.inner.conversation_id();
                        if let Some(path) = temp_path {
                            if !path.exists() {
                                log::info!("downloading file in temp directory: {:?}", path.clone());
                                ch.send(MessagesCommand::DownloadAttachment {
                                    conv_id,
                                    msg_id: message.inner.id(),
                                    file: file2.clone(),
                                    file_path_to_download: path,
                                })
                            }
                        } else {
                            download_file(&file2, message.inner.conversation_id(), message.inner.id(), pending_downloads, ch);
                        }
                    },
                    file: file.clone()
                }
            )
            }),
            message.in_reply_to.as_ref().map(|(other_msg, other_msg_attachments, sender_did)| rsx!(
            MessageReply {
                    key: "reply-{message_key}",
                    with_text: other_msg.to_string(),
                    with_attachments: other_msg_attachments.clone(),
                    // This remote should be true only if the reply itself is remove, not the message being replied to.
                    remote: cx.props.is_remote,
                    remote_message: cx.props.is_remote,
                    sender_did: sender_did.clone(),
                    replier_did: user_did_2.clone(),
                    markdown: render_markdown,
                    transform_ascii_emojis: should_transform_ascii_emojis,
                    state: state,
                    chat: chat_data.read().active_chat.id(),
                    user_image: cx.render(rsx!(UserImage {
                        loading: false,
                        platform: reply_user.platform().into(),
                        status: reply_user.identity_status().into(),
                        image: reply_user.profile_picture(),
                    }))
                }
            )),
            Message {
                id: message_key.clone(),
                key: "{message_key}",
                editing: is_editing,
                remote: cx.props.is_remote,
                with_text: msg_lines,
                is_mention: is_mention,
                reactions: reactions_list,
                state: state,
                chat: chat_data.read().active_chat.id(),
                order: if grouped_message.is_first { Order::First } else if grouped_message.is_last { Order::Last } else { Order::Middle },
                attachments: message
                .inner
                .attachments(),
                attachments_pending_download: pending_downloads.read().get(&message.inner.conversation_id()).cloned(),
                on_click_reaction: move |emoji: String| {
                    ch.send(MessagesCommand::React((user_did.clone(), message.inner.clone(), emoji)));
                },
                pending: cx.props.pending,
                pinned: message.inner.pinned(),
                attachments_pending_uploads: pending_uploads,
                on_resend: move |(txt, file): (Option<String>, FileLocation)|{
                    match txt.clone() {
                        Some(_) => {
                            state
                            .write()
                            .decrement_outgoing_messages(chat_data.read().active_chat.id(), message.inner.id());
                        },
                        None => {
                            state
                            .write()
                            .remove_outgoing_attachment(chat_data.read().active_chat.id(), message.inner.id(), file.clone());
                        },
                    }
                    to_send.with_mut(|s|s.messages_to_send.push((txt, vec![file])));
                },
                on_delete: move |file| {
                    state
                    .write()
                    .remove_outgoing_attachment(chat_data.read().active_chat.id(), message.inner.id(), file);
                },
                parse_markdown: render_markdown,
                transform_ascii_emojis: should_transform_ascii_emojis,
                on_download: move |(file, temp_dir): (warp::constellation::file::File, Option<PathBuf>)| {
                    if temp_dir.is_some() {
                        preview_file_in_the_message.set((true, Some(file.clone())));
                    } else {
                        download_file(&file, message.inner.conversation_id(), message.inner.id(), pending_downloads, ch);
                    }
                },
                on_edit: move |update: String| {
                    edit_msg.write().edit = None;
                    state.write().ui.ignore_focus = false;
                    let msg = update.split('\n').map(|x| x.to_string()).collect::<Vec<String>>();
                    if  message.inner.lines() == msg || !msg.iter().any(|x| !x.trim().is_empty()) {
                        return;
                    }
                    ch.send(MessagesCommand::EditMessage { conv_id: message.inner.conversation_id(), msg_id: message.inner.id(), msg})
                }
            },
            script {
                r#"
                (() => {{
                    Prism.highlightAll();
                }})();
                "#
            } // Highlights Pre blocks
        }
    ))
}

#[derive(Props)]
pub struct PendingMessagesListenerProps<'a> {
    on_context_menu_action: EventHandler<'a, (Event<MouseData>, Identity)>,
}

//The component that listens for upload events
pub fn render_pending_messages_listener<'a>(
    cx: Scope<'a, PendingMessagesListenerProps>,
) -> Element<'a> {
    let state = use_shared_state::<State>(cx)?;
    state.write_silent().scope_ids.pending_message_component = Some(cx.scope_id().0);
    let chat = match state.read().get_active_chat() {
        Some(c) => c,
        None => return cx.render(rsx!(())),
    };
    cx.render(rsx!(pending_wrapper {
        msg: chat.pending_outgoing_messages,
        on_context_menu_action: move |e| cx.props.on_context_menu_action.call(e)
    }))
}

#[derive(Props)]
struct PendingWrapperProps<'a> {
    msg: Vec<PendingMessage>,
    on_context_menu_action: EventHandler<'a, (Event<MouseData>, Identity)>,
}

//We need to do it this way due to reference ownership
fn pending_wrapper<'a>(cx: Scope<'a, PendingWrapperProps>) -> Element<'a> {
    let chat_data = use_shared_state::<ChatData>(cx)?;
    let data = chat_data.read();
    cx.render(rsx!(render_pending_messages {
        pending_outgoing_message: data::pending_group_messages(
            &cx.props.msg,
            data.active_chat.other_participants(),
            data.active_chat.my_id(),
        ),
        active: data.active_chat.id(),
        on_context_menu_action: move |e| cx.props.on_context_menu_action.call(e)
    }))
}

#[derive(Props)]
struct PendingMessagesProps<'a> {
    #[props(!optional)]
    pending_outgoing_message: Option<data::MessageGroup>,
    active: Uuid,
    on_context_menu_action: EventHandler<'a, (Event<MouseData>, Identity)>,
}

fn render_pending_messages<'a>(cx: Scope<'a, PendingMessagesProps>) -> Element<'a> {
    cx.render(rsx!(cx.props.pending_outgoing_message.as_ref().map(
        |group| {
            rsx!(render_message_group {
                group: group,
                active_chat_id: cx.props.active,
                on_context_menu_action: move |e| cx.props.on_context_menu_action.call(e),
                pending: true
            },)
        }
    )))
}

fn download_file(
    file: &warp::constellation::file::File,
    conv_id: Uuid,
    msg_id: Uuid,
    pending_downloads: &UseSharedState<HashMap<Uuid, HashSet<File>>>,
    ch: &Coroutine<MessagesCommand>,
) {
    let file_name = file.name();
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

    if let Some(file_path_to_download) = FileDialog::new()
        .set_directory(dirs::download_dir().unwrap_or_default())
        .set_file_name(file_stem)
        .add_filter("", &[&file_extension])
        .save_file()
    {
        if !pending_downloads.read().contains_key(&conv_id) {
            pending_downloads.write().insert(conv_id, HashSet::new());
        }
        pending_downloads
            .write()
            .get_mut(&conv_id)
            .map(|conv| conv.insert(file.clone()));
        ch.send(MessagesCommand::DownloadAttachment {
            conv_id,
            msg_id,
            file: file.clone(),
            file_path_to_download,
        })
    }
}
