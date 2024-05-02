use common::icons::outline::Shape as Icon;
use common::state::Action;
use common::state::State;
use dioxus::prelude::*;
use kit::elements::input::Input;
use kit::elements::input::Options;
use kit::elements::tooltip::ArrowPosition;
use kit::layout::sidebar::Sidebar as ReusableSidebar;

use crate::components::community::sidebar::SidebarInner;
use crate::UplinkRoute;

#[derive(PartialEq, Props, Clone)]
pub struct Props {
    pub active: UplinkRoute,
}

#[allow(non_snake_case)]
pub fn Sidebar(props: Props) -> Element {
    let mut state = use_context::<Signal<State>>();

    rsx!(ReusableSidebar {
        hidden: state.read().ui.sidebar_hidden,
        with_search: rsx!(
            div {
                class: "search-input",
                Input {
                    placeholder: "The Uplink Community".to_string(),
                    aria_label: "settings-search-input".to_string(),
                    icon: Icon::MagnifyingGlass,
                    disabled: true,
                    options: Options {
                        with_clear_btn: true,
                        ..Options::default()
                    }
                }
            }
        ),
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
        SidebarInner {}
    })
}
