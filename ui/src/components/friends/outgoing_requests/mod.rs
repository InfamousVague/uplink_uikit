use chrono::{Utc, Duration};
use dioxus::prelude::*;
use kit::{elements::label::Label, components::{context_menu::{ContextMenu, ContextItem}, user_image::UserImage, indicator::{Platform}}, icons::Icon};
use rand::Rng;
use shared::language::get_local_text;
use warp::multipass::identity::Relationship;
use crate::{state::{State, Action}, components::friends::friend::{Friend}, utils::convert_status};

#[allow(non_snake_case)]
pub fn OutgoingRequests(cx: Scope) -> Element {
    let state: UseSharedState<State> = use_shared_state::<State>(cx).unwrap();
    let friends_list = state.read().friends.outgoing_requests.clone();

    cx.render(rsx! (
        div {
            class: "friends-list",
            aria_label: "Outgoing Requests List",
            Label {
                text: get_local_text("friends.outgoing_requests"),
            },
            friends_list.into_iter().map(|friend| {
                let did = friend.did_key();
                let did_suffix: String = did.to_string().chars().rev().take(6).collect();
                let mut rng = rand::thread_rng();
                let friend_clone = friend.clone();
                let friend_clone_clone = friend.clone();
                let platform = match friend.platform() {
                    warp::multipass::identity::Platform::Desktop => Platform::Desktop,
                    warp::multipass::identity::Platform::Mobile => Platform::Mobile,
                    _ => Platform::Headless //TODO: Unknown
                };
                rsx!(
                    ContextMenu {
                        id: format!("{}-friend-listing", did),
                        key: "{did}-friend-listing",
                        items: cx.render(rsx!(
                            ContextItem {
                                danger: true,
                                icon: Icon::XMark,
                                text: get_local_text("friends.cancel"),
                                onpress: move |_| {
                                    state.write().mutate(Action::CancelRequest(friend_clone_clone.clone()));
                                }
                            },
                        )),
                        Friend {
                            username: friend.username(),
                            suffix: did_suffix,
                            status_message: friend.status_message().unwrap_or_default(), 
                            relationship: {
                                let mut relationship = Relationship::default();
                                relationship.set_sent_friend_request(true);
                                relationship
                            },
                            request_datetime: Utc::now() - Duration::days(rng.gen_range(0..30)),
                            user_image: cx.render(rsx! (
                                UserImage {
                                    platform: platform,
                                    status: convert_status(&friend.identity_status()),
                                    image: friend.graphics().profile_picture()
                                }
                            )),
                            onremove: move |_| {
                                state.write().mutate(Action::CancelRequest(friend_clone.clone()));
                            }
                        }
                    }
                )
            })
        }
    ))
}