use std::time::Duration;

use crate::layouts::chats::ChatSidebar;
use crate::{
    components::friends::{
        add::AddFriend, blocked::BlockedUsers, friends_list::Friends,
        incoming_requests::PendingFriends, nothing_here::NothingHere,
        outgoing_requests::OutgoingRequests,
    },
    layouts::slimbar::SlimbarLayout,
};
use common::icons::outline::Shape as Icon;
use common::state::{ui, Action, State};
use common::{
    language::get_local_text,
    notifications::{NotificationAction, NOTIFICATION_LISTENER},
};
use dioxus::prelude::*;
use kit::{
    elements::{button::Button, Appearance},
    layout::topbar::Topbar,
};
use tokio::sync::broadcast::error::RecvError;
use tracing::log;

#[derive(PartialEq, Clone)]
pub enum FriendRoute {
    All,
    Pending,
    Blocked,
}

#[allow(non_snake_case)]
pub fn FriendsLayout() -> Element {
    let mut state = use_context::<Signal<State>>();
    let mut route = use_signal(|| FriendRoute::All);
    let show_slimbar = state.read().show_slimbar() & !state.read().ui.is_minimal_view();
    state.write_silent().ui.current_layout = ui::Layout::Friends;

    if state.read().ui.is_minimal_view() {
        return rsx!(MinimalFriendsLayout { route: route });
    }
    log::trace!("rendering FriendsLayout");

    // this is a hack to deal with a change in how Dioxus routing works. The `route` hook used to be shared
    // between elements.
    use_resource(move || {
        to_owned![route];
        async move {
            let mut ch = NOTIFICATION_LISTENER.tx.subscribe();
            log::trace!("starting notification action listener");
            loop {
                let cmd = match ch.recv().await {
                    Ok(cmd) => cmd,
                    Err(RecvError::Closed) => {
                        log::debug!("RecvError::Closed while reading from NOTIFICATION_LISTENER");
                        return;
                    }
                    _ => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                log::debug!("handling notification action {:#?}", cmd);
                match cmd {
                    NotificationAction::FriendListPending => {
                        route.set(FriendRoute::Pending);
                    }
                    NotificationAction::Dummy => {}
                    _ => {}
                }
            }
        }
    });

    rsx!(
        div {
            id: "friends-layout",
            aria_label: "friends-layout",
            class: "disable-select",
            if show_slimbar {
                {rsx!(
                    SlimbarLayout { active: crate::UplinkRoute::FriendsLayout {} },
                )}
            },
            ChatSidebar {
                active_route: crate::UplinkRoute::FriendsLayout {},
            },
            div {
                class: "friends-body",
                aria_label: "friends-body",
                {get_topbar(route)},
                AddFriend {},
                // TODO: Will need to determine if we're loading or not once state is update, and display a loading view if so. (see friends-list)
                {render_route((), route())},
            }
        }
    )
}

#[derive(PartialEq, Props, Clone)]
pub struct MinimalProps {
    route: Signal<FriendRoute>,
}

#[allow(non_snake_case)]
pub fn MinimalFriendsLayout(props: MinimalProps) -> Element {
    log::trace!("rendering MinimalFriendsLayout");
    let mut state = use_context::<Signal<State>>();
    let route = props.route;

    let view = if !state.read().ui.sidebar_hidden {
        rsx!(ChatSidebar {
            active_route: crate::UplinkRoute::FriendsLayout {},
        })
    } else {
        rsx!(
            div {
                class: "friends-body",
                aria_label: "friends-body",
                {get_topbar(route)},
                AddFriend {},
                div {
                    class: "friends-controls",
                    aria_label: "friends-controls",
                },
                // TODO: Will need to determine if we're loading or not once state is update, and display a loading view if so. (see friends-list)
                {render_route(props, route.read().clone())},
                crate::AppNav {
                    active: crate::UplinkRoute::FriendsLayout{},
                }
            }
        )
    };

    rsx!(div {
        id: "friends-layout",
        aria_label: "friends-layout",
        {view}
    })
}

fn render_route<T>(props: T, route: FriendRoute) -> Element {
    rsx!(match route {
        FriendRoute::All => rsx!(Friends {}),
        FriendRoute::Pending => rsx!(
            PendingFriends {},
            OutgoingRequests {},
            NothingHere {
                friends_tab: "Pending".to_string()
            }
        ),
        FriendRoute::Blocked => rsx!(
            BlockedUsers {},
            NothingHere {
                friends_tab: "Blocked".to_string()
            }
        ),
    })
}

fn get_topbar(mut route: Signal<FriendRoute>) -> Element {
    let mut state = use_context::<Signal<State>>();
    let pending_friends = state.read().friends().incoming_requests.len();

    rsx!(Topbar {
        with_back_button: state.read().ui.is_minimal_view() && state.read().ui.sidebar_hidden,
        onback: move |_| {
            let current = state.read().ui.sidebar_hidden;
            state.write().mutate(Action::SidebarHidden(!current));
        },
        controls: rsx!(
            Button {
                icon: Icon::Users,
                text: if state.read().ui.is_minimal_view() {
                    "".into()
                } else {
                    get_local_text("friends.all")
                },
                aria_label: "all-friends-button".to_string(),
                appearance: if route() == FriendRoute::All {
                    Appearance::Primary
                } else {
                    Appearance::Secondary
                },
                onpress: move |_| {
                    route.set(FriendRoute::All);
                }
            },
            Button {
                icon: Icon::Clock,
                appearance: if route() == FriendRoute::Pending {
                    Appearance::Primary
                } else {
                    Appearance::Secondary
                },
                text: if state.read().ui.is_minimal_view() {
                    "".into()
                } else {
                    get_local_text("friends.pending")
                },
                aria_label: "pending-friends-button".to_string(),
                with_badge: if pending_friends > 0 {
                    pending_friends.to_string()
                } else {
                    "".into()
                },
                onpress: move |_| {
                    route.set(FriendRoute::Pending);
                }
            },
            Button {
                icon: Icon::NoSymbol,
                appearance: if route() == FriendRoute::Blocked {
                    Appearance::Primary
                } else {
                    Appearance::Secondary
                },
                text: if state.read().ui.is_minimal_view() {
                    "".into()
                } else {
                    get_local_text("friends.blocked")
                },
                aria_label: "blocked-friends-button".to_string(),
                onpress: move |_| {
                    route.set(FriendRoute::Blocked);
                }
            },
        )
    },)
}
