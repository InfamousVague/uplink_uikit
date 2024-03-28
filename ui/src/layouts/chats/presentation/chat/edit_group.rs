#[allow(unused_imports)]
use std::collections::{BTreeMap, HashMap, HashSet};

use common::{
    icons::outline::Shape as Icon,
    icons::Icon as IconElement,
    language::get_local_text,
    state::{Identity, State},
    warp_runner::{RayGunCmd, WarpCmd},
    WARP_CMD_CH,
};
use dioxus::prelude::*;
use futures::{channel::oneshot, StreamExt};
use kit::{
    components::{
        indicator::{Platform, Status},
        user_image::UserImage,
    },
    elements::{
        button::Button,
        input::{Input, Options},
        Appearance,
    },
    layout::topbar::Topbar,
};
use tracing::log;
use uuid::Uuid;
use warp::crypto::DID;
#[derive(PartialEq, Clone)]
enum EditGroupAction {
    Add,
    Remove,
}

enum ChanCmd {
    AddParticipants,
    RemoveParticipants,
}

#[allow(non_snake_case)]
pub fn EditGroup() -> Element {
    log::trace!("rendering edit_group");
    let state = use_context::<Signal<State>>();
    let minimal = state.read().ui.metadata.minimal_view;
    // Search Input
    let mut friend_prefix = use_signal(String::new);

    // show the ADD or REMOVE components, default to Remove
    let mut edit_group_action = use_signal(|| EditGroupAction::Remove);
    let conv_id = state.read().get_active_chat().unwrap().id;

    let creator_id = state.read().get_active_chat()?.creator.clone()?;

    let friends_did_already_in_group = state.read().get_active_chat().unwrap().participants;

    let friends_list: HashMap<DID, Identity> = HashMap::from_iter(
        state
            .read()
            .friend_identities()
            .iter()
            .map(|id| (id.did_key(), id.clone())),
    );

    let mut friends_group_list = friends_list.clone();
    let mut friends_not_in_group_list = friends_list;

    friends_group_list.retain(|did_key, _| friends_did_already_in_group.contains(did_key));
    friends_not_in_group_list.retain(|did_key, _| !friends_did_already_in_group.contains(did_key));

    friends_not_in_group_list.retain(|_, friend| {
        friend
            .username()
            .to_ascii_lowercase()
            .contains(&friend_prefix().to_ascii_lowercase())
    });
    friends_group_list.retain(|_, friend| {
        friend
            .username()
            .to_ascii_lowercase()
            .contains(&friend_prefix().to_ascii_lowercase())
    });

    // convert back to vec
    let mut friends: Vec<Identity> = if edit_group_action() == EditGroupAction::Add {
        friends_not_in_group_list.values().cloned().collect()
    } else {
        friends_group_list.values().cloned().collect()
    };

    friends.sort_by_key(|d| d.username());

    let add_friends = rsx!(Button {
        aria_label: "edit-group-add-members".to_string(),
        icon: Icon::UserPlus,
        appearance: Appearance::Secondary,
        text: if minimal {
            String::new()
        } else {
            get_local_text("uplink.add-members")
        },
        onpress: move |_| {
            edit_group_action.set(EditGroupAction::Add);
        }
    });

    let remove_friends = rsx!(Button {
        aria_label: "edit-group-remove-members".to_string(),
        icon: Icon::UserMinus,
        appearance: Appearance::Secondary,
        text: if minimal {
            String::new()
        } else {
            get_local_text("uplink.current-members")
        },
        onpress: move |_| {
            edit_group_action.set(EditGroupAction::Remove);
        }
    });

    let creator_did2 = creator_id.clone();
    let am_i_group_creator = creator_id == state.read().did_key();

    rsx!(
        div {
            id: "edit-members",
            aria_label: "edit-members",
            Topbar {
                with_back_button: false,
                div {
                    class: "search-input",
                    Input {
                        // todo: filter friends on input
                        placeholder: get_local_text("uplink.search-placeholder"),
                        disabled: false,
                        aria_label: "friend-search-input".to_string(),
                        icon: Icon::MagnifyingGlass,
                        options: Options {
                            with_clear_btn: true,
                            react_to_esc_key: true,
                            clear_on_submit: false,
                            ..Options::default()
                        },
                        onchange: move |(v, _): (String, _)| {
                            friend_prefix.set(v);
                        },
                    },
                    if edit_group_action() == EditGroupAction::Remove {
                       { rsx! {
                            {add_friends},
                        }}
                    } else {
                        {rsx! {
                            {remove_friends},
                        }}
                    },
                },

            },
            {rsx!(
                div {
                    class: "friend-list vertically-scrollable",
                    aria_label: "friends-list",
                    if !friends.is_empty() {
                        {rsx!(
                            div {
                                class: "friend-list vertically-scrollable",
                                aria_label: "friends-list",
                                div {
                                    key: "{friend-group}",
                                    class: "friend-group",
                                    aria_label: "friend-group",
                                    {friends.iter().map(
                                        |_friend| {
                                            let is_group_creator = creator_did2.clone() == _friend.clone().did_key();
                                            rsx!(
                                                friend_row {
                                                    add_or_remove: if edit_group_action() == EditGroupAction::Add {
                                                        "add".to_string()
                                                    } else {
                                                        "remove".to_string()
                                                    },
                                                    friend_is_group_creator: is_group_creator,
                                                    am_i_group_creator: am_i_group_creator,
                                                    friend: _friend.clone(),
                                                    minimal: minimal,
                                                    conv_id: conv_id,
                                                }
                                            )
                                        }
                                    )},
                                }
                            }
                        )}
                    } else {
                        {rsx!(
                            div {
                                class: "friend-group",
                                {get_local_text("uplink.nothing-here")}
                            }
                        )}
                    }
                }
            )}
        }
    )
}

#[derive(Props, Clone, Eq, PartialEq)]
pub struct FriendRowProps {
    add_or_remove: String,
    friend_is_group_creator: bool,
    am_i_group_creator: bool,
    minimal: bool,
    friend: Identity,
    conv_id: Uuid,
}

/* Friend Row with add/remove button functionality */
fn friend_row(props: FriendRowProps) -> Element {
    let _friend = props.friend.clone();
    let mut selected_friends: Signal<HashSet<DID>> = use_signal(HashSet::new);
    let conv_id = props.conv_id;
    let ch = use_coroutine(|mut rx: UnboundedReceiver<ChanCmd>| {
        to_owned![selected_friends, conv_id];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    ChanCmd::AddParticipants => {
                        let recipients: Vec<DID> = selected_friends().iter().cloned().collect();
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::AddGroupParticipants {
                                conv_id,
                                recipients,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }
                        let res = rx.await.expect("command canceled");
                        if let Err(e) = res {
                            log::error!("failed to add new recipients to a group: {}", e);
                        }
                    }
                    ChanCmd::RemoveParticipants => {
                        let recipients: Vec<DID> = selected_friends().iter().cloned().collect();
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::RemoveGroupParticipants {
                                conv_id,
                                recipients,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }
                        let res = rx.await.expect("command canceled");
                        if let Err(e) = res {
                            log::error!("failed to remove recipients from a group: {}", e);
                        }
                    }
                }
            }
        }
    });

    rsx!(
        div {
            class: "friend-container",
            aria_label: "Friend Container",
            UserImage {
                platform: Platform::from(_friend.platform()),
                status: Status::from(_friend.identity_status()),
                image: _friend.profile_picture()
            },
            div {
                class: "flex-1",
                p {
                    class: "ellipsis-overflow",
                    aria_label: "friend-username",
                    {_friend.username()},
                },
            },
            if props.friend_is_group_creator {
                {rsx!(
                    div {
                        class: "group-creator-container",
                        IconElement {
                            icon: Icon::Satellite
                        }
                        span {
                            class: "group-creator-text",
                            {get_local_text("messages.group-creator-label")}
                        }
                    }
                )}
            }
            if props.am_i_group_creator || props.add_or_remove == "add" {
                {rsx!(Button {
                    aria_label: if props.add_or_remove == "add" {
                        get_local_text("uplink.add")
                    } else {
                        get_local_text("uplink.remove")
                    },
                    icon: if props.add_or_remove == "add" {
                        Icon::UserPlus
                    } else {
                        Icon::UserMinus
                    },
                    text: if props.minimal { String::new() }
                        else if props.add_or_remove == "add" {
                            get_local_text("uplink.add")
                        } else {
                            get_local_text("uplink.remove")
                        }
                    ,
                    onpress: move |_| {
                        let mut friends = selected_friends();
                        friends.clear();
                        selected_friends.set(vec![_friend.did_key()].into_iter().collect());
                        if props.add_or_remove == "add" {
                            ch.send(ChanCmd::AddParticipants);
                        } else {
                            ch.send(ChanCmd::RemoveParticipants);
                        }
                    }
                })}
            }
        }
    )
}
