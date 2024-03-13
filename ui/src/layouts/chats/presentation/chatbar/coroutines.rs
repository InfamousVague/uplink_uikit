use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use common::{
    state::{Action, State},
    warp_runner::{RayGunCmd, WarpCmd},
    STATIC_ARGS, WARP_CMD_CH,
};
use dioxus::prelude::*;
use futures::{channel::oneshot, StreamExt};
use uuid::Uuid;
use warp::raygun::{self, Location};

use crate::{
    layouts::chats::data::{self, ChatProps, MsgChInput, TypingInfo, DEFAULT_MESSAGES_TO_TAKE},
    utils::async_task_queue::chat_upload_stream_handler,
};

use super::TypingIndicator;

pub fn get_msg_ch(
    cx: &Scoped<'_, ChatProps>,
    state: &UseSharedState<State>,
) -> Coroutine<MsgChInput> {
    let upload_streams = chat_upload_stream_handler(cx);
    use_coroutine( |mut rx: UnboundedReceiver<MsgChInput>| {
        to_owned![state, upload_streams];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(MsgChInput {
                msg,
                conv_id,
                appended_msg_id,
                replying_to,
            }) = rx.next().await
            {
                let (tx, rx) = oneshot::channel();
                let attachments = state
                    .read()
                    .get_active_chat()
                    .map(|f| f.files_attached_to_send)
                    .unwrap_or_default();
                let msg_clone = msg.clone();
                let cmd = match replying_to {
                    Some(reply_to) => RayGunCmd::Reply {
                        conv_id,
                        reply_to,
                        msg: msg.clone(),
                        attachments,
                        rsp: tx,
                    },
                    None => RayGunCmd::SendMessage {
                        conv_id,
                        msg: msg.clone(),
                        attachments,
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
                    .map(|p| {
                        let pathbuf = match p {
                            Location::Disk { path } => path.clone(),
                            Location::Constellation { path } => PathBuf::from(path),
                        };
                        pathbuf
                            .file_name()
                            .map_or_else(String::new, |ostr| ostr.to_string_lossy().to_string())
                    })
                    .collect();
                if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(cmd)) {
                    log::error!("failed to send warp command: {}", e);
                    state.write().decrement_outgoing_messages(
                        conv_id,
                        msg_clone,
                        attachment_files,
                        appended_msg_id,
                    );
                    continue;
                }

                let rsp = rx.await.expect("command canceled");
                match rsp {
                    Ok(Some(attachment)) => upload_streams.write().append((
                        conv_id,
                        msg,
                        attachments,
                        appended_msg_id,
                        attachment,
                    )),
                    Err(e) => {
                        log::error!("failed to send message: {}", e);
                        state.write().decrement_outgoing_messages(
                            conv_id,
                            msg_clone,
                            attachment_files,
                            appended_msg_id,
                        )
                    }
                    _ => {}
                }
            }
        }
    })
    .clone()
}

pub fn get_scroll_ch(
    cx: &Scoped<'_, ChatProps>,
    chat_data: &UseSharedState<data::ChatData>,
    state: &UseSharedState<State>,
) -> Coroutine<Uuid> {
    use_coroutine( |mut rx: UnboundedReceiver<Uuid>| {
        to_owned![chat_data, state];
        async move {
            while let Some(conv_id) = rx.next().await {
                match crate::layouts::chats::presentation::chat::coroutines::fetch_most_recent(
                    conv_id,
                    DEFAULT_MESSAGES_TO_TAKE,
                )
                .await
                {
                    Ok((messages, behavior)) => {
                        log::trace!("re-init messages with most recent");
                        chat_data.write().set_active_chat(
                            &state.read(),
                            &conv_id,
                            behavior,
                            messages,
                        );
                    }
                    Err(e) => log::error!("{e}"),
                }
            }
        }
    })
    .clone()
}

// typing indicator notes
// consider side A, the local side, and side B, the remote side
// side A -> (typing indicator) -> side B
// side B removes the typing indicator after a timeout
// side A doesn't want to send too many typing indicators, say once every 4-5 seconds
// should we consider matching the timeout with the send frequency so we can closely match if a person is straight up typing for 5 mins straight.

// tracks if the local participant is typing
// re-sends typing indicator in response to the Refresh command
pub fn get_typing_ch(cx: &Scoped<'_, ChatProps>) -> Coroutine<TypingIndicator> {
    use_coroutine( |mut rx: UnboundedReceiver<TypingIndicator>| {
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
    })
    .clone()
}
