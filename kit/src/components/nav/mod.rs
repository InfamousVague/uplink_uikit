use dioxus::prelude::*;
use uuid::Uuid;

use crate::{
    components::context_menu::ContextMenu,
    elements::{
        button::Button,
        tooltip::{ArrowPosition, Tooltip},
        Appearance,
    },
};
use common::icons::outline::Shape as Icon;
pub type To = &'static str;

#[derive(Clone)]
pub struct Route {
    pub to: To,
    pub icon: Icon,
    pub name: String,
    pub with_badge: Option<String>,
    pub progress_bar: Option<i8>,
    pub loading: Option<bool>,
    pub child: Option<Element>,
    pub context_items: Option<Element>,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            to: "",
            icon: Icon::QuestionMarkCircle,
            name: "Default".to_owned(),
            with_badge: None,
            progress_bar: None,
            loading: None,
            child: None,
            context_items: None,
        }
    }
}

#[derive(Props, Clone)]
pub struct Props {
    #[props(optional)]
    onnavigate: Option<EventHandler<To>>,
    routes: Vec<Route>,
    #[props(optional)]
    active: Option<To>,
    #[props(optional)]
    bubble: Option<bool>,
    pub tooltip_direction: Option<ArrowPosition>,
}

impl PartialEq for Props {
    // TODO(LucasMarchi): Review it later
    fn eq(&self, other: &Self) -> bool {
        self.routes.len() == other.routes.len() && self.active == other.active
    }
}

/// Tells the parent the nav was interacted with.
pub fn emit(props: Props, to: &To) {
    match &props.onnavigate {
        Some(f) => f.call(to.to_owned()),
        None => {}
    }
}

/// Gets the appearance for a nav button based on the active route
pub fn get_appearance(active_route: To, route: To) -> Appearance {
    if active_route == route {
        Appearance::Primary
    } else {
        Appearance::Transparent
    }
}

/// Generates the an optional badge value
pub fn get_badge(route: &Route) -> String {
    route.with_badge.clone().unwrap_or_default()
}

/// Gets the active route, or returns a void one
pub fn get_active(props: Props) -> To {
    props.active.unwrap_or("!void")
}

/// Returns a nav component generated based on given props.
///
/// # Examples
/// //```no_run
/// use dioxus::prelude::*;
/// use kit::{elements::{Icon, IconElement}, components::nav::{Nav, Route}};
///
/// let home = Route { to: "/fake/home", name: "Home", icon: Icon::HomeModern };
/// let routes = vec![
///     home,
///     Route { to: "/fake/chat", name: "Chat", icon: Icon::ChatBubbleBottomCenter },
///     Route { to: "/fake/friends", name: "Friends", icon: Icon::Users },
///     Route { to: "/fake/settings", name: "Settings", icon: Icon::Cog6Tooth },
/// ];
/// let active = routes[0].clone();
///
/// rsx! (
///     Nav {
///        routes: routes,
///        active: active
///    }
/// )
/// ```
#[allow(non_snake_case)]
pub fn Nav(props: Props) -> Element {
    let mut active = use_signal(|| get_active(props.clone()));
    let bubble = props.bubble.unwrap_or_default();
    let tooltip_direction = props.tooltip_direction.unwrap_or(ArrowPosition::Bottom);
    let routes_in_app = props.routes.clone();
    // For some reason if you dont do this the first render will not have a context menu
    let uuid = use_signal(|| Uuid::new_v4().to_string());
    let props_signal = use_signal(|| props.clone());

    rsx!(
        div {
            aria_label: "button-nav",
            class: {
                format_args!("nav disable-select {}", if bubble { "bubble" } else { "" })
            },
            {routes_in_app.iter().cloned().map(|route| {
                let badge = get_badge(&route);
                let key: String = route.name.clone();
                let name: String = route.name.clone();
                let name2: String = name.to_lowercase();
                let aria_label: String = route.name.clone();
                // todo: don't show the tooltip if bubble is true
                let tooltip = if props.bubble.is_some() {
                    rsx!("")
                } else {
                    rsx!(Tooltip {
                        arrow_position: tooltip_direction,
                        text: route.name.clone(),
                    })
                };

                let btn = rsx!(
                    div {
                        position: "relative",
                        display: "inline-grid",
                        key: "{key}",
                        Button {
                            aria_label: aria_label.to_lowercase() + "-button",
                            icon: route.icon,
                            onpress: move |_| {
                                active.set(route.to);
                                emit(props_signal.read().clone(), &route.to)
                            },
                            text: {
                                if bubble { name } else { "".into() }
                            },
                            with_badge: badge,
                            tooltip: tooltip,
                            appearance: get_appearance(active.read().clone(), route.to),
                            with_progress: route.progress_bar.unwrap_or(-1)
                        },
                        {route.child.as_ref()}
                    }
                );
                match route.context_items.as_ref() {
                    None => btn,
                    Some(items) => {
                        rsx!(ContextMenu{
                            id: format!("route-{}-{}", name2, uuid.read()),
                            key: "{name2}-{uuid.read()}",
                            items: items.clone(),
                            {btn}
                        })
                    }
                }
            })}
        }
    )
}
