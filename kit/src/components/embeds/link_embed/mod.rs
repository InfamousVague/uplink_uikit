use crate::components::embeds::youtube::YouTubePlayer;
use dioxus::prelude::*;
use dioxus::prelude::{rsx, Props};
use dioxus_core::Element;
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

pub async fn get_meta(url: String) -> Result<SiteMeta, reqwest::Error> {
    let content = reqwest::get(url.clone()).await?.text().await?;
    let document = Html::parse_document(&content);
    let meta_selector = match Selector::parse("meta") {
        Ok(data) => data,
        Err(_) => {
            return Ok(SiteMeta {
                title: String::new(),
                description: String::new(),
                icon: String::new(),
                url: url.clone(),
            });
        }
    };

    let icon = if let Ok(Some(icon)) = fetch_icon(&url, document.clone()).await {
        icon
    } else {
        get_image_data(document.clone(), meta_selector.clone()).unwrap_or_default()
    };
    let title = get_title_data(document.clone(), meta_selector.clone());
    let description = get_description_data(document.clone(), meta_selector.clone());

    Ok(SiteMeta {
        title,
        description,
        icon,
        url,
    })
}

#[derive(Props, PartialEq, Clone)]
pub struct LinkEmbedProps {
    link: String,
    remote: bool,
}

#[allow(non_snake_case)]
pub fn EmbedLinks(props: LinkEmbedProps) -> Element {
    // TODO(Migration_0.5): Before it was a use_future, verify if it keep same behavior
    let props_link = use_signal(|| props.link.clone());
    let fetch_meta = use_resource(move || async move { get_meta(props_link()).await });

    let fetch_meta_result = fetch_meta.read();
    let fetch_meta_result = fetch_meta_result.as_ref().map(|val| val.as_ref());

    let meta = match fetch_meta_result {
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

    let youtube_video = if props_link.read().contains("youtube.com/watch?v=") {
        Some(props.link.replace("watch?v=", "embed/"))
    } else {
        None
    };

    rsx! {
        if meta.title.is_empty() {
            span {""}
        } else {
                div {
                    class: format_args!("link-embed-container {}", if props.remote {"link-embed-remote"} else {""}),
                    div {
                        class: "link-embed",
                        aria_label: "link-embed",
                        div {
                            class: "embed-icon",
                            aria_label: "embed-icon",
                            if !meta.icon.is_empty() {
                                 img {
                                    src: "{meta.icon}",
                                    alt: "Website Icon",
                                }
                            }
                            if !title.is_empty() {
                                a {
                                    class: "link-title",
                                    aria_label: "link-title",
                                    href: "{props.link}",
                                    "{title}"
                                }
                            }
                        },
                        if desc.is_empty() && youtube_video.is_none() {
                           div {}
                        } else {
                            div {
                                class: "embed-details",
                                aria_label: "embed-details",
                                {youtube_video.is_some().then(|| rsx!(
                                    YouTubePlayer {
                                        video_url: youtube_video.unwrap(),
                                    }
                                ))}
                                p {
                                    "{desc}"
                                }
                            }
                        }
                    }
                }
        }
    }
}
