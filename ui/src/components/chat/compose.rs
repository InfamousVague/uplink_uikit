use std::{rc::Rc};

use dioxus::prelude::*;

use futures::{StreamExt, channel::oneshot};
use kit::{layout::{topbar::Topbar, chatbar::{Chatbar, Reply}}, components::{user_image::UserImage, indicator::{Status, Platform}, context_menu::{ContextMenu, ContextItem}, message_group::{MessageGroup, MessageGroupSkeletal}, message::{Message, Order}, user_image_group::UserImageGroup}, elements::{button::Button, tooltip::{Tooltip, ArrowPosition}, Appearance}, icons::Icon};

use dioxus_desktop::{use_window, use_eval};
use shared::language::get_local_text;
use uuid::Uuid;


use crate::{state::{State, Action, Chat, Identity, self}, components::{media::player::MediaPlayer}, utils::{format_timestamp::format_timestamp_timeago, convert_status, build_participants, build_user_from_identity}, WARP_CMD_CH, warp_runner::{WarpCmd, commands::RayGunCmd}, logger, STATIC_ARGS};

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
    platform: Platform
}

impl PartialEq for ComposeData {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

#[derive(PartialEq, Props)]
struct ComposeProps {
    #[props(!optional)]
    data: Option<Rc<ComposeData>>
}

#[allow(non_snake_case)]
pub fn Compose(cx: Scope) -> Element {
    logger::trace("rendering compose");
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
        None => return None
    };
    let message_groups = s.get_sort_messages(&active_chat);
    let other_participants = s.get_without_me(&active_chat.participants);
    let active_participant = other_participants.first().cloned().expect("chat should have at least 2 participants");
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
        _ => Platform::Headless //TODO: Unknown
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
        platform
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
            appearance: data.as_ref().map(|data| if data.is_favorite { Appearance::Primary } else { Appearance::Secondary }).unwrap_or(Appearance::Secondary),
            tooltip: cx.render(rsx!(
                Tooltip { 
                    arrow_position: ArrowPosition::Top, 
                    text: get_local_text("favorites.add"),
                }
            )),
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
            tooltip: cx.render(rsx!(
                Tooltip { 
                    arrow_position: ArrowPosition::Top, 
                    text: get_local_text("uplink.call"),
                }
            )),
            onpress: move |_| {
                if let Some(chat) = active_chat2.clone() {
                    state.write_silent().mutate(Action::ClearPopout(desktop.clone()));
                    state.write_silent().mutate(Action::DisableMedia);
                    state.write().mutate(Action::SetActiveMedia(chat.id));
                }
            }
        },
        (!state.read().ui.is_minimal_view()).then(|| rsx!(
            Button {
                icon: Icon::VideoCamera,
                disabled: data.is_none(),
                aria_label: "Videocall".into(),
                appearance: Appearance::Secondary,
                tooltip: cx.render(rsx!(
                    Tooltip { 
                        arrow_position: ArrowPosition::Top, 
                        text: get_local_text("uplink.video-call"),
                    }
                )),
            },
        ))
    ))
}

fn get_topbar_children(cx: Scope<ComposeProps>) -> Element {
    let data = cx.props.data.clone();
    let is_loading = data.is_none();
    let other_participants_names = data.as_ref().map(|x| x.other_participants_names.clone()).unwrap_or_default();
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

fn get_messages(cx: Scope<ComposeProps>) -> Element {
    let state = use_shared_state::<State>(cx)?;

    let script = include_str!("./script.js");
    use_eval(cx)(script.to_string());

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
                                                //TODO: Wire to state
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


fn get_chatbar(cx: Scope<ComposeProps>) -> Element {
    logger::trace("get_chatbar");
    let state = use_shared_state::<State>(cx)?;
    let data = cx.props.data.clone();
    let loading = data.is_none();
    let input = use_ref(cx, Vec::<String>::new);
    let reset_input = use_ref(cx, || false);
    let active_chat_id = data.as_ref().map(|d| d.active_chat.id);

    let should_clear_input = *reset_input.read();

    if should_clear_input {
        *reset_input.write_silent() = false;
    }
    
    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<(Vec<String>, Option<Uuid>)>| {
        //to_owned![];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some((msg, chat_id)) = rx.next().await {
                let conv_id = match chat_id {
                    Some(c) => c,
                    None => {
                        continue;
                    }
                };

                // don't send empty messages
                if msg.is_empty() {
                    continue;
                }

                // this is also an empty message
                if msg.iter().all(|line| line.is_empty())  {
                    continue;
                } 
               
                let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                warp_cmd_tx
                    .send(WarpCmd::RayGun(RayGunCmd::SendMessage {
                        conv_id,
                        msg,
                        rsp: tx,
                    }))
                    .expect("failed to send cmd");

                let rsp = rx.await.expect("command canceled");
                if let Err(e) = rsp {
                    logger::error(&format!("failed to send message: {}", e));
                }

             
            }
        }
    });
   
    cx.render(rsx!(
        Chatbar {
            loading: loading,
            placeholder: get_local_text("messages.say-something-placeholder"),
            reset: should_clear_input,
            onchange: move |v: String| {
                *input.write_silent() = v.lines().map(|x| x.to_string()).collect::<Vec<String>>();
            },
            onreturn: move |_| {
                if STATIC_ARGS.use_mock {
                    todo!();
                } else {
                    ch.send((input.read().clone(), active_chat_id));
                }
                reset_input.set(true);
            },
            controls: cx.render(rsx!(
                Button {
                    icon: Icon::ChevronDoubleRight,
                    disabled: loading,
                    appearance: Appearance::Secondary,
                    onpress: move |_| {
                        if STATIC_ARGS.use_mock {
                            todo!();
                        } else {
                            ch.send((input.read().clone(), active_chat_id));
                        }
                        reset_input.set(true);
                    },
                    tooltip: cx.render(rsx!(
                        Tooltip { 
                            arrow_position: ArrowPosition::Bottom, 
                            text: get_local_text("uplink.send"),
                        }
                    )),
                },
            )),
            with_replying_to: data.map(|data| {
                let active_chat = data.active_chat.clone();

                cx.render(rsx!(
                    active_chat.clone().replying_to.map(|msg|
                    {
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
                    )}
                )
                ))
            }).unwrap_or(None),
            with_file_upload: cx.render(rsx!(
                Button {
                    icon: Icon::Plus,
                    disabled: loading,
                    appearance: Appearance::Primary,
                    tooltip: cx.render(rsx!(
                        Tooltip { 
                            arrow_position: ArrowPosition::Bottom, 
                            text: get_local_text("files.upload"),
                        }
                    ))
                }
            ))
        }
    ))
}

fn get_platform_and_status(msg_sender: Option<&Identity>) -> (Platform, Status) {
    let sender = match msg_sender {
        Some(identity) => identity,
        None => return (Platform::Desktop, Status::Offline)
    };
    let user_sender = build_user_from_identity(sender.clone());
    (user_sender.platform, user_sender.status)
}