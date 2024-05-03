use std::collections::HashMap;

use common::{
    icons::outline::Shape as Icon,
    icons::Icon as IconElement,
    language::get_local_text,
    state::{Chat, Identity, State},
};
use dioxus::prelude::*;

use kit::{
    components::{
        indicator::{Platform, Status},
        user_image::UserImage,
    },
    elements::input::{Input, Options},
};
use tracing::log;
use warp::crypto::DID;
#[derive(Props, Clone, PartialEq)]
pub struct Props {
    #[props(!optional)]
    active_chat: Option<Chat>,
    quickprofile_data: Signal<Option<(f64, f64, Identity, bool)>>,
}

#[allow(non_snake_case)]

pub fn GroupUsers(props: Props) -> Element {
    log::trace!("rendering group_users");
    let state = use_context::<Signal<State>>();
    let mut friend_prefix = use_signal(String::new);

    let quickprofile_data = &props.quickprofile_data;

    let active_chat = match props.active_chat.as_ref() {
        Some(r) => r,
        None => return rsx!(div {}),
    };
    if active_chat.participants.is_empty() {
        return rsx!(div {});
    }

    let participant_dids = Vec::from_iter(active_chat.participants.iter().cloned());
    let group_participants = state.peek().get_identities(&participant_dids);
    let hash_map = HashMap::from_iter(
        group_participants
            .iter()
            .map(|ident| (ident.did_key(), ident.clone())),
    );
    let _friends_in_group = State::get_friends_by_first_letter(hash_map);
    let creator_id_vector = Vec::from_iter(active_chat.creator.iter().cloned());
    let creator_id = creator_id_vector.first().cloned()?;

    use_effect(move || {
        let _ = eval(
            r#"
                const right_clickable = document.getElementsByClassName("friend-container");
                const prevent_default = function (ev) { ev.preventDefault(); };
                for (var i = 0; i < right_clickable.length; i++) {
                    //Disable default right click actions (opening the inspect element dropdown)
                    right_clickable.item(i).removeEventListener("contextmenu", prevent_default);
                    right_clickable.item(i).addEventListener("contextmenu", prevent_default);
                }"#,
        );
    });
    rsx!(
        div {
            id: "group-users",
            aria_label: "group-users",
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
                }
            }
            render_friends {
                group_participants: group_participants,
                name_prefix: friend_prefix,
                creator: creator_id,
                is_dev: state.peek().configuration.read().developer.developer_mode,
                context_data: *quickprofile_data,
            }
        }
    )
}

#[derive(Props, Clone, PartialEq)]
pub struct FriendsProps {
    group_participants: Vec<Identity>,
    name_prefix: Signal<String>,
    creator: DID,
    is_dev: bool,
    context_data: Signal<Option<(f64, f64, Identity, bool)>>,
}

fn render_friends(props: FriendsProps) -> Element {
    let name_prefix = props.name_prefix.read();
    let mut group_participants = props.group_participants.clone();
    // reduce group participants vector to just the name_prefix matched
    group_participants.retain(|friend| {
        friend
            .username()
            .to_ascii_lowercase()
            .contains(&name_prefix.to_ascii_lowercase())
    });

    rsx!(
        div {
            class: "friend-list vertically-scrollable",
            aria_label: "friends-list",
            if !group_participants.is_empty() {
                {
                    let friend_group = "friend-group".to_string();
                    rsx!(
                    div {
                        key: "{friend_group}",
                        class: "friend-group",
                        {group_participants.iter().map(|_friend| {
                            let friendid = _friend.did_key();
                            let creator = props.creator.clone();
                            rsx!(render_friend {
                                friend: _friend.clone(),
                                is_creator: friendid == creator,
                                is_dev: props.is_dev,
                                context_data: props.context_data,
                            }
                        )})}
                    }
                )}
            } else {
               { rsx!(
                    div {
                        class: "friend-group",
                        {get_local_text("uplink.nothing-here")}
                    }
                )}
            }
        }
    )
}

#[derive(PartialEq, Props, Clone)]
pub struct FriendProps {
    friend: Identity,
    is_creator: bool,
    is_dev: bool,
    context_data: Signal<Option<(f64, f64, Identity, bool)>>,
}
fn render_friend(props: FriendProps) -> Element {
    let friend = use_signal(|| props.friend.clone());
    let mut context_data = props.context_data;
    rsx!(
        div {
            class: "friend-container",
            aria_label: "Friend Container",
            oncontextmenu: move |e| {
                context_data.set(Some((e.page_coordinates().x, e.page_coordinates().y, friend().to_owned(), true)));
            },
            UserImage {
                platform: Platform::from(props.friend.platform()),
                status: Status::from(props.friend.identity_status()),
                image: props.friend.profile_picture(),
                oncontextmenu: move |e: Event<MouseData>| {
                    context_data.set(Some((e.page_coordinates().x, e.page_coordinates().y, friend().to_owned(), true)));
                }
            },
            div {
                class: "flex-1",
                p {
                    class: "ellipsis-overflow",
                    aria_label: "friend-username",
                    {friend().username()},
                },
            },
            if props.is_creator {
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
        }
    )
}
