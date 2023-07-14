use crate::components::embeds::youtube::YouTubePlayer;
use dioxus::prelude::*;
use dioxus::prelude::{rsx, Props};
use dioxus_core::{Element, Scope};
use dioxus_hooks::use_future;
use scraper::{Html, Selector};

use self::get_link_data::*;

mod get_link_data;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SiteMeta {
    pub title: String,
    pub description: String,
    pub icon: String,
    pub url: String,
}

pub async fn get_meta(url: &str) -> Result<SiteMeta, reqwest::Error> {
    let content = reqwest::get(url).await?.text().await?;
    let document = Html::parse_document(&content);
    let meta_selector = match Selector::parse("meta") {
        Ok(data) => data,
        Err(_) => {
            return Ok(SiteMeta {
                title: String::new(),
                description: String::new(),
                icon: String::new(),
                url: String::from(url),
            });
        }
    };

    let icon = match fetch_icon(url, document.clone()).await {
        Ok(data) => {
            if data.is_none() {
                get_image_data(document.clone(), meta_selector.clone())
                    .await
                    .unwrap_or_default()
            } else {
                data.unwrap_or_default()
            }
        }
        Err(_) => get_image_data(document.clone(), meta_selector.clone())
            .await
            .unwrap_or_default(),
    };
    let title = get_title_data(document.clone(), meta_selector.clone()).await;
    let description = get_description_data(document.clone(), meta_selector.clone()).await;

    Ok(SiteMeta {
        title,
        description,
        icon,
        url: String::from(url),
    })
}

#[derive(Props, PartialEq)]
pub struct LinkEmbedProps {
    link: String,
    remote: bool,
}

#[allow(non_snake_case)]
pub fn EmbedLinks(cx: Scope<LinkEmbedProps>) -> Element {
    let fetch_meta = use_future(cx, &cx.props.link, |link| async move {
        get_meta(link.as_str()).await
    });

    let meta = match fetch_meta.value() {
        Some(Ok(val)) => val.clone(),
        Some(Err(_)) => SiteMeta::default(),
        None => SiteMeta::default(),
    };
    let title = if meta.title.chars().count() > 100 {
        meta.title[0..97].to_string() + "..."
    } else {
        meta.title.clone()
    };

    let desc = if meta.description.chars().count() > 200 {
        meta.description[0..197].to_string() + "..."
    } else {
        meta.description.clone()
    };

    let youtube_video = if cx.props.link.contains("youtube.com/watch?v=") {
        Some(cx.props.link.replace("watch?v=", "embed/"))
    } else {
        None
    };

    cx.render(rsx! {
        if meta.title.is_empty() {
            rsx! { span {""} }
        } else {
            rsx! {
                div {
                    class: format_args!("link-embed-container {}", if cx.props.remote {"link-embed-remote"} else {""}),
                    div {
                        class: "link-embed",
                        aria_label: "link-embed",
                        div {
                            class: "embed-icon",
                            aria_label: "embed-icon",
                            if !meta.icon.is_empty() {
                                rsx!(  img {
                                    src: "{meta.icon}",
                                    alt: "Website Icon",
                                },)
                            }
                            if !title.is_empty() {
                                rsx!(a {
                                    class: "link-title",
                                    aria_label: "link-title",
                                    href: "{cx.props.link}",
                                    "{title}"
                                })
                            }
                        },
                        if desc.is_empty() && youtube_video.is_none() {
                           rsx!(div {})
                        } else {
                            rsx!( div {
                                class: "embed-details",
                                aria_label: "embed-details",
                                youtube_video.is_some().then(|| rsx!(
                                    YouTubePlayer {
                                        video_url: youtube_video.unwrap(),
                                    }
                                ))
                                p {
                                    "{desc}"
                                }
                            })
                        }
                    }
                }
            }
        }
    })
}
