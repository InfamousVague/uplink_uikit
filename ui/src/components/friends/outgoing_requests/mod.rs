use std::collections::HashSet;

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
use kit::components::indicator::{Platform, Status};
use kit::{
    components::{
        context_menu::{ContextItem, ContextMenu},
        user_image::UserImage,
    },
    elements::label::Label,
};
use warp::{crypto::DID, multipass::identity::Relationship};

use tracing::log;

#[allow(non_snake_case)]
pub fn OutgoingRequests() -> Element {
    let mut state = use_context::<Signal<State>>();
    let friends_list = state.peek().outgoing_fr_identities();
    let remove_in_progress: Signal<HashSet<DID>> = use_signal(HashSet::new);

    let ch = use_coroutine(|mut rx: UnboundedReceiver<DID>| {
        to_owned![remove_in_progress];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(did) = rx.next().await {
                //tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
                let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::CancelRequest {
                    did: did.clone(),
                    rsp: tx,
                })) {
                    log::error!("failed to send warp command: {}", e);
                    remove_in_progress().remove(&did);
                    continue;
                }

                let rsp = rx.await.expect("command canceled");
                remove_in_progress().remove(&did);
                if let Err(e) = rsp {
                    log::error!("failed to cancel request: {}", e);
                }
            }
        }
    });

    if friends_list.is_empty() {
        return rsx!({});
    }
    rsx!(div {
        class: "friends-list",
        aria_label: "Outgoing Requests List",
        Label {
            text: get_local_text("friends.outgoing_requests"),
            aria_label: "outgoing-list-label".to_string(),
        },
        {friends_list.into_iter().map(|friend| {
            let did = friend.did_key();
            let did2 = did.clone();
            let did_suffix = friend.short_id().to_string();
            let platform = Platform::from(friend.platform());
            let any_button_disabled = remove_in_progress().contains(&did);
            rsx!(
                ContextMenu {
                    id: format!("{did}-friend-listing"),
                    key: "{did}-friend-listing",
                    devmode: state.peek().configuration.read().developer.developer_mode,
                    items: rsx!(
                        ContextItem {
                            danger: true,
                            icon: Icon::XMark,
                            aria_label: "friends-cancel".to_string(),
                            text: get_local_text("friends.cancel"),
                            should_render: !any_button_disabled,
                            onpress: move |_| {
                                if STATIC_ARGS.use_mock {
                                    state.write().mutate(Action::CancelRequest(&did));
                                } else {
                                    ch.send(did.clone());
                                }
                            }
                        },
                    ),
                    Friend {
                        username: friend.username(),
                        aria_label: friend.username(),
                        suffix: did_suffix,
                        status_message: friend.status_message().unwrap_or_default(),
                        relationship: {
                            let mut relationship = Relationship::default();
                            relationship.set_sent_friend_request(true);
                            relationship
                        },
                        remove_button_disabled: remove_in_progress().contains(&friend.did_key()),
                        user_image: rsx! (
                            UserImage {
                                platform: platform,
                                status: Status::from(friend.identity_status()),
                                image: friend.profile_picture()
                            }
                        ),
                        onremove: move |_| {
                            if STATIC_ARGS.use_mock {
                                state.write().mutate(Action::CancelRequest(&did2));
                            } else {
                                remove_in_progress().insert(did2.clone());
                                ch.send(did2.clone());
                            }
                        }
                    }
                }
            )
        })}
    })
}
