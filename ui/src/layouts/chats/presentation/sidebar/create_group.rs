use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{layouts::chats::data::get_input_options, UplinkRoute};
use common::{
    icons::outline::Shape as Icon,
    language::get_local_text,
    state::{Action, Identity, State, ToastNotification},
    warp_runner::{RayGunCmd, WarpCmd},
    WARP_CMD_CH,
};
use dioxus::prelude::*;
use dioxus_router::prelude::use_navigator;
use futures::{channel::oneshot, StreamExt};
use kit::{
    components::user_image::UserImage,
    elements::{
        button::Button,
        checkbox::Checkbox,
        input::{Input, Options},
        label::Label,
        Appearance,
    },
};
use tracing::log;
use uuid::Uuid;
use warp::{crypto::DID, raygun::GroupSettings};

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    oncreate: EventHandler<MouseEvent>,
}

#[allow(non_snake_case)]
pub fn CreateGroup(props: Props) -> Element {
    log::trace!("rendering create_group");
    let state = use_context::<Signal<State>>();
    let router = use_navigator();
    let friend_prefix = use_signal(|| String::new());
    let selected_friends: Signal<HashSet<DID>> = use_signal(|| HashSet::new());
    let chat_with: Signal<Option<Uuid>> = use_signal(|| None);
    let group_name = use_signal(|| Some(String::new()));
    let friends_list = HashMap::from_iter(
        state
            .read()
            .friend_identities()
            .iter()
            .map(|id| (id.did_key(), id.clone())),
    );

    if let Some(id) = *chat_with.read() {
        chat_with.set(None);
        state.write().mutate(Action::ChatWith(&id, true));
        if state.read().ui.is_minimal_view() {
            state.write().mutate(Action::SidebarHidden(true));
        }
        router.replace(UplinkRoute::ChatLayout {});
    }

    // the leading underscore is to pass this to a prop named "friends"
    let _friends = State::get_friends_by_first_letter(friends_list);

    let ch = use_coroutine(|mut rx: UnboundedReceiver<()>| {
        to_owned![selected_friends, chat_with, group_name];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while rx.next().await.is_some() {
                let recipients: Vec<DID> = selected_friends.read().iter().cloned().collect();
                let group_name: Option<String> = group_name.read().as_ref().clone().cloned();
                let group_name_string = group_name.clone().unwrap_or_default();

                let (tx, rx) = oneshot::channel();
                let cmd = RayGunCmd::CreateGroupConversation {
                    recipients,
                    group_name: if group_name_string.is_empty()
                        || group_name_string.chars().all(char::is_whitespace)
                    {
                        None
                    } else {
                        group_name
                    },
                    settings: GroupSettings::default(),
                    rsp: tx,
                };

                if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(cmd)) {
                    log::error!("failed to send warp command: {}", e);
                    continue;
                }

                let rsp = rx.await.expect("command canceled");

                let id = match rsp {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("failed to create conversation: {}", e);
                        continue;
                    }
                };
                chat_with.set(Some(id));
            }
        }
    });

    rsx!(
        div {
            id: "create-group",
            aria_label: "Create Group",
            div {
                id: "create-group-name",
                aria_label: "create-group-name",
                class: "create-group-name",
                div {
                    align_items: "start",
                    Label {
                        aria_label: "group-name-label".into(),
                        text: get_local_text("messages.group-name"),
                    },
                }
                Input {
                        placeholder:  get_local_text("messages.group-name"),
                        default_text: group_name.read().clone().unwrap_or_default(),
                        aria_label: "groupname-input".into(),
                        focus_just_on_render: true,
                        options: Options {
                            with_clear_btn: true,
                            ..get_input_options()
                        },
                        onreturn: move |(v, is_valid, _): (String, bool, _)| {
                            if !is_valid {
                                group_name.set(None);
                                return;
                            }
                            group_name.set(Some(v));
                        },
                    },
            },
            div {
                class: "search-input",
                Label {
                    aria_label: "users-label".into(),
                    text: "Users".into(),
                },
                Input {
                    // todo: filter friends on input
                    placeholder: get_local_text("uplink.search-placeholder"),
                    disabled: false,
                    aria_label: "friend-search-input".into(),
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
                friends: _friends,
                name_prefix: friend_prefix.clone(),
                selected_friends: selected_friends.clone()
            },
            Button {
                text: get_local_text("messages.create-group-chat"),
                aria_label: "create-dm-button".into(),
                appearance: Appearance::Primary,
                onpress: move |e| {
                    log::info!("create dm button");
                    if group_name.read().is_some() {
                        ch.send(());
                        props.oncreate.call(e);
                    } else {
                        state
                        .write()
                        .mutate(common::state::Action::AddToastNotification(
                            ToastNotification::init(
                                "".into(),
                                get_local_text("messages.group-name-invalid"),
                                None,
                                3,
                            ),
                        ));
                    }
                }
            }
        }
    )
}

#[derive(PartialEq, Props, Clone)]
pub struct FriendsProps {
    friends: BTreeMap<char, Vec<Identity>>,
    name_prefix: Signal<String>,
    selected_friends: Signal<HashSet<DID>>,
}

fn render_friends(props: FriendsProps) -> Element {
    let name_prefix = props.name_prefix.read();
    rsx!(
        div {
            class: "friend-list vertically-scrollable",
            aria_label: "friends-list",
            {props.friends.iter().map(
                |(letter, sorted_friends)| {
                    let group_letter = letter.to_string();
                    rsx!(
                        div {
                            key: "friend-group-{group_letter}",
                            class: "friend-group",
                            {sorted_friends.iter().filter(|friend| {
                                let name = friend.username().to_lowercase();
                                if name.len() < name_prefix.len() {
                                    false
                                } else {
                                    name[..(name_prefix.len())] == name_prefix.to_lowercase()
                                }
                            } )}.map(|_friend| {
                                rsx!(
                                render_friend {
                                    friend: _friend.clone(),
                                    selected_friends: props.selected_friends.clone()
                                }
                            )})
                        }
                    )
                }
            )},
        }
    )
}

#[derive(PartialEq, Props, Clone)]
pub struct FriendProps {
    friend: Identity,
    selected_friends: Signal<HashSet<DID>>,
}
fn render_friend(props: FriendProps) -> Element {
    let is_checked = use_signal(|| false);
    if !*is_checked.read()
        && props
            .selected_friends
            .read()
            .contains(&props.friend.did_key())
    {
        is_checked.set(true);
    }

    let update_fn = || {
        let friend_did = props.friend.did_key();
        let new_value = !is_checked();
        is_checked.set(new_value);
        let mut friends = props.selected_friends.read();
        if new_value {
            friends.insert(friend_did);
        } else {
            friends.remove(&friend_did);
        }
        props.selected_friends.set(friends);
    };

    rsx!(
        div {
            class: "friend-container",
            aria_label: "Friend Container",
            UserImage {
                platform: props.friend.platform().into(),
                status: props.friend.identity_status().into(),
                image: props.friend.profile_picture(),
                on_press: move |_| {
                    update_fn();
                },
            },
            div {
                class: "flex-1",
                p {
                    class: "friend-name",
                    aria_label: "friend-name",
                    onclick: move |_| {
                        update_fn();
                    },
                    {props.friend.username()},
                },
            },
            Checkbox {
                disabled: false,
                width: "1em".into(),
                height: "1em".into(),
                is_checked: is_checked(),
                on_click: move |_| {
                    update_fn();
                }
            }
        }
    )
}
