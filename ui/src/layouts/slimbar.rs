use crate::{utils::build_participants, UplinkRoute};
use common::icons::outline::Shape as Icon;

use common::{
    language::get_local_text,
    state::{Action, State},
};
use dioxus::prelude::*;
use dioxus_router::prelude::*;
use kit::elements::button::Button;
use kit::elements::tooltip::{ArrowPosition, Tooltip};
use kit::{
    components::{
        context_menu::{ContextItem, ContextMenu},
        user_image_group::UserImageGroup,
    },
    layout::slimbar::Slimbar,
};

#[derive(PartialEq, Props, Clone)]
pub struct Props {
    pub active: UplinkRoute,
}

#[allow(non_snake_case)]
pub fn SlimbarLayout(props: Props) -> Element {
    let mut state = use_context::<Signal<State>>();
    let router = use_navigator();

    let favorites = if state.read().initialized {
        state.read().chats_favorites()
    } else {
        vec![]
    };

    rsx!(
        Slimbar { // TODO: This should hide when the sidebar is hidden if the view is minimal (mobile).
            with_back_button: !state.read().ui.is_minimal_view() && state.read().ui.sidebar_hidden,
            onback: move |_| {
                let current = state.read().ui.sidebar_hidden;
                state.write().mutate(Action::SidebarHidden(!current));
            },
            top_children: rsx!(
                // Only display favorites if we have some.
                {(!favorites.is_empty()).then(|| rsx!(
                    div {
                        id: "favorites",
                        aria_label: "Favorites",
                        {favorites.iter().cloned().map(|chat| {
                            let users_typing = chat.typing_indicator.iter().any(|(k, _)| *k != state.read().did_key())
                                && !state.read().chats_sidebar().contains(&chat);
                            let favorites_chat = chat.clone();
                            let remove_favorite = chat.clone();
                            let chat_id = chat.id;
                            let participants = state.peek().chat_participants(&chat);
                            let other_participants: Vec<_> = state.read().remove_self(&participants);
                            let participants_name = match chat.conversation_name {
                                Some(name) => name,
                                None => State::join_usernames(&other_participants)
                            };
                            rsx! (
                                ContextMenu {
                                    key: "{chat_id}-favorite",
                                    id: chat_id.to_string(),
                                    items: rsx!(
                                        ContextItem {
                                            aria_label: "favorites-chat".to_string(),
                                            icon: Icon::ChatBubbleBottomCenterText,
                                            text: get_local_text("uplink.chat"),
                                            onpress: move |_| {
                                                if state.read().ui.is_minimal_view() {
                                                    state.write().mutate(Action::SidebarHidden(true));
                                                }
                                                state.write().mutate(Action::ChatWith(&favorites_chat.id, false));
                                                router.replace(UplinkRoute::ChatLayout{});
                                            }
                                        },
                                        ContextItem {
                                            aria_label: "favorites-remove".to_string(),
                                            icon: Icon::HeartSlash,
                                            text: get_local_text("favorites.remove"),
                                            onpress: move |_| {
                                                state.write().mutate(Action::ToggleFavorite(&remove_favorite.id));
                                            }
                                        }
                                    ),
                                    UserImageGroup {
                                        participants: build_participants(&other_participants),
                                        aria_label: participants_name.clone(),
                                        with_username: participants_name,
                                        use_tooltip: true,
                                        typing: users_typing,
                                        onpress: move |_| {
                                            if state.read().ui.is_minimal_view() {
                                                state.write().mutate(Action::SidebarHidden(true));
                                            }
                                            state.write().mutate(Action::ChatWith(&chat.id, false));
                                            router.replace(UplinkRoute::ChatLayout{});
                                        }
                                    }
                                }
                            )
                        })}
                    }
                ))},
            ),
            navbar_visible: state.read().ui.sidebar_hidden,
            with_nav: rsx!(
                crate::AppNav {
                    active: props.active.clone(),
                    tooltip_direction: ArrowPosition::Left,
                    onnavigate: move |_| {
                        if state.peek().configuration.peek().audiovideo.interface_sounds {
                            common::sounds::Play(common::sounds::Sounds::Interaction);
                        }
                        if state.read().ui.is_minimal_view() {
                            state.write().mutate(Action::SidebarHidden(true));
                        }
                    },
                },
            ),
            {state.peek().configuration.read().developer.experimental_features.then(|| rsx!(
                Button {
                    icon: Icon::Plus,
                    tooltip: rsx!(
                        Tooltip {
                            arrow_position: ArrowPosition::Left,
                            text: "Create Community".to_string()
                        }
                    ),
                    onpress: move |_| {
                        router.replace(UplinkRoute::CommunityLayout {});
                    }
                }
            ))}
        }
    )
}
