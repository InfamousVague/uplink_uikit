use std::collections::HashSet;
use std::rc::Rc;

use crate::components::friends::friend::Friend;
use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use common::{
    state::{Action, State},
    warp_runner::{MultiPassCmd, WarpCmd},
    STATIC_ARGS, WARP_CMD_CH,
};
use dioxus::prelude::*;
use futures::{channel::oneshot, StreamExt};
use kit::components::indicator::Status;
use kit::{
    components::{
        context_menu::{ContextItem, ContextMenu},
        user_image::UserImage,
    },
    elements::label::Label,
};
use warp::crypto::DID;
use warp::multipass::identity::Relationship;

use tracing::log;

enum ChanCmd {
    AcceptRequest(DID),
    DenyRequest(DID),
}

#[allow(non_snake_case)]
pub fn PendingFriends() -> Element {
    let mut state = use_context::<Signal<State>>();
    let friends_list = state.peek().incoming_fr_identities();
    let deny_in_progress: Signal<HashSet<DID>> = use_signal(HashSet::new);
    let accept_in_progress: Signal<HashSet<DID>> = use_signal(HashSet::new);

    let ch = use_coroutine(|mut rx: UnboundedReceiver<ChanCmd>| {
        to_owned![deny_in_progress, accept_in_progress];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                //tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
                match cmd {
                    ChanCmd::AcceptRequest(identity) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::AcceptRequest {
                                did: identity.clone(),
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            accept_in_progress().remove(&identity);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        accept_in_progress().remove(&identity);
                        if let Err(e) = rsp {
                            log::error!("failed to accept request: {}", e);
                        }
                    }
                    ChanCmd::DenyRequest(identity) => {
                        let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::DenyRequest {
                                did: identity.clone(),
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            deny_in_progress().remove(&identity);
                            continue;
                        }

                        let rsp = rx.await.expect("command canceled");
                        deny_in_progress().remove(&identity);
                        if let Err(e) = rsp {
                            log::error!("failed to deny request: {}", e);
                        }
                    }
                }
            }
        }
    });

    if friends_list.is_empty() {
        return rsx!({});
    }
    rsx!(div {
        class: "friends-list",
        aria_label: "Incoming Requests List",
        Label {
            text: get_local_text("friends.incoming_requests"),
            aria_label: "incoming-list-label".to_string(),
        },
        {friends_list.into_iter().map(|friend| {
            let friend = Rc::new(friend);
            let _username = friend.username();
            let _status_message = friend.status_message().unwrap_or_default();
            let did = friend.did_key();
            let did2 = did.clone();
            let did_suffix = friend.short_id().to_string();
            let platform = friend.platform().into();
            let friend2 = friend.clone();
            let friend3 = friend.clone();
            let friend4 = friend.clone();

            let any_button_disabled = accept_in_progress().contains(&did)
                ||  deny_in_progress().contains(&did);

            rsx!(
                ContextMenu {
                    id: format!("{did}-friend-listing"),
                    key: "{did}-friend-listing",
                    devmode: state.peek().configuration.read().developer.developer_mode,
                    items: rsx!(
                        ContextItem {
                            danger: true,
                            icon: Icon::Check,
                            text: get_local_text("friends.accept"),
                            aria_label: "friends-accept".to_string(),
                            should_render: !any_button_disabled,
                            onpress: move |_| {
                                if STATIC_ARGS.use_mock {
                                    state.write().mutate(Action::AcceptRequest(&friend));
                                } else {
                                    accept_in_progress().insert(friend.did_key());
                                    ch.send(ChanCmd::AcceptRequest(friend.did_key()));
                                }
                            }
                        },
                        ContextItem {
                            danger: true,
                            icon: Icon::XMark,
                            aria_label: "friends-deny".to_string(),
                            text: get_local_text("friends.deny"),
                            should_render: !any_button_disabled,
                            onpress: move |_| {
                                if STATIC_ARGS.use_mock {
                                    state.write().mutate(Action::DenyRequest(&did));
                                } else {
                                    deny_in_progress().insert(did.clone());
                                    ch.send(ChanCmd::DenyRequest(did.clone()));
                                }
                            }
                        }
                    ),
                    Friend {
                        aria_label: _username.clone(),
                        username: _username,
                        suffix: did_suffix,
                        status_message: _status_message,
                        relationship: {
                            let mut relationship = Relationship::default();
                            relationship.set_received_friend_request(true);
                            relationship
                        },
                        user_image: rsx! (
                            UserImage {
                                platform: platform,
                                status: Status::from(friend2.identity_status()),
                                image: friend2.profile_picture()
                            }
                        ),
                        accept_button_disabled: accept_in_progress().contains(&did2),
                        remove_button_disabled: deny_in_progress().contains(&did2),
                        onaccept: move |_| {
                            if STATIC_ARGS.use_mock {
                                state.write().mutate(Action::AcceptRequest(&friend4));
                            } else {
                                accept_in_progress().insert(friend4.did_key());
                                ch.send(ChanCmd::AcceptRequest(friend4.did_key()));
                            }

                        },
                        onremove: move |_| {
                            if STATIC_ARGS.use_mock {
                                state.write().mutate(Action::AcceptRequest(&friend3));
                            } else {
                                deny_in_progress().insert(friend3.did_key());
                                ch.send(ChanCmd::DenyRequest(friend3.did_key()));
                            }
                        }
                    }
                }
            )
        })}
    })
}
