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
use kit::components::indicator::Status;
use kit::{
    components::{
        context_menu::{ContextItem, ContextMenu},
        user_image::UserImage,
    },
    elements::label::Label,
};
use warp::{crypto::DID, error::Error, multipass::identity::Relationship};

use tracing::log;

#[allow(non_snake_case)]
pub fn BlockedUsers() -> Element {
    let mut state = use_context::<Signal<State>>();
    let block_list = state.read().blocked_fr_identities();
    let unblock_in_progress: Signal<HashSet<DID>> = use_signal(|| HashSet::new());

    let ch = use_coroutine(|mut rx: UnboundedReceiver<DID>| {
        to_owned![unblock_in_progress];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(did) = rx.next().await {
                let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::Unblock {
                    did: did.clone(),
                    rsp: tx,
                })) {
                    log::error!("failed to send warp command: {}", e);
                    unblock_in_progress().remove(&did);
                    continue;
                }

                let rsp = rx.await.expect("command canceled");
                unblock_in_progress().remove(&did);
                if let Err(e) = rsp {
                    match e {
                        Error::PublicKeyIsntBlocked => {}
                        _ => {
                            log::error!("failed to unblock user: {}", e);
                        }
                    }
                }
            }
        }
    });

    if block_list.is_empty() {
        return rsx!({});
    }
    rsx!(div {
        class: "friends-list",
        aria_label: "Blocked List",
        Label {
            text: get_local_text("friends.blocked"),
            aria_label: "blocked-list-label".to_string(),
        },
        {block_list.into_iter().map(|blocked_user| {
            let did = blocked_user.did_key();
            let did_suffix = blocked_user.short_id().to_string();
            let unblock_user = blocked_user.clone();
            let unblock_user_clone = unblock_user.clone();
            let platform = blocked_user.platform().into();
            let mut relationship = Relationship::default();
            relationship.set_blocked(true);
            rsx!(
                ContextMenu {
                    id: format!("{did}-friend-listing"),
                    key: "{did}-friend-listing",
                    devmode: state.read().configuration.developer.developer_mode,
                    items: rsx!(
                        ContextItem {
                            danger: true,
                            icon: Icon::XMark,
                            aria_label: "friends-unblock".to_string(),
                            text: get_local_text("friends.unblock"),
                            onpress: move |_| {
                                if STATIC_ARGS.use_mock {
                                    state.write().mutate(Action::Unblock(&unblock_user.did_key()));
                                } else {
                                    unblock_in_progress().insert(unblock_user.did_key());
                                    ch.send(unblock_user.clone().did_key());
                                }
                            }
                        },
                    ),
                    Friend {
                        username: blocked_user.username(),
                        aria_label: blocked_user.username(),
                        suffix: did_suffix,
                        status_message: blocked_user.status_message().unwrap_or_default(),
                        relationship: relationship,
                        remove_button_disabled: unblock_in_progress().contains(&blocked_user.did_key()),
                        user_image: rsx! (
                            UserImage {
                                platform: platform,
                                status: Status::from(blocked_user.identity_status()),
                                image: blocked_user.profile_picture()
                            }
                        ),
                        onremove: move |_| {
                            if STATIC_ARGS.use_mock {
                                state.write().mutate(Action::Unblock(&unblock_user_clone.did_key()));
                            } else {
                                unblock_in_progress().insert(unblock_user_clone.did_key());
                                ch.send(unblock_user_clone.clone().did_key());
                            }
                        }
                    }
                }
            )
        })}
    })
}
