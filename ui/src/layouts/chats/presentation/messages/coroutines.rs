use common::{
    state::{
        chats2::{ChatBehavior, MsgRange},
        Action, State,
    },
    warp_runner::{RayGunCmd, WarpCmd},
    WARP_CMD_CH,
};

use dioxus_core::Scoped;
use dioxus_hooks::{
    to_owned, use_coroutine, Coroutine, UnboundedReceiver, UseRef, UseSharedState, UseState,
};
use futures::{pin_mut, StreamExt};

use uuid::Uuid;
use warp::raygun::{PinState, ReactionState};

use crate::layouts::chats::{data::JsMsg, scripts::OBSERVER_SCRIPT, ActiveChat};

use super::{DownloadTracker, MessagesCommand, NewelyFetchedMessages};

pub fn hangle_msg_scroll<'a>(
    cx: &'a Scoped,
    eval_provider: &crate::utils::EvalProvider,
    active_chat: &UseSharedState<ActiveChat>,
) -> Coroutine<Uuid> {
    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<Uuid>| {
        to_owned![eval_provider, active_chat];
        async move {
            let mut current_conv_id: Option<Uuid> = None;

            // don't begin the coroutine until use_eval sends a command over the channel.
            'WAIT_FOR_INIT: while let Some(conv_id) = rx.next().await {
                println!("init handle_msg_scroll for conv id {}", conv_id);
                current_conv_id.replace(conv_id);

                'CONFIGURE_EVAL: loop {
                    // todo: set these correctly. they may change each time top or bottom is scrolled to.
                    let should_send_top_evt = true;
                    let should_send_bottom_evt = false;
                    let mut observer_script = OBSERVER_SCRIPT.replace(
                        "$SEND_TOP_EVENT",
                        should_send_top_evt.then_some("1").unwrap_or("0"),
                    );
                    observer_script = observer_script.replace(
                        "$SEND_BOTTOM_EVENT",
                        should_send_bottom_evt.then_some("1").unwrap_or("0"),
                    );
                    observer_script =
                        observer_script.replace("$CONVERSATION_ID", &conv_id.to_string());

                    let eval = match eval_provider(&observer_script) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("use eval failed: {:?}", e);
                            return;
                        }
                    };

                    // not sure if it's safe to call eval.recv() in a select! statement. turning it into something
                    // which should definitely work for that.
                    let eval_stream = async_stream::stream! {
                        loop {
                            match eval.recv().await {
                                Ok(msg) => {
                                    println!("got this from js: {msg}");
                                    yield msg;
                                },
                                Err(e) => {
                                    eprintln!("error receiving from js: {e:?}");
                                    break;
                                }
                            }
                        }
                    };
                    pin_mut!(eval_stream);

                    loop {
                        tokio::select! {
                            opt = rx.next() => {
                                match opt {
                                    Some(conv_id) => {
                                        let conv_id_changed = current_conv_id.as_ref().map(|x| x != &conv_id).unwrap_or(true);
                                        println!("conv id changed from: {:?} to {}", current_conv_id, conv_id);
                                        if !conv_id_changed {
                                            log::warn!("extra command sent to hangle_msg_scroll");
                                        }
                                        current_conv_id.replace(conv_id);
                                        continue 'CONFIGURE_EVAL;
                                    }
                                    None => {
                                        // failed to read from stream. use_coroutine is probably done for.
                                        return;
                                    }
                                }
                            },
                            res = eval_stream.next() => match res {
                                Some(msg) => {
                                    if let Some(s) = msg.as_str() {
                                        match serde_json::from_str::<JsMsg>(s) {
                                            // ignore events with a different conversation id
                                            Ok(msg) => match msg {
                                                JsMsg::Add { msg_id, conv_id } => {
                                                    let conv_id_changed = current_conv_id.as_ref().map(|x| x == &conv_id).unwrap_or(true);
                                                    if conv_id_changed { continue; }
                                                    active_chat.write_silent().add_message_to_view(msg_id);
                                                },
                                                JsMsg::Remove { msg_id, conv_id } => {
                                                    let conv_id_changed = current_conv_id.as_ref().map(|x| x == &conv_id).unwrap_or(true);
                                                    if conv_id_changed { continue; }
                                                    active_chat.write_silent().remove_message_from_view(msg_id);
                                                }
                                                JsMsg::Top { conv_id } => {
                                                    let conv_id_changed = current_conv_id.as_ref().map(|x| x == &conv_id).unwrap_or(true);
                                                    if conv_id_changed { continue; }
                                                }
                                                JsMsg::Bottom { conv_id } => {
                                                    let conv_id_changed = current_conv_id.as_ref().map(|x| x == &conv_id).unwrap_or(true);
                                                    if conv_id_changed { continue; }
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("failed to deserialize message: {}: {}", s, e);
                                            }
                                        }
                                    }
                                }
                                None => {
                                    // the evaluator broke
                                    continue 'WAIT_FOR_INIT;
                                }
                            },
                        }
                    }
                }
            }
        }
    });

    ch.clone()
}

pub fn handle_warp_commands<'a>(
    cx: &'a Scoped,
    state: &UseSharedState<State>,
    newly_fetched_messages: &UseRef<Option<NewelyFetchedMessages>>,
    pending_downloads: &UseSharedState<DownloadTracker>,
) -> Coroutine<MessagesCommand> {
    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<MessagesCommand>| {
        to_owned![state, newly_fetched_messages, pending_downloads];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    MessagesCommand::React((user, message, emoji)) => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        let reaction_state =
                            match message.reactions().iter().find(|x| x.emoji() == emoji) {
                                Some(reaction) if reaction.users().contains(&user) => {
                                    ReactionState::Remove
                                }
                                _ => ReactionState::Add,
                            };
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::React {
                            conversation_id: message.conversation_id(),
                            message_id: message.id(),
                            reaction_state,
                            emoji: emoji.clone(),
                            rsp: tx,
                        })) {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        match res {
                            Ok(_) => state.write().mutate(Action::AddReaction(
                                message.conversation_id(),
                                message.id(),
                                emoji,
                            )),
                            Err(e) => {
                                log::error!("failed to add/remove reaction: {}", e);
                            }
                        }
                    }
                    MessagesCommand::DeleteMessage { conv_id, msg_id } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::DeleteMessage {
                                conv_id,
                                msg_id,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        if let Err(e) = res {
                            log::error!("failed to delete message: {}", e);
                        }
                    }
                    MessagesCommand::DownloadAttachment {
                        conv_id,
                        msg_id,
                        file,
                        file_path_to_download,
                    } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::DownloadAttachment {
                                conv_id,
                                msg_id,
                                file_name: file.name(),
                                file_path_to_download,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            if let Some(conv) = pending_downloads.write().get_mut(&conv_id) {
                                conv.remove(&file);
                            }
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        match res {
                            Ok(mut stream) => {
                                while let Some(p) = stream.next().await {
                                    log::debug!("{p:?}");
                                }
                            }
                            Err(e) => {
                                log::error!("failed to download attachment: {}", e);
                            }
                        }
                        if let Some(conv) = pending_downloads.write().get_mut(&conv_id) {
                            conv.remove(&file);
                        }
                    }
                    MessagesCommand::EditMessage {
                        conv_id,
                        msg_id,
                        msg,
                    } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::EditMessage {
                            conv_id,
                            msg_id,
                            msg,
                            rsp: tx,
                        })) {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        if let Err(e) = res {
                            log::error!("failed to edit message: {}", e);
                        }
                    }
                    MessagesCommand::FetchMore {
                        conv_id,
                        to_fetch,
                        current_len,
                    } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::FetchMessagesDeprecated {
                                conv_id,
                                to_fetch,
                                current_len,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        match rx.await.expect("command canceled") {
                            Ok((messages, has_more)) => {
                                newly_fetched_messages.set(Some(NewelyFetchedMessages {
                                    conversation_id: conv_id,
                                    messages,
                                    has_more,
                                }));
                            }
                            Err(e) => {
                                log::error!("failed to fetch more message: {}", e);
                            }
                        }
                    }
                    MessagesCommand::Pin(msg) => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        let pinstate = if msg.pinned() {
                            PinState::Unpin
                        } else {
                            PinState::Pin
                        };
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::Pin {
                            conversation_id: msg.conversation_id(),
                            message_id: msg.id(),
                            pinstate,
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
                }
            }
        }
    });
    ch.clone()
}
