use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;
use dioxus_router::prelude::use_navigator;
use futures::{channel::oneshot, StreamExt};
use kit::{
    components::{
        context_menu::{ContextItem, ContextMenu},
        message::format_text,
        user::User,
        user_image::UserImage,
        user_image_group::UserImageGroup,
    },
    elements::{
        button::Button,
        checkbox::Checkbox,
        input::{Input, Options},
        label::Label,
        Appearance,
    },
    layout::modal::Modal,
};

use common::{get_images_dir, icons::outline::Shape as Icon, language::get_local_text_with_args};
use common::{language::get_local_text, state::Identity};
use common::{
    state::{Action, Chat, State},
    warp_runner::{MultiPassCmd, RayGunCmd, WarpCmd},
    STATIC_ARGS, WARP_CMD_CH,
};
use uuid::Uuid;
use warp::{
    crypto::DID,
    multipass::identity::Relationship,
    raygun::{self, ConversationType},
};

use tracing::log;

use crate::{
    components::friends::friend::{Friend, SkeletalFriend},
    utils::build_participants,
    UplinkRoute,
};

#[allow(clippy::large_enum_variant)]
enum ChanCmd {
    CreateConversation { recipient: DID, chat: Option<Chat> },
    RemoveFriend(DID),
    BlockFriend(DID),
    // will remove direct conversations involving the friend
    RemoveDirectConvs(DID),
}

#[allow(non_snake_case)]
pub fn Friends(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;
    let reset_filter = use_state(cx, || false);
    let friend_filter = use_state(cx, String::new);
    if *reset_filter.get() {
        friend_filter.set(String::new());
        reset_filter.set(false);
    }
    let filter = friend_filter.get().to_lowercase();
    let friends_all = state.read().friend_identities();
    let friends_list = HashMap::from_iter(
        friends_all
            .iter()
            .filter(|id| filter.is_empty() || id.username().to_lowercase().starts_with(&filter))
            .map(|id| (id.did_key(), id.clone())),
    );
    let block_in_progress: &UseState<HashSet<DID>> = use_state(cx, HashSet::new);
    let remove_in_progress: &UseState<HashSet<DID>> = use_state(cx, HashSet::new);

    let share_did = use_state(cx, || None);

    let friends = State::get_friends_by_first_letter(friends_list);

    let router = use_navigator(cx);

    let chat_with: &UseState<Option<Uuid>> = use_state(cx, || None);

    if let Some(id) = *chat_with.get() {
        chat_with.set(None);
        state.write().mutate(Action::ChatWith(&id, true));
        if state.read().ui.is_minimal_view() {
            state.write().mutate(Action::SidebarHidden(true));
        }
        router.replace(UplinkRoute::ChatLayout {});
    }

    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<ChanCmd>| {
        to_owned![chat_with, block_in_progress, remove_in_progress];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                //tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
                match cmd {
                    ChanCmd::CreateConversation { chat, recipient } => {
                        // verify chat exists
                        let chat = match chat {
                            Some(c) => c.id,
                            None => {
                                // if not, create the chat
                                let (tx, rx) = oneshot::channel();
                                if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(
                                    RayGunCmd::CreateConversation { recipient, rsp: tx },
                                )) {
                                    log::error!("failed to send warp command: {}", e);
                                    continue;
                                }

                                let rsp = rx.await.expect("command canceled");

                                match rsp {
                                    Ok(c) => c,
                                    Err(e) => {
                                        log::error!("failed to create conversation: {}", e);
                                        continue;
                                    }
                                }
                            }
                        };
                        chat_with.set(Some(chat));
                    }
                    ChanCmd::RemoveFriend(did) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::RemoveFriend {
                                did: did.clone(),
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            remove_in_progress.make_mut().remove(&did);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        remove_in_progress.make_mut().remove(&did);
                        if let Err(e) = rsp {
                            log::error!("failed to remove friend: {}", e);
                        }
                    }
                    ChanCmd::BlockFriend(did) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::Block {
                            did: did.clone(),
                            rsp: tx,
                        })) {
                            log::error!("failed to send warp command: {}", e);
                            block_in_progress.make_mut().remove(&did);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        block_in_progress.make_mut().remove(&did);
                        if let Err(e) = rsp {
                            // todo: display message to user
                            log::error!("failed to block friend: {}", e);
                        }
                    }
                    ChanCmd::RemoveDirectConvs(recipient) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::RemoveDirectConvs {
                                recipient: recipient.clone(),
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        if let Err(e) = rsp {
                            log::error!(
                                "failed to remove conversation with friend {}: {}",
                                recipient,
                                e
                            );
                        }
                    }
                }
            }
        }
    });

    let image_path = get_images_dir()
        .unwrap_or_default()
        .join("mascot")
        .join("party.webp")
        .to_str()
        .map(|x| x.to_string())
        .unwrap_or_default();

    cx.render(rsx! (
        div {
            class: "friends-list",
            aria_label: "Friends List",
            Label {
                text: get_local_text("friends.friends"),
                aria_label: "friends-list-label".into(),
            },
            (!friends_all.is_empty()).then(||{
                rsx!(Input {
                    placeholder: get_local_text("friends.search-placeholder"),
                    icon: Icon::MagnifyingGlass,
                    options: Options {
                        with_clear_btn: true,
                        clear_validation_on_no_chars: true,
                        clear_on_submit: false,
                        ..Options::default()
                    },
                    disable_onblur: true,
                    reset: reset_filter.clone(),
                    onchange: |(s, _)| {
                        friend_filter.set(s);
                    },
                    aria_label: "Search Friend".into()
                })
            }),
            (friends.is_empty()).then(|| rsx! (
                div {
                    class: "empty-friends-list",
                    img {
                        src: "{image_path}"
                    },
                }
            )),
            share_did.is_some().then(||{
                rsx!(ShareFriendsModal{
                    did: share_did.clone()
                })
            }),
            friends.into_iter().map(|(letter, sorted_friends)| {
                let group_letter = letter.to_string();
                rsx!(
                    div {
                        key: "friend-group-{group_letter}",
                        Label {
                            text: letter.into(),
                            aria_label: letter.into()
                        },
                        sorted_friends.into_iter().map(|friend| {
                            let did = friend.did_key();
                            let chat = state.read().get_chat_with_friend(friend.did_key());
                            let chat2 = chat.clone();
                            let chat3 = chat.clone();
                            let favorite = chat.clone().map(|c| state.read().is_favorite(&c));
                            let did_suffix = friend.short_id().to_string();
                            let remove_friend = friend.clone();
                            let remove_friend_2 = friend.clone();
                            let chat_with_friend = friend.clone();
                            let block_friend = friend.clone();
                            let block_friend_2 = friend.clone();
                            let context_friend = friend.clone();
                            let share_friend = friend.clone();
                            let mut relationship = Relationship::default();
                            relationship.set_friends(true);
                            let platform = friend.platform().into();
                            rsx!(
                                ContextMenu {
                                    id: format!("{did}-friend-listing"),
                                    key: "{did}-friend-listing",
                                    devmode: state.read().configuration.developer.developer_mode,
                                    items: cx.render(rsx!(
                                        ContextItem {
                                            icon: Icon::ChatBubbleBottomCenterText,
                                            text: get_local_text("uplink.chat"),
                                            aria_label: "friends-chat".into(),
                                            onpress: move |_| {
                                                ch.send(ChanCmd::CreateConversation{recipient: context_friend.did_key(), chat: chat2.clone()});
                                            }
                                        },
                                        ContextItem {
                                            danger: false,
                                            icon: Icon::Link,
                                            text: get_local_text("friends.share"),
                                            aria_label: "friends-share".into(),
                                            onpress: move |_| {
                                                share_did.set(Some(share_friend.did_key()));
                                            }
                                        },
                                        if let Some(f) = favorite {
                                            rsx!(ContextItem {
                                                icon: if f {Icon::HeartSlash} else {Icon::Heart},
                                                text: get_local_text(if f {"favorites.remove"} else {"favorites.favorites"}),
                                                aria_label: if f {"favorites-remove".into()} else {"favorites-add".into()},
                                                onpress: move |_| {
                                                    // can't favorite a non-existent conversation
                                                    // todo: don't even allow favoriting from the friends page unless there's a conversation
                                                    if let Some(c) = &chat {
                                                        state.write().mutate(Action::ToggleFavorite(&c.id));
                                                    }
                                                }
                                            })
                                        },
                                        hr{}
                                        ContextItem {
                                            danger: true,
                                            icon: Icon::UserMinus,
                                            text: get_local_text("uplink.remove"),
                                            aria_label: "friends-remove".into(),
                                            should_render: !remove_in_progress.current().contains(&remove_friend.did_key()),
                                            onpress: move |_| {
                                                let did = remove_friend.did_key();
                                                if STATIC_ARGS.use_mock {
                                                    state.write().mutate(Action::RemoveFriend(&did));
                                                } else {
                                                    remove_in_progress.make_mut().insert(did.clone());
                                                    ch.send(ChanCmd::RemoveFriend(did.clone()));
                                                    ch.send(ChanCmd::RemoveDirectConvs(did));
                                                }
                                            }
                                        },
                                        ContextItem {
                                            danger: true,
                                            icon: Icon::NoSymbol,
                                            text: get_local_text("friends.block"),
                                            aria_label: "friends-block".into(),
                                            should_render: !block_in_progress.current().contains(&block_friend.did_key()),
                                            onpress: move |_| {
                                                let did = block_friend.did_key();
                                                if STATIC_ARGS.use_mock {
                                                    state.write().mutate(Action::Block(&did));
                                                } else {
                                                    block_in_progress.make_mut().insert(did.clone());
                                                    ch.send(ChanCmd::BlockFriend(did.clone()));
                                                    ch.send(ChanCmd::RemoveDirectConvs(did));
                                                }
                                            }
                                        },
                                    )),
                                    Friend {
                                        username: friend.username(),
                                        aria_label: friend.username(),
                                        suffix: did_suffix,
                                        status_message: friend.status_message().unwrap_or_default(),
                                        relationship: relationship,
                                        block_button_disabled: block_in_progress.current().contains(&friend.did_key()),
                                        remove_button_disabled: remove_in_progress.current().contains(&friend.did_key()),
                                        user_image: cx.render(rsx! (
                                            UserImage {
                                                platform: platform,
                                                status: friend.identity_status().into(),
                                                image: friend.profile_picture()
                                            }
                                        )),
                                        onchat: move |_| {
                                            // this works for mock data because the conversations already exist
                                           ch.send(ChanCmd::CreateConversation{recipient: chat_with_friend.did_key(), chat: chat3.clone()});
                                        },
                                        onremove: move |_| {
                                            if STATIC_ARGS.use_mock {
                                                state.write().mutate(Action::RemoveFriend(&remove_friend_2.did_key()));
                                            } else {
                                                remove_in_progress.make_mut().insert(remove_friend_2.did_key());
                                                ch.send(ChanCmd::RemoveFriend(remove_friend_2.did_key()));
                                                ch.send(ChanCmd::RemoveDirectConvs(remove_friend_2.did_key()));
                                            }
                                        },
                                        onblock: move |_| {
                                            if STATIC_ARGS.use_mock {
                                                state.write().mutate(Action::Block(&block_friend_2.did_key()));
                                            } else {
                                                block_in_progress.make_mut().insert(block_friend_2.did_key());
                                                ch.send(ChanCmd::BlockFriend(block_friend_2.did_key()));
                                                ch.send(ChanCmd::RemoveDirectConvs(block_friend_2.did_key()));
                                            }
                                        }
                                    }
                                }
                            )
                        })
                    }
                )
            })
        }
    ))
}

// todo: remove this
#[allow(unused)]
#[allow(non_snake_case)]
pub fn FriendsSkeletal(cx: Scope) -> Element {
    cx.render(rsx!(
        div {
            class: "friends-list",
            Label {
                text: get_local_text("friends.friends"),
            },
            SkeletalFriend {},
            SkeletalFriend {},
            SkeletalFriend {},
        }
    ))
}

#[derive(PartialEq, Props)]
pub struct FriendProps {
    did: UseState<Option<DID>>,
    excluded_chat: Option<Uuid>,
}

pub fn ShareFriendsModal(cx: Scope<FriendProps>) -> Element {
    let state = use_shared_state::<State>(cx)?;
    let chats_selected = use_ref(cx, Vec::new);
    let ch = use_coroutine(
        cx,
        |mut rx: UnboundedReceiver<(DID, Vec<Uuid>)>| async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some((id, uuid)) = rx.next().await {
                let msg = vec![id.to_string()];
                let (tx, rx) = oneshot::channel();
                let cmd = RayGunCmd::SendMessageForSeveralChats {
                    convs_id: uuid,
                    msg,
                    attachments: Vec::new(),
                    rsp: tx,
                };
                if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(cmd)) {
                    log::error!("failed to send warp command: {}", e);
                    continue;
                }

                let rsp = rx.await.expect("command canceled");
                if let Err(e) = rsp {
                    log::error!("failed to send message: {}", e);
                }
            }
        },
    );
    let chats: Vec<_> = state
        .read()
        .chats_sidebar()
        .iter()
        .filter(|c| {
            cx.props
                .excluded_chat
                .map(|id| !c.id.eq(&id))
                .unwrap_or(true)
        })
        .cloned()
        .collect();
    cx.render(rsx!(Modal {
        open: cx.props.did.get().is_some(),
        onclose: move |_| cx.props.did.set(None),
        show_close_button: false,
        transparent: false,
        close_on_click_inside_modal: false,
        dont_pad: true,
        div {
            aria_label: "share-did-modal",
            class: "modal-share-friends",
            div {
                class: "modal-share-friends-header",
                padding: "12px",
                Label {
                    aria_label: "share-did-header".into(),
                    text: get_local_text("friends.select-chat"),
                },
                div {
                    class: "send-chat-button",
                    Button {
                        text: get_local_text("friends.share-to-chat"),
                        icon: Icon::Share,
                        aria_label: "share-to-chat-button".into(),
                        appearance: Appearance::Secondary,
                        disabled: chats_selected.read().is_empty(),
                        onpress: move |_| {
                            ch.send((cx.props.did.as_ref().unwrap().clone(), chats_selected.read().clone()));
                            cx.props.did.set(None);
                        },
                    },
                }
            }
            chats.is_empty().then(||{
                rsx!(div {
                    class: "modal-share-friend-empty",
                    aria_label: "modal-share-friend-empty",
                    get_local_text("messages.no-chats")
                })
            }),
            chats.iter().map(|chat| {
                let id = chat.id;
                let participants = state.read().chat_participants(chat);
                let other_participants =  state.read().remove_self(&participants);
                let user: Identity = other_participants.first().cloned().unwrap_or_default();
                let platform = user.platform().into();
                // todo: how to tell who is participating in a group chat if the chat has a conversation_name?
                let participants_name = match &chat.conversation_name {
                    Some(name) => name.clone(),
                    None => State::join_usernames(&other_participants)
                };
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
                let selected = chats_selected.read().contains(&id);
                rsx!(div {
                    class: format_args!("modal-share-friend {}", if selected {"share-friend-selected"} else {""}),
                    User {
                        aria_label: participants_name.clone(),
                        username: participants_name,
                        subtext: subtext_val,
                        timestamp: raygun::Message::default().date(),
                        active: false,
                        user_image: cx.render(rsx!(
                            div {
                                class: "modal-share-friend-image-group",
                                Checkbox {
                                    aria_label: "user-to-share-did-checkbox".into(),
                                    disabled: false,
                                    width: "1em".into(),
                                    height: "1em".into(),
                                    is_checked: selected,
                                    on_click: move |_| {
                                        chats_selected.with_mut(|v|{
                                            if !selected {
                                                v.push(id);
                                            } else {
                                                v.retain(|c|!c.eq(&id));
                                            }
                                        });
                                    }
                                },
                                match chat.conversation_type {
                                    ConversationType::Direct => rsx!(UserImage {
                                        platform: platform,
                                        status:  user.identity_status().into(),
                                        image: user.profile_picture(),
                                        typing: false,
                                    }),
                                    _ => rsx!(UserImageGroup {
                                        participants: build_participants(&participants),
                                        typing: false,
                                    })
                                }
                            }
                        )),
                        onpress: move |_| {
                            chats_selected.with_mut(|v|{
                                if !selected {
                                    v.push(id);
                                } else {
                                    v.retain(|c|!c.eq(&id));
                                }
                            });
                        }
                    }
                }
            )
            })
        }
    }))
}
