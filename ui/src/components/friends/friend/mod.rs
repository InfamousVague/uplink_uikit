use dioxus::prelude::*;

use kit::{
    components::{
        indicator::{Platform, Status},
        user_image::UserImage,
    },
    elements::{
        button::Button,
        label::Label,
        tooltip::{ArrowPosition, Tooltip},
        Appearance,
    },
};

use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use common::state::State;
use warp::multipass::identity::Relationship;

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    // The username of the friend request sender
    username: String,
    // A suffix to the username, typically a unique identifier
    suffix: String,
    // Users relationship
    relationship: Relationship,
    // Status message from friend
    status_message: String,
    // The user image element to display
    user_image: Element,
    // An optional event handler for the "onchat" event
    onchat: Option<EventHandler>,
    // An optional event handler for the "onremove" event
    onremove: Option<EventHandler>,
    onaccept: Option<EventHandler>,
    // An optional event handler for the "onblock" event
    onblock: Option<EventHandler>,
    accept_button_disabled: Option<bool>,
    block_button_disabled: Option<bool>,
    remove_button_disabled: Option<bool>,
    aria_label: Option<String>,
}

#[allow(non_snake_case)]
pub fn Friend(props: Props) -> Element {
    let state = use_context::<Signal<State>>();
    let relationship = props.relationship;
    let status_message = props.status_message.clone();
    let aria_label = props.aria_label.clone().unwrap_or_default();

    let any_button_disabled = props.accept_button_disabled.unwrap_or(false)
        || props.block_button_disabled.unwrap_or(false)
        || props.remove_button_disabled.unwrap_or(false);

    rsx!(
        div {
            class: "friend",
            aria_label: "{aria_label}",
            {&props.user_image},
            div {
                class: "request-info",
                aria_label: "Friend Info",
                p {
                    aria_label: "friend-username",
                    "{props.username}",
                    {(!state.read().ui.is_minimal_view()).then(|| rsx!(
                        span {
                            "#{props.suffix}"
                        }
                    ))},
                },
                if relationship.friends() {
                   {rsx!(p {
                        class: "status-message",
                        aria_label: "status-message",
                        {(!state.read().ui.is_minimal_view()).then(|| rsx!( "{status_message}" ))}
                    })}
                } else  {
                    {rsx!(Label {
                        // TODO: this is stubbed for now, wire up to the actual request time
                        aria_label: "friendship-status".to_string(),
                        text: get_local_text(
                            if relationship.blocked() {
                                "friends.blocked-desc"
                            } else if relationship.sent_friend_request() {
                                "friends.sent"
                            } else {
                                "friends.requested"
                            })
                    })}
                }
            },
            div {
                class: "request-controls",
                {props.onaccept.is_some().then(|| rsx!(
                    Button {
                        icon: Icon::Check,
                        text: get_local_text("friends.accept"),
                        aria_label: "Accept Friend".to_string(),
                        loading:  props.accept_button_disabled.unwrap_or(false),
                        disabled:any_button_disabled,
                        onpress: move |_| match &props.onaccept {
                            Some(f) => f.call(()),
                            None    => {},
                        }
                    }
                ))},
                {props.onchat.is_some().then(|| rsx! (
                    Button {
                        icon: Icon::ChatBubbleBottomCenterText,
                        aria_label: "Chat With Friend".to_string(),
                        disabled: any_button_disabled,
                        text: if state.read().ui.is_minimal_view() { "".into() } else { get_local_text("uplink.chat") },
                        onpress: move |_| match &props.onchat {
                            Some(f) => f.call(()),
                            None    => {},
                        }
                    }
                ))},
                Button {
                    icon: Icon::UserMinus,
                    appearance: Appearance::Secondary,
                    loading:  props.remove_button_disabled.unwrap_or(false),
                    disabled: any_button_disabled,
                    onpress: move |_| {
                        // note that the blocked list uses the onremove callback to unblock the user.yes, it's kind of a hack
                        match &props.onremove {
                            Some(f) => f.call(()),
                            None => {},
                        }
                    },
                    aria_label: "Remove or Deny Friend".to_string(),
                    tooltip: rsx!(Tooltip {
                        arrow_position: ArrowPosition::Right,
                        text: if props.relationship.blocked() { get_local_text("friends.unblock") } else if props.onaccept.is_none() { get_local_text("friends.remove") } else { get_local_text("friends.deny") }
                    }),
                },
                {(props.onchat.is_some() && !state.read().ui.is_minimal_view()).then(|| rsx!(
                    Button {
                        icon: Icon::NoSymbol,
                        appearance: Appearance::Secondary,
                        loading:  props.block_button_disabled.unwrap_or(false),
                        disabled: any_button_disabled,
                        onpress: move |_| match &props.onblock {
                            Some(f) => f.call(()),
                            None    => {},
                        },
                        aria_label: "Block Friend".to_string(),
                        tooltip: rsx!(Tooltip {
                            arrow_position: ArrowPosition::Right,
                            text: get_local_text("friends.block"),
                        })
                    }
                ))}
            }
        }
    )
}

// todo: remove this
#[allow(unused)]
#[allow(non_snake_case)]
pub fn SkeletalFriend() -> Element {
    rsx!(
        div {
            class: "skeletal-friend",
            UserImage {
                loading: true,
                platform: Platform::Desktop,
                status: Status::Offline,
            },
            div {
                class: "skeletal-bars",
                div {
                    class: "skeletal-bar"
                },
                div {
                    class: "skeletal-bar"
                }
            },
            // TODO: include the disabled controls, should show only controls relevant to parent props.
        }
    )
}
