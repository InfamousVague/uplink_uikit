use dioxus::prelude::*;

use common::icons::Icon as IconElement;
use common::{icons::outline::Shape as Icon, state::State};

use crate::components::context_menu::{ContextItem, ContextMenu};

#[derive(Clone, PartialEq)]
pub struct VoiceChannelUser {
    pub talking: bool,
    pub muted: bool,
    pub deafened: bool,
    pub name: String,
    pub avatar: String,
}

#[derive(Clone, PartialEq)]
pub enum ChannelType {
    Text,
    Photo,
    Announcements,
    Robot,
    SharedFolder,
    Docs,
    Voice(Vec<VoiceChannelUser>),
}

#[derive(Clone, PartialEq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub kind: ChannelType,
}

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    channel: Channel,
    onpress: EventHandler<Channel>,
}

#[allow(non_snake_case)]
pub fn ChannelElement(props: Props) -> Element {
    let state = use_context::<Signal<State>>();

    rsx!(
        ContextMenu {
            id: format!("{}-channel", props.channel.id),
            key: "{props.channel.id}-channel",
            devmode: state.read().configuration.developer.developer_mode,
            items: rsx!(
                ContextItem {
                    icon: Icon::PencilSquare,
                    text: String::from("Rename"),
                    onpress: move |_| {}
                },
                ContextItem {
                    icon: Icon::ShieldCheck,
                    text: String::from("Permissions"),
                    onpress: move |_| {}
                },
                ContextItem {
                    danger: true,
                    icon: Icon::XMark,
                    text: String::from("Delete"),
                    onpress: move |_| {}
                },
            ),
            div {
                class: "channel",
                onclick: move |_| {
                    props.onpress.call(props.channel.clone());
                },
                {match &props.channel.kind {
                    ChannelType::Text => rsx! {IconElement {
                        icon: Icon::ChatBubbleBottomCenterText
                    }},
                    ChannelType::Photo => rsx! {IconElement {
                        icon: Icon::Photo
                    }},
                    ChannelType::SharedFolder => rsx! {IconElement {
                        icon: Icon::Folder
                    }},
                    ChannelType::Robot => rsx! {IconElement {
                        icon: Icon::CommandLine
                    }},
                    ChannelType::Announcements => rsx! {IconElement {
                        icon: Icon::InformationCircle
                    }},
                    ChannelType::Voice(_) => rsx! {IconElement {
                        icon: Icon::Speaker
                    }},
                    ChannelType::Docs => rsx! {IconElement {
                        icon: Icon::BookOpen
                    }},
                }},
                div {
                    class: "channel-info",
                    match &props.channel.kind {
                        ChannelType::Voice(_) => rsx!(
                            div {
                                class: "channel-type-voice",
                                p {
                                    class: "channel-name",
                                    {props.channel.name.clone()}
                                }
                            }
                        ),
                        _ => rsx!(
                            div {
                                class: "channel-type-text",
                                p {
                                    class: "channel-name",
                                   { props.channel.name.clone()}
                                }
                            }
                        ),
                    }
                }
            }
        }
    )
}
