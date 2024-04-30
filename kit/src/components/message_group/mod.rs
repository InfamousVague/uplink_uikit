use dioxus::prelude::*;

use crate::components::{
    indicator::{Platform, Status},
    user_image::UserImage,
};

#[derive(Props, Clone)]
pub struct Props {
    children: Element,
    user_image: Element,
    sender: String,
    #[props(optional)]
    remote: Option<bool>,
    #[props(optional)]
    timestamp: Option<String>,
}

impl PartialEq for Props {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

#[allow(non_snake_case)]
pub fn MessageGroup(props: Props) -> Element {
    println!("Rendering Message GROUP");
    let remote = props.remote.unwrap_or_default();
    let time_ago = props.timestamp.clone().unwrap_or_default();

    rsx! (
        div {
            class: "message-group-wrap",
            aria_label: {
                format_args!("message-group-wrap-{}", if remote { "remote" } else { "local" })
            },
            {remote.then(|| rsx!(
                {&props.user_image}
            ))}
            div {
                class: {
                    format_args!("message-group {}", if remote { "remote" } else { "" })
                },
                aria_label: {
                    format_args!("message-group{}", if remote { "-remote" } else { "" })
                },
                {&props.children},
                p {
                    class: "time-ago noselect defaultcursor",
                    aria_label: "time-ago",
                    "{props.sender} - {time_ago}"
                }
            }
            {(!remote).then(|| rsx!(
                {&props.user_image}
            ))}
        }
    )
}

#[derive(PartialEq, Props, Clone)]
pub struct SkeletalProps {
    #[props(optional)]
    alt: Option<bool>,
}

#[allow(non_snake_case)]
pub fn MessageGroupSkeletal(props: SkeletalProps) -> Element {
    let alt = props.alt.unwrap_or_default();

    rsx!(
        div {
            class: format_args!("message-group-skeletal {}", if alt { "alt" } else { "" }),
            UserImage {
                loading: true,
                status: Status::Offline,
                platform: Platform::Desktop
            },
            div {
                class: "skeletal-messages",
                div {
                    class: "skeletal-message",
                    div {
                        class: "skeletal-message-content skeletal",
                    }
                },
                div {
                    class: "skeletal-message",
                    div {
                        class: "skeletal-message-content skeletal",
                    }
                },
                div {
                    class: "skeletal-message",
                    div {
                        class: "skeletal-message-content skeletal",
                    }
                },
                div {
                    class: "skeletal-timestamp"
                }
            }
        }
    )
}
