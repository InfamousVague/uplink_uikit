use anyhow::bail;
use chrono::{DateTime, Utc};
use common::{
    state::State,
    warp_runner::{
        ui_adapter::{self, MessageEvent},
        FetchMessagesConfig, FetchMessagesResponse, RayGunCmd, WarpCmd, WarpEvent,
    },
    WARP_CMD_CH, WARP_EVENT_CH,
};
use dioxus::prelude::*;
use futures::channel::oneshot;
use uuid::Uuid;

use crate::layouts::chats::data::{self, ChatBehavior, ChatData};

pub fn handle_warp_events(state: Signal<State>, chat_data: Signal<ChatData>) {
    let active_chat_id_signal = use_signal(move || {
        let active_chat_id = state.read().get_active_chat().map(|x| x.id);
        active_chat_id
    });
    let _ = use_future(move || {
        to_owned![chat_data];
        async move {
            let mut ch = WARP_EVENT_CH.tx.subscribe();
            while let Ok(evt) = ch.recv().await {
                let message_evt = match evt {
                    WarpEvent::Message(evt) => evt,
                    _ => continue,
                };
                let chat_id = match active_chat_id_signal.read().as_ref() {
                    Some(x) => *x,
                    None => continue,
                };

                match message_evt {
                    MessageEvent::Received {
                        conversation_id,
                        message,
                    }
                    | MessageEvent::Sent {
                        conversation_id,
                        message,
                    } => {
                        if conversation_id != chat_id {
                            continue;
                        }
                        if chat_data
                            .write_silent()
                            .new_message(conversation_id, message)
                        {
                            log::trace!("adding message to conversation");
                            chat_data.write().active_chat.new_key();
                        }
                    }
                    MessageEvent::Edited {
                        conversation_id,
                        message,
                    } => {
                        if chat_data.read().active_chat.id() != conversation_id {
                            continue;
                        }
                        chat_data.write().update_message(message.inner);
                    }
                    MessageEvent::Deleted {
                        conversation_id,
                        message_id,
                        ..
                    } => {
                        if chat_data.read().active_chat.id() != conversation_id {
                            continue;
                        }
                        chat_data
                            .write()
                            .delete_message(conversation_id, message_id);
                    }
                    MessageEvent::MessageReactionAdded { message }
                    | MessageEvent::MessageReactionRemoved { message } => {
                        if chat_data.read().active_chat.id() != message.conversation_id() {
                            continue;
                        }
                        chat_data.write().update_message(message);
                    }
                    MessageEvent::MessagePinned { message }
                    | MessageEvent::MessageUnpinned { message } => {
                        if chat_data.read().active_chat.has_message_id(message.id()) {
                            chat_data.write().update_message(message);
                        }
                    }
                    _ => continue,
                }
            }
        }
    });
}

// any use_future should be in the coroutines file to prevent a naming conflict with the futures crate.
pub fn init_chat_data(state: Signal<State>, chat_data: Signal<ChatData>) -> Signal<bool> {
    let active_chat_id_signal = use_signal(move || {
        let active_chat_id = state.read().get_active_chat().map(|x| x.id);
        active_chat_id
    });
    let mut chat_active = use_signal(|| false);
    use_future(move || {
        to_owned![state, chat_data];
        async move {
            while !state.read().initialized {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }

            let conv_id = match active_chat_id_signal() {
                None => return,
                Some(x) => x,
            };

            let behavior = chat_data.read().get_chat_behavior(conv_id);
            let config = behavior.messages_config();

            let r = match config {
                FetchMessagesConfig::MostRecent { limit } => {
                    log::trace!("fetching most recent messages for chat");
                    fetch_most_recent(conv_id, limit).await
                }
                FetchMessagesConfig::Window { center, half_size } => {
                    log::trace!("fetching window for chat");
                    fetch_window(conv_id, behavior, center, half_size).await
                }
                _ => unreachable!(),
            };

            match r {
                Ok((messages, behavior)) => {
                    log::debug!("init_chat_data");

                    chat_data.write_silent().set_active_chat(
                        &state.read(),
                        &conv_id,
                        behavior,
                        messages,
                    );
                    chat_active.set(true);
                }
                Err(e) => log::error!("{e}"),
            }
        }
    });
    chat_active
}

pub async fn fetch_window<'a>(
    conv_id: Uuid,
    chat_behavior: ChatBehavior,
    date: DateTime<Utc>,
    half_size: usize,
) -> anyhow::Result<(Vec<ui_adapter::Message>, ChatBehavior)> {
    let mut messages = vec![];
    let has_more_before: bool;
    let has_more_after: bool;
    let most_recent_msg_id: Option<Uuid>;

    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    let (tx, rx) = oneshot::channel();

    if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::FetchMessages {
        conv_id,
        config: FetchMessagesConfig::Earlier {
            start_date: date,
            limit: half_size,
        },
        rsp: tx,
    })) {
        bail!("failed to init messages: {e}");
    }

    let rsp = match rx.await {
        Ok(r) => r,
        Err(e) => {
            bail!("failed to send warp command. channel closed. {e}");
        }
    };

    match rsp {
        Ok(FetchMessagesResponse {
            messages: mut new_messages,
            has_more,
            most_recent: _,
        }) => {
            has_more_before = has_more;
            messages.append(&mut new_messages);
        }
        Err(e) => {
            bail!("FetchMessages command failed: {e}");
        }
    };

    let (tx, rx) = oneshot::channel();

    if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::FetchMessages {
        conv_id,
        config: FetchMessagesConfig::Later {
            start_date: date,
            limit: half_size,
        },
        rsp: tx,
    })) {
        bail!("failed to init messages: {e}");
    }

    let rsp = match rx.await {
        Ok(r) => r,
        Err(e) => {
            bail!("failed to send warp command. channel closed. {e}");
        }
    };

    match rsp {
        Ok(mut r) => {
            has_more_after = r.has_more;
            most_recent_msg_id = r.most_recent;
            if let Some(msg) = r.messages.first() {
                if msg.inner.id() == messages.last().map(|x| x.inner.id()).unwrap_or_default() {
                    messages.pop();
                }
                messages.append(&mut r.messages);
            }
        }
        Err(e) => {
            bail!("FetchMessages command failed: {e}");
        }
    };

    let new_behavior = ChatBehavior {
        view_init: chat_behavior.view_init,
        on_scroll_end: if has_more_after {
            data::ScrollBehavior::FetchMore
        } else {
            data::ScrollBehavior::DoNothing
        },
        on_scroll_top: if has_more_before {
            data::ScrollBehavior::FetchMore
        } else {
            data::ScrollBehavior::DoNothing
        },
        most_recent_msg_id,
        ..Default::default()
    };

    Ok((messages, new_behavior))
}

pub async fn fetch_most_recent<'a>(
    conv_id: Uuid,
    limit: usize,
) -> anyhow::Result<(Vec<ui_adapter::Message>, ChatBehavior)> {
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    let (tx, rx) = oneshot::channel();

    // todo: save the config during runtime
    if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::FetchMessages {
        conv_id,
        config: FetchMessagesConfig::MostRecent { limit },
        rsp: tx,
    })) {
        bail!("failed to init messages: {e}");
    }

    let rsp = match rx.await {
        Ok(r) => r,
        Err(e) => {
            bail!("failed to send warp command. channel closed. {e}");
        }
    };

    match rsp {
        Ok(FetchMessagesResponse {
            messages,
            has_more,
            most_recent: most_recent_msg_id,
        }) => {
            let chat_behavior = ChatBehavior {
                on_scroll_top: if has_more {
                    data::ScrollBehavior::FetchMore
                } else {
                    data::ScrollBehavior::DoNothing
                },
                most_recent_msg_id,
                ..Default::default()
            };
            Ok((messages, chat_behavior))
        }
        Err(e) => {
            bail!("FetchMessages command failed: {e}");
        }
    }
}
