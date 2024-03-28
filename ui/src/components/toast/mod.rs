use common::icons::outline::Shape as Icon;
use common::state::State;
use dioxus::prelude::*;
use kit::elements::Appearance;
use uuid::Uuid;

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    id: Uuid,
    #[props(optional)]
    icon: Option<Icon>,
    #[props(optional)]
    with_title: Option<String>,
    #[props(optional)]
    with_content: Option<String>,
    #[props(optional)]
    appearance: Option<Appearance>,
}

#[allow(non_snake_case)]
pub fn Toast(props: Props) -> Element {
    let mut state = use_context::<Signal<State>>();
    rsx!(kit::components::toast::Toast {
        id: props.id,
        on_hover: move |_| state.write_silent().reset_toast_timer(&props.id),
        on_close: move |_| state.write().remove_toast(&props.id),
        icon: props.icon,
        with_title: props.with_title.clone(),
        with_content: props.with_content.clone(),
        appearance: props.appearance
    })
}
