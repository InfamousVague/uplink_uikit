use common::icons::outline::Shape as Icon;
use derive_more::Display;
use dioxus::prelude::*;
use humansize::{format_size, DECIMAL};
use warp::constellation::file::File;

use crate::elements::button;

#[derive(Eq, PartialEq, Clone, Copy, Display)]
pub enum Order {
    #[display(fmt = "message-first")]
    First,

    #[display(fmt = "message-middle")]
    Middle,

    #[display(fmt = "message-last")]
    Last,
}

#[derive(Props)]
pub struct Props<'a> {
    // An optional field that, if set to true, will add a CSS class of "loading" to the div element.
    #[props(optional)]
    loading: Option<bool>,

    // An optional field that, if set, will be used as the content of a nested div element with a class of "content".
    #[props(optional)]
    with_content: Option<Element<'a>>,

    // An optional field that, if set, will be used as the text content of a nested p element with a class of "text".
    #[props(optional)]
    with_text: Option<String>,

    // todo: remove unused attribute
    // todo: does this need to be an option like the rest of 'em?
    #[allow(unused)]
    reactions: Vec<warp::raygun::Reaction>,

    // An optional field that, if set to true, will add a CSS class of "remote" to the div element.
    #[props(optional)]
    remote: Option<bool>,

    // An optional field that, if set, will be used to determine the ordering of the div element relative to other Message elements.
    // The value will be converted to a string using the Order enum's fmt::Display implementation and used as a CSS class of the div element.
    // If not set, the default value of Order::Last will be used.
    #[props(optional)]
    order: Option<Order>,

    // todo: remove this
    #[allow(unused)]
    #[props(optional)]
    attachments: Option<Vec<File>>,
}

#[allow(non_snake_case)]
pub fn Message<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let text = cx.props.with_text.clone().unwrap_or_default();
    // todo: render reactions
    // todo: render part of message being replied to

    let loading = cx.props.loading.unwrap_or_default();
    let remote = cx.props.remote.unwrap_or_default();
    let order = cx.props.order.unwrap_or(Order::Last);

    let attachment_list = cx.props.attachments.iter().map(|vec| {
        vec.iter().map(|file| {
            let key = file.id();
            rsx!(Attachment {
                key: "{key}",
                file: file.clone(),
            })
        })
    });

    cx.render(rsx! (
        div {
            class: {
                format_args!(
                    "message {} {} {}",
                    if loading {
                        "loading"
                    } else { "" },
                    if remote {
                        "remote"
                    } else { "" },
                    if cx.props.order.is_some() {
                        order.to_string()
                    } else { "".into() }
                )
            },
            aria_label: "Message",
            (cx.props.with_content.is_some()).then(|| rsx! (
                    div {
                    class: "content",
                    cx.props.with_content.as_ref(),
                },
            )),
            (cx.props.with_text.is_some()).then(|| rsx! (
                p {
                    class: "text",
                    "{text}"
                }
            ))
            attachment_list.map(|list| {
                rsx!(div { list })
            })
        }
    ))
}

#[derive(PartialEq, Eq, Props)]
pub struct AttachmentProps {
    file: File,
}

#[allow(non_snake_case)]
pub fn Attachment(cx: Scope<AttachmentProps>) -> Element {
    let size = format_size(cx.props.file.size(), DECIMAL);
    let name = cx.props.file.name();
    cx.render(rsx! {
        div {
            class: "attachment-embed",
            div {
                class: "embed-icon",
                common::icons::Icon {
                    icon: Icon::Document,
                },
                h2 {
                    "{name}"
                }
            }
            div {
                class: "embed-details",
                p {
                    "{size}"
                },
                button::Button {
                    icon: Icon::DocumentArrowDown,
                    text: String::new(),
                }
            }
        }
    })
}
