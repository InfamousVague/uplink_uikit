pub mod coroutines;

use std::{path::PathBuf, time::Duration};

use common::{
    icons::{self},
    language::{get_local_text, get_local_text_with_args},
    state::{
        utils::{mention_to_did_key, parse_mentions},
        Action, Identity, State,
    },
    MAX_FILES_PER_MESSAGE, STATIC_ARGS,
};
use dioxus::prelude::*;
use dioxus_html::input_data::keyboard_types::Code;
use dioxus_html::input_data::keyboard_types::Modifiers;
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
    layout::chatbar::{Chatbar, Reply, SuggestionType},
};
use once_cell::sync::Lazy;
use regex::Regex;
use rfd::FileDialog;
use uuid::Uuid;
use warp::{crypto::DID, raygun::Location};

use tracing::log;

const MAX_CHARS_LIMIT: usize = 1024;
pub static EMOJI_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(":[^:]{2,}:?$").unwrap());
pub static TAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("@[^@ ]{2,} ?$").unwrap());
use super::context_menus::FileLocation as FileLocationContext;
use crate::{
    components::{files::attachments::Attachments, shortcuts},
    layouts::{
        chats::{
            data::{
                ChatData, ChatProps, MessagesToEdit, MessagesToSend, MsgChInput, ScrollBtn,
                TypingIndicator,
            },
            scripts::SHOW_CONTEXT,
        },
        storage::send_files_layout::{modal::SendFilesLayoutModal, SendFilesStartLocation},
    },
    utils::{
        build_user_from_identity,
        clipboard::clipboard_data::{
            check_if_there_is_file_or_string_in_clipboard, get_files_path_from_clipboard,
            ClipboardDataType,
        },
    },
};

pub fn get_chatbar<'a>(cx: &'a Scoped<'a, ChatProps>) -> Element<'a> {
    log::trace!("get_chatbar");
    let state = use_shared_state::<State>(cx)?;
    let chat_data = use_shared_state::<ChatData>(cx)?;
    let scroll_btn = use_shared_state::<ScrollBtn>(cx)?;
    let to_send = use_shared_state::<MessagesToSend>(cx)?;
    let edit_msg = use_shared_state::<MessagesToEdit>(cx)?;
    state.write_silent().scope_ids.chatbar = Some(cx.scope_id().0);

    let active_chat_id = chat_data.read().active_chat.id();

    // this may just be paranoia
    let state_matches_active_chat = state
        .read()
        .get_active_chat()
        .map(|c| c.id == active_chat_id)
        .unwrap_or_default();

    let is_loading = !state_matches_active_chat || !chat_data.read().active_chat.is_initialized;
    let can_send = use_state(cx, || state.read().active_chat_has_draft());
    let update_script = use_state(cx, String::new);
    let upload_button_menu_uuid = &*cx.use_hook(|| Uuid::new_v4().to_string());
    let show_storage_modal = use_state(cx, || false);

    let suggestions = use_state(cx, || SuggestionType::None);
    let mentions = use_ref(cx, Vec::new);

    let with_scroll_btn = scroll_btn.read().get(active_chat_id) && !is_loading;

    // if the active chat is scrolled up and a message is received, want to increment unreads
    // but the needed information isn't accessible in main.rs. so a flag was added to State
    // and is set here in the chatbar. This was done here instead of in messages.rs as
    // an attempted optimization - don't want to re-render messages whenever scroll_btn
    // is written to, which could be a lot.
    if !is_loading {
        state
            .write_silent()
            .set_chat_scrolled(active_chat_id, with_scroll_btn);
    }

    // this was moved from chat/mod.rs so that unreads doesn't get cleared automatically.
    if !with_scroll_btn && state.read().chats().active_chat_has_unreads() && !is_loading {
        state.write().mutate(Action::ClearActiveUnreads);
    }

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
            .mutate(Action::SetChatAttachments(active_chat_id, files_attached));
    }

    let my_id = state.read().did_key();
    let users_typing: Vec<DID> = state
        .read()
        .get_active_chat()
        .map(|x| {
            x.typing_indicator
                .keys()
                .filter(|did| !my_id.eq(*did))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let users_typing = state.read().get_identities(&users_typing);

    // this is used to scroll to the bottom of the chat.
    let scroll_ch = coroutines::get_scroll_ch(cx, chat_data, state);
    let msg_ch: Coroutine<MsgChInput> = coroutines::get_msg_ch(cx, state);
    let messages_to_send = &to_send.read().messages_to_send.clone();
    if !messages_to_send.is_empty() {
        for (txt, files) in messages_to_send {
            state.write().mutate(Action::SetChatAttachments(
                active_chat_id,
                files.iter().map(|f| f.clone().into()).collect(),
            ));
            msg_ch.send(MsgChInput {
                msg: txt
                    .as_ref()
                    .map(|t| t.lines().map(|s| s.to_string()).collect())
                    .unwrap_or_default(),
                conv_id: active_chat_id,
                replying_to: None,
            });
        }
        to_send.with_mut(|s| s.messages_to_send.clear())
    }
    let local_typing_ch = coroutines::get_typing_ch(cx);
    let local_typing_ch2 = local_typing_ch.clone();

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
            if !current_chat.is_nil() {
                local_typing_ch1.send(TypingIndicator::Refresh(current_chat));
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

    let chat_participants: Vec<_> = state
        .read()
        .get_active_chat()
        .map(|chat| {
            chat.participants
                .iter()
                .filter_map(|did| state.read().get_identity(did))
                .collect()
        })
        .unwrap_or_default();
    let chat_participants_2 = chat_participants.clone();
    let chat_participants_3 = chat_participants.clone();

    let submit_fn = move || {
        local_typing_ch.send(TypingIndicator::NotTyping);
        let active_chat_id = chat_data.read().active_chat.id();

        let msg = state
            .read()
            .get_active_chat()
            .as_ref()
            .and_then(|d| d.draft.clone())
            .map(|msg| {
                let (txt, _) =
                    parse_mentions(&msg, &chat_participants_3, &my_id, true, mention_to_did_key);
                txt
            })
            .unwrap_or_default()
            .lines()
            .map(|x| x.trim_end().to_string())
            .collect::<Vec<String>>();

        if !active_chat_id.is_nil() {
            state
                .write()
                .mutate(Action::SetChatDraft(active_chat_id, String::new()));
        }

        suggestions.set(SuggestionType::None);
        mentions.set(vec![]);

        if !msg_valid(&msg) || active_chat_id.is_nil() {
            return;
        }

        can_send.set(false);
        if STATIC_ARGS.use_mock {
            state.write().mutate(Action::MockSend(active_chat_id, msg));
        } else {
            let replying_to = state.read().chats().get_replying_to();
            if replying_to.is_some() {
                state.write().mutate(Action::CancelReply(active_chat_id));
            }
            msg_ch.send(MsgChInput {
                msg,
                conv_id: active_chat_id,
                replying_to,
            });
        }
    };

    let submit_fn2 = submit_fn.clone();

    let extensions = &state.read().ui.extensions;
    let ext_renders = extensions
        .values()
        .filter(|(is_enabled, ext)| {
            ext.details().location == extensions::Location::Chatbar && *is_enabled
        })
        .map(|(_, ext)| ext.render(cx.scope))
        .collect::<Vec<_>>();

    let disabled = !state.read().can_use_active_chat();
    // todo: don't define a hook so far down
    let error = use_state(cx, || (false, active_chat_id));
    let value_chatbar = state
        .read()
        .get_active_chat()
        .as_ref()
        .and_then(|d| d.draft.clone())
        .unwrap_or_default();

    if value_chatbar.len() >= MAX_CHARS_LIMIT && !error.0 {
        error.set((true, active_chat_id));
    } else if value_chatbar.len() < MAX_CHARS_LIMIT && error.0 {
        error.set((false, active_chat_id));
    }

    let validate_max = move || {
        let value_chatbar = state
            .read()
            .get_active_chat()
            .as_ref()
            .and_then(|d| d.draft.clone())
            .unwrap_or_default();
        if value_chatbar.len() >= MAX_CHARS_LIMIT {
            error.set((true, active_chat_id));
        } else if value_chatbar.len() < MAX_CHARS_LIMIT && error.0 {
            error.set((false, active_chat_id));
        }
    };
    let placeholder_text = if !state.read().ui.is_minimal_view() {
        get_local_text("messages.say-something-placeholder")
    } else {
        "...".to_string()
    };

    let typing_users: Vec<String> = users_typing.iter().map(|id| (*id).username()).collect();

    let chatbar = cx.render(rsx!(
        Chatbar {
            key: "{active_chat_id}",
            id: format!("{}-chatbar", active_chat_id.to_string()),
            loading: is_loading,
            placeholder: placeholder_text,
            typing_users: typing_users,
            is_disabled: disabled,
            ignore_focus: cx.props.ignore_focus,
            on_paste_keydown: move |e: Event<KeyboardData>| {
                // HACK: Allow copy and paste files for Linux
                if cfg!(target_os = "linux") {
                    let keyboard_data = e;
                 if keyboard_data.code() == Code::KeyV
                        && keyboard_data.modifiers() == Modifiers::CONTROL && *enable_paste_shortcut.read()
                    {
                    let files_local_path = get_files_path_from_clipboard().unwrap_or_default();
                    state
                        .write()
                        .mutate(Action::AppendChatAttachments(active_chat_id, files_local_path));
                }
                }
            },
            onchange: move |v: String| {
                if !active_chat_id.is_nil() {
                    state.write_silent().mutate(Action::SetChatDraft(active_chat_id, v));
                    validate_max();
                    update_send();
                    local_typing_ch2.send(TypingIndicator::Typing(active_chat_id));
                }
            },
            value: state.read().get_active_chat().as_ref().and_then(|d| d.draft.clone()).unwrap_or_default(),
            onreturn: move |_| submit_fn(),
            extensions: cx.render(rsx!(for node in ext_renders { rsx!(node) })),
            suggestions: suggestions,
            oncursor_update: move |(mut v, p): (String, i64)| {
                if !active_chat_id.is_nil() {
                    let sub: String = v.chars().take(p as usize).collect();
                    let emoji_capture = EMOJI_REGEX.captures(&sub);
                    if let Some(emoji) = emoji_capture {
                            let emoji = &emoji[0];
                            if emoji.contains(char::is_whitespace) {
                                suggestions.set(SuggestionType::None);
                                return;
                            }
                            if emoji.ends_with(':') {
                                // Replace emoji alias
                                let alias = emoji.replace(':', "");
                                let s = state.read().ui.emojis.get_matching_emoji(&alias, true);
                                let replacement = s.first();
                                if let Some((emoji, _)) = replacement {
                                    v = v.replace(&sub, &sub.replace(&format!(":{alias}:"), emoji));
                                    state.write().mutate(Action::SetChatDraft(active_chat_id, v));
                                }
                                suggestions.set(SuggestionType::None);
                            } else {
                                //Suggest emojis
                                let alias = emoji.replace(':', "");
                                suggestions.set(SuggestionType::Emoji(emoji.to_string(), state.read().ui.emojis.get_matching_emoji(&alias, false)));
                            }
                            return;
                    }
                    let tag_capture = TAG_REGEX.captures(&sub);
                    match tag_capture {
                        Some(tag) => {
                            let tag = &tag[0];
                            let tag = tag.replace('@', "");
                            if tag.ends_with(' ') {
                                let name = tag.replace(' ', "").to_lowercase();
                                let replacement = chat_participants.iter().find(|id|id.username().to_lowercase().eq(&name));
                                if let Some(id) = replacement {
                                    let username = format!("{}#{}", id.username(), id.short_id());
                                    v = v.replace(&sub, &sub.replace(&tag, &format!("{username} ")));
                                    state.write().mutate(Action::SetChatDraft(active_chat_id, v));
                                    mentions.write_silent().push((id.did_key(), username));
                                }
                                suggestions.set(SuggestionType::None);
                                return;
                            }
                            let lower = tag.to_lowercase();
                            let users: Vec<_> = chat_participants.iter().filter(|id|id.username().to_lowercase().starts_with(&lower))
                                .cloned().collect();
                            suggestions.set(SuggestionType::Tag(tag, users));
                        }
                        None => {
                            suggestions.set(SuggestionType::None);
                        }
                    }
                }
            },
            on_suggestion_click: move |(replacement, pattern, p): (String, String, i64)| {
                if !active_chat_id.is_nil() {
                    let mut draft = state
                        .read()
                        .get_active_chat()
                        .as_ref()
                        .and_then(|d| d.draft.clone())
                        .unwrap_or_default();
                    let sub: String = draft.chars().take(p as usize).collect();
                    draft = draft.replace(&sub, &sub.replace(&pattern, &replacement));
                    state
                        .write()
                        .mutate(Action::SetChatDraft(active_chat_id, draft));
                    if let SuggestionType::Tag(_, _) = suggestions.get() {
                        let amount = replacement.chars().count() - 9;
                        let name: String = replacement.chars().take(amount).collect(); // remove short did
                        if let Some(participant) = chat_participants_2.iter().find(|id|id.username().eq(&name)) {
                            mentions.write_silent().push((participant.did_key(), replacement));
                        }
                    }
                    suggestions.set(SuggestionType::None);
                }
            },
            onup_down_arrow: move |code|{
                if code == Code::ArrowUp && edit_msg.read().edit.is_none() {
                    if let Some(msg) = chat_data.read().active_chat.messages.last_user_msg {
                        edit_msg.write().edit = Some(msg);
                    }
                }
            },
            controls: cx.render(
                rsx!(
                    Button {
                        icon: icons::outline::Shape::ChevronDoubleRight,
                        disabled: is_loading || disabled,
                        appearance: if * can_send.get() { Appearance::Primary } else { Appearance::Secondary },
                        aria_label: "send-message-button".into(),
                        onpress: move |_| submit_fn2(),
                        tooltip: cx.render(rsx!(Tooltip {
                            arrow_position: ArrowPosition::Bottom,
                            text :get_local_text("uplink.send"),
                        })),
                    }
                ),
            ),
            with_replying_to: (!disabled).then(|| {
                cx.render(
                    rsx!(
                        chat_data.read().active_chat.replying_to().as_ref().map(|msg| {
                            let our_did = state.read().did_key();
                            let msg_owner = if state.read().did_key() == msg.sender() {
                                state.read().get_identity(&state.read().did_key())
                            } else {
                                chat_data.read().active_chat.other_participants().iter().find(|x| x.did_key() == msg.sender()).cloned()
                            };

                            let (platform, status, profile_picture) = get_platform_and_status(msg_owner.as_ref());

                            rsx!(
                                Reply {
                                    label: get_local_text("messages.replying"),
                                    remote: our_did != msg.sender(),
                                    onclose: move |_| {
                                        state.write().mutate(Action::CancelReply(active_chat_id))
                                    },
                                    attachments: msg.attachments(),
                                    message: msg.lines().join("\n"), 
                                    markdown: state.read().ui.should_transform_markdown_text(),
                                    transform_ascii_emojis: state.read().ui.should_transform_ascii_emojis(),
                                    state: state,
                                    chat: chat_data.read().active_chat.id(),
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
                            let script = SHOW_CONTEXT
                                .replace("UUID", upload_button_menu_uuid)
                                .replace("$PAGE_X", &mouse_data.page_coordinates().x.to_string())
                                .replace("$PAGE_Y", &mouse_data.page_coordinates().y.to_string())
                                .replace("$SELF", "false");
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
                            state
                                .write()
                                .mutate(Action::AppendChatAttachments(active_chat_id, new_files));
                            update_send();
                            }
                        },
                    }
                ),
            )
        }
        error.0.then(|| rsx!(
            p {
                class: "chatbar-error-input-message",
                aria_label: "chatbar-input-error",
                get_local_text_with_args("warning-messages.maximum-of", vec![("num", MAX_CHARS_LIMIT)])
            }
        ))
    ));

    cx.render(rsx!(
        if state.read().ui.metadata.focused && *enable_paste_shortcut.read() {
                rsx!(shortcuts::paste_file_shortcut::PasteFilesShortcut {
                    on_paste: move |files_local_path: Vec<PathBuf>| {
                        state
                            .write()
                            .mutate(Action::AppendChatAttachments(active_chat_id, files_local_path));
                    }})
            }
                SendFilesLayoutModal {
                    send_files_from_storage: show_storage_modal,
                    send_files_start_location: SendFilesStartLocation::Chats,
                    on_send: move |(files_location, _): (Vec<Location>, _)| {
                        let mut new_files_to_upload: Vec<_> = state.read().get_active_chat().map(|f| f.files_attached_to_send)
                        .unwrap_or_default()
                        .iter()
                        .filter(|file_location| {
                            !files_location.contains(file_location)
                        })
                        .cloned()
                        .collect();
                        new_files_to_upload.extend(files_location);
                        state.write().mutate(Action::SetChatAttachments(active_chat_id, new_files_to_upload));
                        update_send();
                    },
                },
        div {
            class: "chatbar-container",
            with_scroll_btn.then(|| {
                rsx!(div {
                    class: "btn scroll-bottom-btn",
                    onclick: move |_| {
                        scroll_btn.write().clear(active_chat_id);
                        state.write().mutate(Action::ClearUnreads(active_chat_id));
                        // note that if scroll_behavior.on_scroll_end == ScrollBehavior::DoNothing then it isn't necessary to 
                        // fetch more messages - one could just use a regular javascript to scroll to the end of the page. 
                        // however, this is easier and seems to work well enough. 
                        scroll_ch.send(active_chat_id);
                    },
                    get_local_text("messages.scroll-bottom"),
                })
            })
        },
        Attachments {
            chat_id: active_chat_id,
            files_to_attach: state.read().get_active_chat().map(|f| f.files_attached_to_send).unwrap_or_default(),
            on_remove: move |files_attached| {
                state.write().mutate(Action::SetChatAttachments(active_chat_id, files_attached));
                update_send();
            }
        },
        chatbar
    ))
}

fn get_platform_and_status(msg_sender: Option<&Identity>) -> (Platform, Status, String) {
    let sender = match msg_sender {
        Some(identity) => identity,
        None => return (Platform::Desktop, Status::Offline, String::new()),
    };
    let user_sender = build_user_from_identity(sender);
    (user_sender.platform, user_sender.status, user_sender.photo)
}
