use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use dioxus::prelude::*;

use futures::{channel::oneshot, StreamExt};
use kit::{
    components::{
        context_menu::{ContextItem, ContextMenu},
        indicator::{Platform, Status},
        message::{Message, Order},
        message_group::{MessageGroup, MessageGroupSkeletal},
        user_image::UserImage,
        user_image_group::UserImageGroup,
    },
    elements::{
        button::Button,
        tooltip::{ArrowPosition, Tooltip},
        Appearance,
    },
    icons::Icon,
    layout::{
        chatbar::{Chatbar, Reply},
        topbar::Topbar,
    },
};

use dioxus_desktop::{use_eval, use_window};
use shared::language::get_local_text;
use uuid::Uuid;
use warp::{
    logging::tracing::log,
    raygun::{self, ReactionState},
};

use crate::{
    components::media::player::MediaPlayer,
    state::{self, Action, Chat, Identity, State},
    utils::{
        build_participants, build_user_from_identity, convert_status,
        format_timestamp::format_timestamp_timeago,
    },
    warp_runner::{RayGunCmd, WarpCmd},
    STATIC_ARGS, WARP_CMD_CH,
};

use super::sidebar::build_participants_names;

struct ComposeData {
    active_chat: Chat,
    message_groups: Vec<state::MessageGroup>,
    other_participants: Vec<Identity>,
    active_participant: Identity,
    subtext: String,
    is_favorite: bool,
    first_image: String,
    other_participants_names: String,
    active_media: bool,
    platform: Platform,
}

impl PartialEq for ComposeData {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

#[derive(PartialEq, Props)]
struct ComposeProps {
    #[props(!optional)]
    data: Option<Rc<ComposeData>>,
}

#[allow(non_snake_case)]
pub fn Compose(cx: Scope) -> Element {
    log::trace!("rendering compose");
    let state = use_shared_state::<State>(cx)?;
    let data = get_compose_data(cx);
    let data2 = data.clone();

    cx.render(rsx!(
        div {
            id: "compose",
            Topbar {
                with_back_button: state.read().ui.is_minimal_view() || state.read().ui.sidebar_hidden,
                with_currently_back: state.read().ui.sidebar_hidden,
                onback: move |_| {
                    let current = state.read().ui.sidebar_hidden;
                    state.write().mutate(Action::SidebarHidden(!current));
                },
                controls: cx.render(rsx!(get_controls{data: data2})),
                get_topbar_children{data: data.clone()}
            },
            data.as_ref().and_then(|data| data.active_media.then(|| rsx!(
                MediaPlayer {
                    settings_text: get_local_text("settings.settings"), 
                    enable_camera_text: get_local_text("media-player.enable-camera"),
                    fullscreen_text: get_local_text("media-player.fullscreen"),
                    popout_player_text: get_local_text("media-player.popout-player"),
                    screenshare_text: get_local_text("media-player.screenshare"),
                    end_text: get_local_text("uplink.end"),
                },
            ))),
            get_messages{data: data.clone()},
            get_chatbar{data: data}
        }
    ))
}

fn get_compose_data(cx: Scope) -> Option<Rc<ComposeData>> {
    let state = use_shared_state::<State>(cx)?;

    // the Compose page shouldn't be called before chats is initialized. but check here anyway.
    if !state.read().chats.initialized {
        return None;
    }

    let s = state.read();
    let active_chat = match s.get_active_chat() {
        Some(c) => c,
        None => return None,
    };
    let message_groups = s.get_sort_messages(&active_chat);
    let other_participants = s.get_without_me(&active_chat.participants);
    let active_participant = other_participants
        .first()
        .cloned()
        .expect("chat should have at least 2 participants");
    let subtext = active_participant.status_message().unwrap_or_default();
    let is_favorite = s.is_favorite(&active_chat);
    let first_image = active_participant.graphics().profile_picture();
    let other_participants_names = build_participants_names(&other_participants);
    let active_media = Some(active_chat.id) == s.chats.active_media;

    // TODO: Pending new message divider implementation
    // let _new_message_text = LOCALES
    //     .lookup(&*APP_LANG.read(), "messages.new")
    //     .unwrap_or_default();

    let platform = match active_participant.platform() {
        warp::multipass::identity::Platform::Desktop => Platform::Desktop,
        warp::multipass::identity::Platform::Mobile => Platform::Mobile,
        _ => Platform::Headless, //TODO: Unknown
    };

    let data = Rc::new(ComposeData {
        active_chat,
        message_groups,
        other_participants,
        active_participant,
        subtext,
        is_favorite,
        first_image,
        other_participants_names,
        active_media,
        platform,
    });

    Some(data)
}

fn get_controls(cx: Scope<ComposeProps>) -> Element {
    let state = use_shared_state::<State>(cx)?;
    let desktop = use_window(cx);
    let data = cx.props.data.clone();
    let active_chat = data.as_ref().map(|x| x.active_chat.clone());
    let active_chat2 = active_chat.clone();
    cx.render(rsx!(
        Button {
            icon: Icon::Heart,
            disabled: data.is_none(),
            aria_label: "Add to Favorites".into(),
            appearance: data
                .as_ref()
                .map(|data| if data.is_favorite {
                    Appearance::Primary
                } else {
                    Appearance::Secondary
                })
                .unwrap_or(Appearance::Secondary),
            tooltip: cx.render(rsx!(Tooltip {
                arrow_position: ArrowPosition::Top,
                text: get_local_text("favorites.add"),
            })),
            onpress: move |_| {
                if let Some(chat) = active_chat.clone() {
                    state.write().mutate(Action::ToggleFavorite(chat));
                }
            }
        },
        Button {
            icon: Icon::PhoneArrowUpRight,
            disabled: data.is_none(),
            aria_label: "Call".into(),
            appearance: Appearance::Secondary,
            tooltip: cx.render(rsx!(Tooltip {
                arrow_position: ArrowPosition::Top,
                text: get_local_text("uplink.call"),
            })),
            onpress: move |_| {
                if let Some(chat) = active_chat2.clone() {
                    state
                        .write_silent()
                        .mutate(Action::ClearPopout(desktop.clone()));
                    state.write_silent().mutate(Action::DisableMedia);
                    state.write().mutate(Action::SetActiveMedia(chat.id));
                }
            }
        },
        (!state.read().ui.is_minimal_view()).then(|| rsx!(Button {
            icon: Icon::VideoCamera,
            disabled: data.is_none(),
            aria_label: "Videocall".into(),
            appearance: Appearance::Secondary,
            tooltip: cx.render(rsx!(Tooltip {
                arrow_position: ArrowPosition::Top,
                text: get_local_text("uplink.video-call"),
            })),
        },))
    ))
}

fn get_topbar_children(cx: Scope<ComposeProps>) -> Element {
    let data = cx.props.data.clone();
    let is_loading = data.is_none();
    let other_participants_names = data
        .as_ref()
        .map(|x| x.other_participants_names.clone())
        .unwrap_or_default();
    let subtext = data.as_ref().map(|x| x.subtext.clone()).unwrap_or_default();
    cx.render(rsx!(
        if let Some(data) = data {
            if data.other_participants.len() < 2 {rsx! (
                UserImage {
                    loading: false,
                    platform: data.platform,
                    status: convert_status(&data.active_participant.identity_status()),
                    image: data.first_image.clone(),
                }
            )} else {rsx! (
                UserImageGroup {
                    loading: false,
                    participants: build_participants(&data.other_participants),
                }
            )}
        } else {rsx! (
            UserImageGroup {
                loading: true,
                participants: vec![]
            }
        )}
        div {
            class: "user-info",
            if is_loading {
                rsx!(
                    div {
                        class: "skeletal-bars",
                        div {
                            class: "skeletal skeletal-bar",
                        },
                        div {
                            class: "skeletal skeletal-bar",
                        },
                    }
                )
            } else {
                rsx! (
                    p {
                        class: "username",
                        "{other_participants_names}"
                    },
                    p {
                        class: "status",
                        "{subtext}"
                    }
                )
            }
        }
    ))
}

enum MessagesCommand {
    // contains the emoji reaction
    React((raygun::Message, String)),
}

fn get_messages(cx: Scope<ComposeProps>) -> Element {
    log::trace!("get_messages");
    let state = use_shared_state::<State>(cx)?;
    let user = state.read().account.identity.did_key();

    let script = include_str!("./script.js");
    use_eval(cx)(script.to_string());

    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<MessagesCommand>| {
        //to_owned![];
        async move {
            while let Some(cmd) = rx.next().await {
                match cmd {
                    MessagesCommand::React((message, emoji)) => {
                        let warp_cmd_tx = WARP_CMD_CH.tx.clone();
                        let (tx, rx) = futures::channel::oneshot::channel();

                        let mut reactions = message.reactions();
                        reactions.retain(|x| x.users().contains(&user));
                        reactions.retain(|x| x.emoji().eq(&emoji));
                        let reaction_state = if reactions.is_empty() {
                            ReactionState::Add
                        } else {
                            ReactionState::Remove
                        };
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::React {
                            conversation_id: message.conversation_id(),
                            message_id: message.id(),
                            reaction_state,
                            emoji,
                            rsp: tx,
                        })) {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        if res.is_err() {
                            // failed to add/remove reaction
                        }
                    }
                }
            }
        }
    });

    let data = match &cx.props.data {
        Some(d) => d.clone(),
        None => {
            return cx.render(rsx!(
                div {
                    id: "messages",
                    MessageGroupSkeletal {},
                    MessageGroupSkeletal { alt: true }
                }
            ))
        }
    };

    cx.render(rsx!(
        div {
            id: "messages",
            div {
                data.message_groups.iter().map(|group| {
                    let messages = &group.messages;
                    let active_chat = data.active_chat.clone();
                    let last_message = messages.last().unwrap().message.clone();
                    let sender = state.read().get_friend_identity(&group.sender);
                    let active_language = state.read().settings.language.clone();
                    let platform = match sender.platform() {
                        warp::multipass::identity::Platform::Desktop => Platform::Desktop,
                        warp::multipass::identity::Platform::Mobile => Platform::Mobile,
                        _ => Platform::Headless //TODO: Unknown
                    };
                    let status = convert_status(&sender.identity_status());

                    rsx!(
                        MessageGroup {
                            user_image: cx.render(rsx!(
                                UserImage {
                                    platform: platform,
                                    status: status
                                }
                            )),
                            timestamp: format_timestamp_timeago(last_message.date(), active_language),
                            with_sender: if sender.username().is_empty() { get_local_text("messages.you") } else { sender.username()},
                            remote: group.remote,
                            messages.iter().map(|grouped_message| {
                                let message = grouped_message.message.clone();
                                let message2 = message.clone();
                                let reply_message = grouped_message.message.clone();
                                let active_chat = active_chat.clone();
                                rsx! (
                                    ContextMenu {
                                        id: format!("message-{}", message.id()),
                                        items: cx.render(rsx!(
                                            ContextItem {
                                                icon: Icon::ArrowLongLeft,
                                                text: get_local_text("messages.reply"),
                                                onpress: move |_| {
                                                    state.write().mutate(Action::StartReplying(active_chat.clone(), reply_message.clone()));
                                                }
                                            },
                                            ContextItem {
                                                icon: Icon::FaceSmile,
                                                text: get_local_text("messages.react"),
                                                //TODO: let the user pick a reaction
                                                onpress: move |_| {
                                                      // using "like" for now
                                                    ch.send(MessagesCommand::React((message2.clone(), "👍".into())));
                                                }
                                            },
                                        )),
                                        Message {
                                            remote: group.remote,
                                            with_text: message.value().join("\n"),
                                            order: if grouped_message.is_first { Order::First } else if grouped_message.is_last { Order::Last } else { Order::Middle },
                                        }
                                    }
                                )
                            })
                        }
                    )
                })
            }
        },
    ))
}

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

fn get_chatbar(cx: Scope<ComposeProps>) -> Element {
    log::trace!("get_chatbar");
    let state = use_shared_state::<State>(cx)?;
    let data = cx.props.data.clone();
    let is_loading = data.is_none();
    let input = use_ref(cx, Vec::<String>::new);
    let should_clear_input = use_state(cx, || false);
    let active_chat_id = data.as_ref().map(|d| d.active_chat.id);

    // todo: use this to render the typing indicator
    let _users_typing = active_chat_id
        .and_then(|id| state.read().chats.all.get(&id).cloned())
        .map(|chat| {
            chat.participants
                .iter()
                .filter(|x| chat.typing_indicator.contains_key(&x.did_key()))
                .map(|x| x.username())
                .collect::<Vec<_>>()
        });

    let msg_ch = use_coroutine(cx, |mut rx: UnboundedReceiver<(Vec<String>, Uuid)>| {
        //to_owned![];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some((msg, conv_id)) = rx.next().await {
                let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::SendMessage {
                    conv_id,
                    msg,
                    rsp: tx,
                })) {
                    log::error!("failed to send warp command: {}", e);
                    continue;
                }

                let rsp = rx.await.expect("command canceled");
                if let Err(e) = rsp {
                    log::error!("failed to send message: {}", e);
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
    use_future(cx, &active_chat_id.clone(), |current_chat| async move {
        loop {
            tokio::time::sleep(Duration::from_secs(STATIC_ARGS.typing_indicator_refresh)).await;
            if let Some(c) = current_chat {
                local_typing_ch1.send(TypingIndicator::Refresh(c));
            }
        }
    });

    let msg_valid =
        |msg: &[String]| !msg.is_empty() && msg.iter().any(|line| !line.trim().is_empty());

    let extensions = &state.read().ui.extensions;

    let ext_renders = {
        let mut list = vec![];
        let extensions = extensions.iter();
        for (_, proxy) in extensions {
            list.push(rsx!(proxy.extension.render(cx)));
        }

        list
    };

    cx.render(rsx!(Chatbar {
        loading: is_loading,
        placeholder: get_local_text("messages.say-something-placeholder"),
        reset: should_clear_input.clone(),
        onchange: move |v: String| {
            *input.write_silent() = v.lines().map(|x| x.to_string()).collect::<Vec<String>>();
            if let Some(id) = &active_chat_id {
                local_typing_ch.send(TypingIndicator::Typing(*id));
            }
        },
        onreturn: move |_| {
            local_typing_ch.send(TypingIndicator::NotTyping);

            let msg = input.read().clone();
            // clearing input here should prevent the possibility to double send a message if enter is pressed twice
            input.write().clear();
            should_clear_input.set(true);

            if !msg_valid(&msg) {
                return;
            }
            let id = match active_chat_id {
                Some(i) => i,
                None => return,
            };

            if STATIC_ARGS.use_mock {
                state.write().mutate(Action::MockSend(id, msg));
            } else {
                msg_ch.send((msg, id));
            }
        },
        controls: cx.render(rsx!(
            Button {
                icon: Icon::ChevronDoubleRight,
                disabled: is_loading,
                appearance: Appearance::Secondary,
                onpress: move |_| {
                    local_typing_ch.send(TypingIndicator::NotTyping);

                    let msg = input.read().clone();
                    // clearing input here should prevent the possibility to double send a message if enter is pressed twice
                    input.write().clear();
                    should_clear_input.set(true);

                    if !msg_valid(&msg) {
                        return;
                    }

                    let id = match active_chat_id {
                        Some(i) => i,
                        None => return,
                    };

                    if STATIC_ARGS.use_mock {
                        state.write().mutate(Action::MockSend(id, msg));
                    } else {
                        msg_ch.send((msg, id));
                    }
                },
                tooltip: cx.render(rsx!(Tooltip {
                    arrow_position: ArrowPosition::Bottom,
                    text: get_local_text("uplink.send"),
                })),
            },
            // Load extensions
            for node in ext_renders {
                rsx!(node)
            }
        )),
        with_replying_to: data
            .map(|data| {
                let active_chat = data.active_chat.clone();
                cx.render(rsx!(active_chat.clone().replying_to.map(|msg| {
                    let our_did = state.read().account.identity.did_key();
                    let mut participants = data.active_chat.participants.clone();
                    participants.retain(|p| p.did_key() == msg.sender());
                    let msg_owner = participants.first();
                    let (platform, status) = get_platform_and_status(msg_owner);

                    rsx!(
                        Reply {
                            label: get_local_text("messages.replying"),
                            remote: our_did != msg.sender(),
                            onclose: move |_| {
                                state.write().mutate(Action::CancelReply(active_chat.clone()))
                            },
                            message: msg.value().join("\n"),
                            UserImage {
                                platform: platform,
                                status: status,
                            },
                        }
                    )
                })))
            })
            .unwrap_or(None),
        with_file_upload: cx.render(rsx!(Button {
            icon: Icon::Plus,
            disabled: is_loading,
            appearance: Appearance::Primary,
            tooltip: cx.render(rsx!(Tooltip {
                arrow_position: ArrowPosition::Bottom,
                text: get_local_text("files.upload"),
            }))
        }))
    }))
}

fn get_platform_and_status(msg_sender: Option<&Identity>) -> (Platform, Status) {
    let sender = match msg_sender {
        Some(identity) => identity,
        None => return (Platform::Desktop, Status::Offline),
    };
    let user_sender = build_user_from_identity(sender.clone());
    (user_sender.platform, user_sender.status)
}
