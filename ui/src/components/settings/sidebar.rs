use std::str::FromStr;

use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use common::sounds;
use common::state::State;
use dioxus::prelude::*;
use kit::{
    components::nav::Nav,
    components::nav::Route as UIRoute,
    elements::input::{Input, Options},
    layout::sidebar::Sidebar as ReusableSidebar,
};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Page {
    About,
    Audio,
    Developer,
    Extensions,
    General,
    Messages,
    //Files,
    //Privacy,
    Profile,
    Notifications,
    Accessibility,
    Licenses,
}

impl Page {
    pub fn set(&mut self, p: Page) {
        *self = p;
    }
    pub fn get(&self) -> Self {
        *self
    }
    pub fn matches_str(&self, s: &str) -> bool {
        let other = match Self::from_str(s) {
            Ok(x) => x,
            Err(_) => return false,
        };
        self == &other
    }
}

impl FromStr for Page {
    fn from_str(input: &str) -> Result<Page, Self::Err> {
        match input {
            "about" => Ok(Page::About),
            "audio" => Ok(Page::Audio),
            "developer" => Ok(Page::Developer),
            "extensions" => Ok(Page::Extensions),
            //"files" => Ok(Page::Files),
            "general" => Ok(Page::General),
            "messages" => Ok(Page::Messages),
            //"privacy" => Ok(Page::Privacy),
            "profile" => Ok(Page::Profile),
            "notifications" => Ok(Page::Notifications),
            "accessibility" => Ok(Page::Accessibility),
            "licenses" => Ok(Page::Licenses),
            _ => Ok(Page::General),
        }
    }

    type Err = ();
}

#[derive(Props)]
pub struct Props<'a> {
    #[props(optional)]
    onpress: Option<EventHandler<'a, Page>>,
}

pub fn emit(cx: &Scope<Props>, e: Page) {
    match &cx.props.onpress {
        Some(f) => f.call(e),
        None => {}
    }
}

#[allow(non_snake_case)]
pub fn Sidebar<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let state = use_shared_state::<State>(cx)?;
    let page = use_shared_state::<Page>(cx)?;
    let _router = dioxus_router::hooks::use_navigator(cx);

    let profile = UIRoute {
        to: "profile",
        name: get_local_text("settings.profile"),
        icon: Icon::User,
        ..UIRoute::default()
    };

    let general = UIRoute {
        to: "general",
        name: get_local_text("settings.general"),
        icon: Icon::Cog6Tooth,
        ..UIRoute::default()
    };

    let messages = UIRoute {
        to: "messages",
        name: get_local_text("settings.messages"),
        icon: Icon::ChatBubbleBottomCenterText,
        ..UIRoute::default()
    };

    let audio = UIRoute {
        to: "audio",
        name: get_local_text("settings.audio"),
        icon: Icon::MusicalNote,
        ..UIRoute::default()
    };
    /*let privacy = UIRoute {
        to: "privacy",
        name: get_local_text("settings.privacy"),
        icon: Icon::LockClosed,
        ..UIRoute::default()
    };*/
    /*let files = UIRoute {
        to: "files",
        name: get_local_text("settings.files"),
        icon: Icon::Folder,
        ..UIRoute::default()
    };*/
    let extensions = UIRoute {
        to: "extensions",
        name: get_local_text("settings.extensions"),
        icon: Icon::Beaker,
        ..UIRoute::default()
    };
    let notifications = UIRoute {
        to: "notifications",
        name: get_local_text("settings.notifications"),
        icon: Icon::BellAlert,
        ..UIRoute::default()
    };
    let accessibility = UIRoute {
        to: "accessibility",
        name: get_local_text("settings.accessibility"),
        icon: Icon::EyeSlash,
        ..UIRoute::default()
    };
    let developer = UIRoute {
        to: "developer",
        name: get_local_text("settings.developer"),
        icon: Icon::CommandLine,
        ..UIRoute::default()
    };
    let about = UIRoute {
        to: "about",
        name: get_local_text("settings.about"),
        icon: Icon::ExclamationCircle,
        ..UIRoute::default()
    };
    let licenses = UIRoute {
        to: "licenses",
        name: get_local_text("settings.licenses"),
        icon: Icon::DocumentText,
        ..UIRoute::default()
    };

    let mut routes = vec![
        profile,
        general,
        messages,
        //privacy,
        audio,
        // files,
        extensions,
        accessibility,
        notifications,
        about,
        licenses,
    ];

    if state.read().ui.show_dev_settings {
        routes.push(developer);
    }

    // not the prettiest but matches the current code design.
    let active_page = page.read().get();
    let active_route = routes
        .iter()
        .find(|x| active_page.matches_str(x.to))
        .cloned()
        .unwrap_or(routes[0].clone());

    cx.render(rsx!(
        ReusableSidebar {
            hidden: state.read().ui.sidebar_hidden,
            with_search: cx.render(rsx!(
                div {
                    class: "search-input",
                    Input {
                        placeholder: get_local_text("settings.search-placeholder"),
                        aria_label: "settings-search-input".into(),
                        icon: Icon::MagnifyingGlass,
                        disabled: true,
                        options: Options {
                            with_clear_btn: true,
                            ..Options::default()
                        }
                    }
                }
            )),
            with_nav: cx.render(rsx!(
                crate::AppNav {
                    active: crate::UplinkRoute::SettingsLayout{},
                    onnavigate: move |_| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            common::sounds::Play(common::sounds::Sounds::Interaction);
                        }
                    }
                }
            )),
            Nav {
                routes: routes.clone(),
                active: active_route.to,
                bubble: true,
                onnavigate: move |route| {
                    if state.read().configuration.audiovideo.interface_sounds {
                       sounds::Play(sounds::Sounds::Interaction);
                    }
                    emit(&cx, Page::from_str(route).unwrap());
                }
            }
        }
    ))
}
