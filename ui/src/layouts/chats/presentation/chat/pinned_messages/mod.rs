use chrono::{DateTime, Utc};
use common::{
    language::get_local_text,
    state::{Identity, State},
    warp_runner::{thumbnail_to_base64, RayGunCmd, WarpCmd},
    WARP_CMD_CH,
};
use dioxus::prelude::*;

use futures::StreamExt;
use kit::components::{embeds::file_embed::FileEmbed, message::ChatText, user_image::UserImage};
use uuid::Uuid;
use warp::raygun::PinState;

use tracing::log;

use crate::layouts::chats::{
    data::{self, ChatData},
    presentation::chat::coroutines::fetch_window,
};

const MARKDOWN_FOR_CODE_BLOCK: &str = r#"
(() => {{
    Prism.highlightAll();
}})();
"#;

pub enum ChannelCommand {
    RemovePinnedMessage {
        conversation_id: Uuid,
        message_id: Uuid,
        show_pinned: Signal<bool>,
    },
    GoToPinnedMessage {
        conversation_id: Uuid,
        message_id: Uuid,
        message_date: DateTime<Utc>,
        show_pinned: Signal<bool>,
    },
}

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    show_pinned: Signal<bool>,
}

#[allow(non_snake_case)]
pub fn PinnedMessages(props: Props) -> Element {
    log::trace!("rendering pinned_messages");
    let mut state = use_context::<Signal<State>>();
    let chat_data = use_context::<Signal<ChatData>>();
    let minimal = state.read().ui.metadata.minimal_view;

    let ch = use_coroutine(|mut rx: UnboundedReceiver<ChannelCommand>| {
        to_owned![chat_data, state];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    ChannelCommand::RemovePinnedMessage {
                        conversation_id,
                        message_id,
                        show_pinned: _,
                    } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::Pin {
                            conversation_id,
                            message_id,
                            pinstate: PinState::Unpin,
                            rsp: tx,
                        })) {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        if let Err(e) = res {
                            log::error!("failed to pin message: {}", e);
                        }
                    }
                    ChannelCommand::GoToPinnedMessage {
                        conversation_id,
                        message_id,
                        message_date,
                        mut show_pinned,
                    } => {
                        log::debug!("fetching pinned message");
                        let view_init = data::ViewInit {
                            scroll_to: data::ScrollTo::ScrollUp {
                                view_top: message_id,
                            },
                            msg_time: Some(message_date),
                            limit: data::DEFAULT_MESSAGES_TO_TAKE,
                        };
                        let behavior = data::ChatBehavior {
                            view_init,
                            // these fields will be overwritten by fetch_window
                            on_scroll_end: data::ScrollBehavior::FetchMore,
                            on_scroll_top: data::ScrollBehavior::FetchMore,
                            ..Default::default()
                        };
                        let r = fetch_window(
                            conversation_id,
                            behavior,
                            message_date,
                            data::DEFAULT_MESSAGES_TO_TAKE / 2,
                        )
                        .await;

                        match r {
                            Ok((messages, behavior)) => {
                                log::debug!("re-init messages with pinned message");
                                chat_data.write().set_active_chat(
                                    &state.read(),
                                    &conversation_id,
                                    behavior,
                                    messages,
                                );
                            }
                            Err(e) => log::error!("{e}"),
                        }

                        show_pinned.set(false);
                    }
                }
            }
        }
    });
    let pinned_messages = chat_data.read().active_chat.pinned_messages();
    let _ = eval(MARKDOWN_FOR_CODE_BLOCK);

    rsx!(
        script {
            {MARKDOWN_FOR_CODE_BLOCK}
        }
        div {
            id: "pinned-messages-container",
            class: format_args!("{}", if minimal {"pinned-minimal"} else {""}),
            aria_label: "pinned-messages-label",
        div {
            class: "pinned-messages",
            aria_label: "pinned-messages-container",
            if pinned_messages.is_empty() {
                {rsx!(div {
                    class: "pinned-empty",
                    aria_label: "pinned-empty",
                    div {
                        {get_local_text("messages.pinned-none")}
                    }
                })}
            } else {
                {rsx!({pinned_messages.iter().map(|message|{
                    let sender = state.read().get_identity(&message.sender());
                    let time = message.date().format(&get_local_text("uplink.date-time-format")).to_string();
                    let conversation_id = message.conversation_id();
                    let message_id = message.id();
                    let message_date = message.date();
                    rsx!(PinnedMessage {
                        message: message.clone(),
                        chat: chat_data.read().active_chat.id(),
                        sender: sender,
                        onremove: move |(_,msg): (Event<MouseData>, warp::raygun::Message)| {
                            let conv = &msg.conversation_id();
                            ch.send(ChannelCommand::RemovePinnedMessage{ conversation_id: *conv, message_id: msg.id(), show_pinned: props.show_pinned.clone() })
                        },
                        time: time,
                        onclick: move |_| {
                            ch.send(ChannelCommand::GoToPinnedMessage{conversation_id, message_id, message_date, show_pinned: props.show_pinned.clone()});
                        }
                    },
     )
                })})}
            }
        }
    })
}

#[derive(Props, Clone, PartialEq)]
pub struct PinnedMessageProp {
    message: warp::raygun::Message,
    chat: Uuid,
    #[props(!optional)]
    sender: Option<Identity>,
    onremove: EventHandler<(Event<MouseData>, warp::raygun::Message)>,
    time: String,
    onclick: EventHandler<()>,
}

#[allow(non_snake_case)]
pub fn PinnedMessage(props: PinnedMessageProp) -> Element {
    let mut state = use_context::<Signal<State>>();
    let message = &props.message;
    let attachments = message.attachments();

    let attachment_list = attachments.iter().map(|file| {
        let key = file.id();
        rsx!(FileEmbed {
            key: "{key}",
            filename: file.name(),
            filesize: file.size(),
            thumbnail: thumbnail_to_base64(file),
            with_download_button: false,
            big: false,
            remote: true,
            download_pending: false,
            on_press: move |_| {},
        })
    });
    let has_attachments = !attachments.is_empty();

    rsx!(div {
            class: "pinned-message-wrap",
            aria_label: "pinned-message-wrap",
            {props.sender.as_ref().map(|sender| {
                rsx!(UserImage {
                    image: sender.profile_picture(),
                    platform: sender.platform().into(),
                })
            })},
            div {
                class: "pinned-message",
                aria_label: "pinned-message",
                white_space: "pre-wrap",
                div{
                    class: "pinned-content-container",
                    div {
                        class: "pinned-sender-container",
                        div {
                            class: "full-flex",
                            {props.sender.as_ref().map(|sender| {
                                rsx!(
                                    p {
                                        class: "ellipsis-overflow",
                                        aria_label: "pinned-sender",
                                        {sender.username()}
                                    },
                                )
                            })},
                            p {
                                class: "pinned-sender-time",
                                aria_label: "pinned-time",
                                "{props.time}"
                            }
                        }
                        div {
                            class: "pinned-button-container",
                            aria_label: "pinned-button-container",
                            button {
                                class: "pinned-buttons",
                                aria_label: "pin-button-go-to",
                                onclick: move |_| {
                                    props.onclick.call(());
                                },
                                {get_local_text("messages.pin-button-goto")}
                            },
                            button {
                                class: "pinned-buttons",
                                aria_label: "pin-button-unpin",
                                onclick: move |e| {
                                    props.onremove.call((e, props.message.clone()));
                                },
                                {get_local_text("messages.pin-button-unpin")},
                            }
                        }
                    }
                    ChatText {
                        text: message.lines().join("\n"),
                        remote: true,
                        pending: false,
                        state: state,
                        chat: props.chat,
                        markdown: state.read().ui.should_transform_markdown_text(),
                        ascii_emoji: state.read().ui.should_transform_ascii_emojis(),
                    }
                },
                {has_attachments.then(|| {
                    rsx!(
                        div {
                            class: "attachment-list",
                            aria_label: "pinned-attachments",
                            {attachment_list.map(|list| {
                                rsx!({list})
                            })}
                        }
                    )
                })}
            }
        }
    )
}
