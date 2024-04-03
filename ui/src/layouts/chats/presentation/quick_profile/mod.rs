use dioxus::{html::input_data::keyboard_types::Code, prelude::*};

use dioxus_router::prelude::use_navigator;
use futures::{channel::oneshot, StreamExt};

use kit::{
    components::context_menu::{ContextItem, ContextMenu, IdentityHeader},
    elements::{input::Input, label::Label, range::Range},
};

use common::{
    icons::outline::Shape as Icon,
    state::{Identity, ToastNotification},
    warp_runner::{BlinkCmd, MultiPassCmd},
};
use common::{
    state::{Action, Chat, State},
    warp_runner::{RayGunCmd, WarpCmd},
    WARP_CMD_CH,
};

use common::language::get_local_text;

use uuid::Uuid;
use warp::{crypto::DID, error::Error};

use tracing::log;

use crate::{
    components::{friends::friends_list::ShareFriendsModal, settings::sidebar::Page},
    UplinkRoute,
};

pub const USER_VOL_MIN: f32 = 0.25;
pub const USER_VOL_MAX: f32 = 5.0;

#[derive(Props)]
pub struct QuickProfileProps<'a> {
    id: &'a String,
    did_key: &'a DID,
    update_script: &'a UseState<String>,
    children: Element<'a>,
}

#[allow(clippy::large_enum_variant)]
enum QuickProfileCmd {
    CreateConversation(Option<Chat>, DID),
    RemoveFriend(DID),
    BlockFriend(DID),
    UnBlockFriend(DID),
    RemoveDirectConvs(DID),
    Chat(Option<Chat>, Vec<String>),
    AdjustVolume(DID, f32),
    SendFriendRequest(DID, Vec<Identity>),
}

// Create a quick profile context menu
#[allow(non_snake_case)]
pub fn QuickProfileContext<'a>(cx: Scope<'a, QuickProfileProps<'a>>) -> Element<'a> {
    let state = use_shared_state::<State>(cx)?;
    let settings_page = use_shared_state::<Page>(cx)?;
    let id = cx.props.id;
    let share_did = use_state(cx, || None);

    let identity = state
        .read()
        .get_identity(cx.props.did_key)
        .unwrap_or_default();
    let remove_identity = identity.clone();
    let block_identity = identity.clone();

    let did = &identity.did_key();
    let did_cloned = did.clone();
    let did_cloned_2 = did.clone();
    let chat_of = state.read().get_chat_with_friend(identity.did_key());
    let chat_send = chat_of.clone();

    let chat_is_current = match state.read().get_active_chat() {
        Some(c) => match &chat_of {
            Some(cO) => c.eq(cO),
            None => false,
        },
        None => false,
    };

    let eval = use_eval(cx);
    use_future(cx, cx.props.update_script, |update_script| {
        to_owned![eval];
        async move {
            if !update_script.is_empty() {
                _ = eval(&update_script);
            }
        }
    });

    let is_self = state.read().get_own_identity().did_key().eq(did);
    let is_friend = state.read().has_friend_with_did(did);
    let in_vc = state
        .read()
        .get_active_chat()
        .map(|call| call.participants.contains(did))
        .unwrap_or_default();
    let blocked = state.read().is_blocked(did);
    let volume = state
        .read()
        .settings
        .user_volumes
        .get(did)
        .cloned()
        .unwrap_or(1.0);

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

    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<QuickProfileCmd>| {
        to_owned![chat_with, state];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    QuickProfileCmd::CreateConversation(chat, did) => {
                        // verify chat exists
                        let chat = match chat {
                            Some(c) => c.id,
                            None => {
                                // if not, create the chat
                                let (tx, rx) = oneshot::channel();
                                if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(
                                    RayGunCmd::CreateConversation {
                                        recipient: did,
                                        rsp: tx,
                                    },
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
                    QuickProfileCmd::RemoveFriend(did) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::RemoveFriend {
                                did,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(_) => {
                                state.write().mutate(Action::AddToastNotification(
                                    ToastNotification::init(
                                        "".into(),
                                        get_local_text("friends.removed"),
                                        None,
                                        2,
                                    ),
                                ));
                            }
                            Err(e) => {
                                log::error!("failed to remove friend: {}", e);
                            }
                        }
                    }
                    QuickProfileCmd::BlockFriend(did) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        if let Err(e) = warp_cmd_tx
                            .send(WarpCmd::MultiPass(MultiPassCmd::Block { did, rsp: tx }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(_) => {
                                state.write().mutate(Action::AddToastNotification(
                                    ToastNotification::init(
                                        "".into(),
                                        get_local_text("friends.blocked-action"),
                                        None,
                                        2,
                                    ),
                                ));
                            }
                            Err(e) => {
                                log::error!("failed to block friend: {}", e);
                            }
                        }
                    }
                    QuickProfileCmd::UnBlockFriend(did) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        if let Err(e) = warp_cmd_tx
                            .send(WarpCmd::MultiPass(MultiPassCmd::Unblock { did, rsp: tx }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok(_) => {
                                state.write().mutate(Action::AddToastNotification(
                                    ToastNotification::init(
                                        "".into(),
                                        get_local_text("friends.unblocked"),
                                        None,
                                        2,
                                    ),
                                ));
                            }
                            Err(e) => {
                                log::error!("failed to unblock friend: {}", e);
                            }
                        }
                    }
                    QuickProfileCmd::RemoveDirectConvs(recipient) => {
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
                    QuickProfileCmd::Chat(chat, msg) => {
                        let c = match chat {
                            Some(c) => c.id,
                            None => return,
                        };
                        let msg_vec = msg.clone();
                        let (tx, rx) = oneshot::channel();
                        let cmd = RayGunCmd::SendMessage {
                            conv_id: c,
                            msg,
                            attachments: Vec::new(),
                            rsp: tx,
                        };
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(cmd)) {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        match rsp {
                            Ok((id, _)) => {
                                state
                                    .write_silent()
                                    .increment_outgoing_messages_for(c, id, msg_vec);
                            }
                            Err(e) => {
                                log::error!("failed to send message: {}", e);
                            }
                        }
                        chat_with.set(Some(c));
                    }
                    QuickProfileCmd::AdjustVolume(user, volume) => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::AdjustVolume {
                            user: user.clone(),
                            volume,
                            rsp: tx,
                        })) {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                state
                                    .write_silent()
                                    .settings
                                    .user_volumes
                                    .insert(user, volume);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to unmute self: {e}");
                            }
                        }
                    }
                    QuickProfileCmd::SendFriendRequest(id, outgoing_requests) => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        let _ = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::RequestFriend {
                            id: id.to_string(),
                            outgoing_requests,
                            rsp: tx,
                        }));
                        let res = rx.await.expect("failed to get response from warp_runner");
                        match res {
                            Ok(_) => {
                                state.write().mutate(Action::AddToastNotification(
                                    ToastNotification::init(
                                        "".into(),
                                        get_local_text("friends.request-sent"),
                                        None,
                                        2,
                                    ),
                                ));
                            }
                            Err(e) => match e {
                                Error::PublicKeyIsBlocked => {
                                    log::warn!("add friend failed: {}", e);
                                    state.write().mutate(Action::AddToastNotification(
                                        ToastNotification::init(
                                            "".into(),
                                            get_local_text("friends.key-blocked"),
                                            None,
                                            2,
                                        ),
                                    ));
                                }
                                _ => {
                                    //The other errors are covered by button already
                                    log::error!("add friend failed: {}", e);
                                    state.write().mutate(Action::AddToastNotification(
                                        ToastNotification::init(
                                            "".into(),
                                            get_local_text("friends.add-failed"),
                                            None,
                                            2,
                                        ),
                                    ));
                                }
                            },
                        }
                    }
                }
            }
        }
    });

    cx.render(rsx!(div{
        class: "quick-profile-context",
        ContextMenu {
        id: format!("{id}"),
        items: cx.render(rsx!(
            IdentityHeader {
                sender_did: identity.did_key()
            },
            div {
                class: "profile-container",
                div {
                    id: "profile-name",
                    aria_label: "profile-name",
                    p {
                        class: "text",
                        aria_label: "profile-name-value",
                        format!("{}", if identity.username().is_empty() { get_local_text("uplink.unknown") } else { identity.username() })
                    }
                }
                identity.status_message().and_then(|s|{
                    cx.render(rsx!(
                        div {
                            id: "profile-status",
                            aria_label: "profile-status",
                            p {
                                class: "text",
                                aria_label: "profile-status-value",
                                s
                            }
                        }
                    ))
                }),
            }
            div {
                class: "profile-context-items",
                if is_self {
                    rsx!(hr{},
                        ContextItem {
                        icon: Icon::UserCircle,
                        aria_label: "quick-profile-self-edit".into(),
                        text: get_local_text("quickprofile.self-edit"),
                        onpress: move |_| {
                            settings_page.write().set(Page::Profile);
                            router.replace(UplinkRoute::SettingsLayout {});
                        }
                    })
                } else {
                    rsx!(
                    if state.read().configuration.developer.experimental_features && in_vc {
                        rsx!(
                            div {
                                class: "range-container",
                                Label {
                                    text: get_local_text("quickprofile.volume")
                                },
                                Range {
                                    aria_label: "range-quick-profile-speaker".into(),
                                    initial_value: volume,
                                    min: USER_VOL_MIN,
                                    max: USER_VOL_MAX,
                                    step: 0.1,
                                    no_num: true,
                                    icon_left: Icon::Speaker,
                                    icon_right: Icon::SpeakerWave,
                                    onchange: move |val| {
                                        ch.send(QuickProfileCmd::AdjustVolume(did_cloned.clone(), val));
                                    }
                                }
                            },
                            hr{}
                        )
                    }
                        /*ContextItem {
                        icon: Icon::UserCircle,
                        text: get_local_text("quickprofile.profile"),
                        // TODO: Show a profile popup
                    },*/
                    if is_friend {
                        rsx!(
                            if !chat_is_current {
                                rsx!(
                                    ContextItem {
                                    icon: Icon::ChatBubbleBottomCenterText,
                                    aria_label: "quick-profile-message".into(),
                                    text: get_local_text("quickprofile.message"),
                                    onpress: move |_| {
                                        ch.send(QuickProfileCmd::CreateConversation(chat_of.clone(), identity.did_key()));
                                    }
                                })
                            }
                            /*ContextItem {
                                icon: Icon::PhoneArrowUpRight,
                                text: get_local_text("quickprofile.call"),
                                // TODO: Impl missing
                            }*/
                        )
                    } else {
                        let outgoing = state.read().outgoing_fr_identities();
                        let disabled = outgoing.contains(&identity);
                        rsx!(
                            ContextItem {
                                icon: Icon::Plus,
                                aria_label: "quick-profile-friend-request".into(),
                                text: if disabled {get_local_text("quickprofile.pending-friend-request")} else {get_local_text("quickprofile.friend-request")},
                                disabled: disabled,
                                onpress: move |_| {
                                    ch.send(QuickProfileCmd::SendFriendRequest(identity.did_key(), outgoing.clone()));
                                }
                            }
                        )
                    }
                    if is_friend {
                        rsx!(ContextItem {
                            danger: false,
                            icon: Icon::Link,
                            text: get_local_text("friends.share"),
                            aria_label: "friends-share".into(),
                            onpress: move |_| {
                                share_did.set(Some(did_cloned_2.clone()));
                            }
                        })
                    },
                    if is_friend {
                        rsx!(ContextItem {
                            danger: true,
                            icon: Icon::UserMinus,
                            text: get_local_text("quickprofile.friend-remove"),
                            aria_label: "quick-profile-friend-remove".into(),
                            onpress: move |_| {
                                ch.send(QuickProfileCmd::RemoveFriend(remove_identity.did_key()));
                                ch.send(QuickProfileCmd::RemoveDirectConvs(remove_identity.did_key()));
                            }
                        })
                    }
                    ContextItem {
                        danger: true,
                        icon: if blocked {Icon::UserBlocked} else {Icon::UserBlock},
                        aria_label: if blocked {"quick-profile-unblock".into()} else {"quick-profile-block".into()},
                        text: if blocked {get_local_text("quickprofile.unblock")} else {get_local_text("quickprofile.block")},
                        onpress: move |_| {
                            if blocked {
                                ch.send(QuickProfileCmd::UnBlockFriend(block_identity.did_key()));
                            } else {
                                ch.send(QuickProfileCmd::BlockFriend(block_identity.did_key()));
                                ch.send(QuickProfileCmd::RemoveDirectConvs(block_identity.did_key()));
                            }
                        }
                    },
                    if is_friend && !chat_is_current {
                        rsx!(
                            hr{},
                            Input {
                                placeholder: get_local_text("quickprofile.chat-placeholder"),
                                disable_onblur: true,
                                onreturn: move |(val, _,_): (String,bool,Code)|{
                                    ch.send(QuickProfileCmd::Chat(chat_send.to_owned(), vec![val]));
                                    let script = format!(r#"document.getElementById("{id}-context-menu").classList.add("hidden")"#);
                                    let _ = eval(&script);
                                }
                            }
                        )
                    })
                }
            }
        ))
        ,
        share_did.as_ref().map(|_|{
            match state.read().get_active_chat() {
                Some(chat) => rsx!(ShareFriendsModal{
                    did: share_did.clone(),
                    excluded_chat: chat.id
                }),
                None => rsx!(ShareFriendsModal{
                    did: share_did.clone(),
                })
            }
        }),
        &cx.props.children
    }}))
}
