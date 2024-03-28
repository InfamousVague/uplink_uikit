use chrono::{DateTime, Utc};

use dioxus::{
    events::{MouseData, MouseEvent},
    prelude::*,
};

use crate::components::{
    indicator::{Platform, Status},
    user_image::UserImage,
};

pub mod card;

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    username: String,
    user_image: Element,
    subtext: String,
    timestamp: Option<DateTime<Utc>>,
    aria_label: Option<String>,
    #[props(optional)]
    loading: Option<bool>,
    #[props(optional)]
    with_badge: Option<String>,
    #[props(optional)]
    active: Option<bool>,
    #[props(optional)]
    onpress: Option<EventHandler<MouseEvent>>,
}

pub fn get_time_ago(props: Props) -> String {
    let f = timeago::Formatter::new();
    let current_time = Utc::now();
    let c: chrono::Duration = current_time - props.timestamp.unwrap_or(current_time);
    let duration: std::time::Duration = match c.to_std() {
        // for the sidebar, don't want the timestamp to increment a few seconds every time the typing indicator comes over.
        // prevent this by rounding down and giving a duration in minutes only.
        Ok(d) => std::time::Duration::from_secs(d.as_secs() / 60 * 60),
        Err(_e) => std::time::Duration::ZERO,
    };

    f.convert(duration)
}

/// Generates the optional badge for the user.
/// If there is no badge provided, we'll return an empty string.
pub fn get_badge(props: Props) -> String {
    props.with_badge.clone().unwrap_or_default()
}

/// Tells the parent the user was interacted with.
pub fn emit(props: Props, e: Event<MouseData>) {
    if let Some(f) = props.onpress.as_ref() {
        f.call(e)
    }
}

#[allow(non_snake_case)]
pub fn User(props: Props) -> Element {
    let props_signal = use_signal(|| props.clone());
    let time_ago = get_time_ago(props.clone());
    let badge = get_badge(props.clone());
    let aria_label = props.aria_label.clone().unwrap_or_default();
    let active = props.active.unwrap_or_default();
    let loading = props.loading.unwrap_or_default();

    rsx!(if loading {
        UserLoading {}
    } else {
            div {
                class: {
                    format_args!("user {} noselect defaultcursor", if active { "active" } else { "" })
                },
                onclick: move |e| emit(props_signal(), e),
                aria_label: "{aria_label}",
                {(!badge.is_empty()).then(|| rsx!(
                    span {
                        class: "badge",
                        aria_label: "User Badge",
                        span {
                            class: "badge-prefix",
                            aria_label: "badge-prefix",
                            "{time_ago}"
                        }
                        span {
                            class: "badge-count",
                            aria_label: "badge-count",
                            "{badge}"
                        }
                    }
                ))},
                {&props.user_image},
                div {
                    class: "info",
                    aria_label: "User Info",
                    p {
                        class: "username",
                        aria_label: "Username",
                        "{props.username}"
                    },
                    p {
                        class: "subtext",
                        aria_label: "User Status",
                        dangerous_inner_html: "{props.subtext}"
                    }
                }
            }
    })
}

#[allow(non_snake_case)]
fn UserLoading() -> Element {
    rsx!(
        div {
            class: "skeletal-user",
            aria_label: "skeletal-user",
            UserImage {
                loading: true,
                status: Status::Offline,
                platform: Platform::Desktop,
            },
            div {
                class: "skeletal-bars",
                div {
                    class: "skeletal-inline",
                    div {
                        class: "skeletal skeletal-bar seventy-five",
                    },
                    div {
                        class: "skeletal skeletal-bar flex",
                    }
                },
                div {
                    class: "skeletal skeletal-bar thick",
                }
            }
        }
    )
}
