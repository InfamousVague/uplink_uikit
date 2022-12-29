use chrono::{Utc, Duration};
use dioxus::prelude::*;
use kit::{elements::label::Label, components::{context_menu::{ContextMenu, ContextItem}, user_image::UserImage, indicator::{Platform, Status}}, icons::Icon};
use rand::Rng;
use warp::multipass::identity::Relationship;
use crate::{state::{State, Action}, utils::language::get_local_text, components::friends::friend::{Friend}};

#[allow(non_snake_case)]
pub fn PendingFriends(cx: Scope) -> Element {
    let state: UseSharedState<State> = use_shared_state::<State>(cx).unwrap();
    let friends_list = state.read().friends.incoming_requests.clone();

    cx.render(rsx! (
        div {
            class: "friends-list",
            Label {
                text: get_local_text("friends.incoming_requests"),
            },
            friends_list.into_iter().map(|friend| {
                let mut rng = rand::thread_rng();
                let did = friend.did_key();
                let did_suffix: String = did.to_string().chars().rev().take(6).collect();
                let friend_clone = friend.clone();
                let friend_clone_clone = friend.clone();
                let friend_clone_clone_clone = friend.clone();
                let platform = match friend.platform() {
                    warp::multipass::identity::Platform::Desktop => Platform::Desktop,
                    warp::multipass::identity::Platform::Mobile => Platform::Mobile,
                    _ => Platform::Headless //TODO: Unknown
                };
                let status = match friend.identity_status() {
                    warp::multipass::identity::IdentityStatus::Online => Status::Online,
                    warp::multipass::identity::IdentityStatus::Away => Status::Idle,
                    warp::multipass::identity::IdentityStatus::Busy => Status::DoNotDisturb,
                    warp::multipass::identity::IdentityStatus::Offline => Status::Offline,
                };
                rsx!(
                    ContextMenu {
                        id: format!("{}-friend-listing", did),
                        key: "{did}-friend-listing",
                        items: cx.render(rsx!(
                            ContextItem {
                                danger: true,
                                icon: Icon::XMark,
                                text: get_local_text("friends.deny"),
                                onpress: move |_| {
                                    state.write().mutate(Action::DenyRequest(friend_clone_clone_clone.clone()));
                                }
                            },
                        )),
                        Friend {
                            username: friend.username(),
                            suffix: did_suffix,
                            status_message: friend.status_message().unwrap_or_default(), 
                            relationship: {
                                let mut relationship = Relationship::default();
                                relationship.set_received_friend_request(true);
                                relationship
                            },
                            request_datetime: Utc::now() - Duration::days(rng.gen_range(0..30)),
                            user_image: cx.render(rsx! (
                                UserImage {
                                    platform: platform,
                                    status: status,
                                    image: friend.graphics().profile_picture()
                                }
                            )),
                            onaccept: move |_| {
                                state.write().mutate(Action::AcceptRequest(friend_clone.clone()));
                            },
                            onremove: move |_| {
                               state.write().mutate(Action::DenyRequest(friend_clone_clone.clone()));
                            }
                        }
                    }
                )
            })
        }
    ))
}