use dioxus::prelude::*;

// Remember: owned props must implement PartialEq!
#[derive(Props, Clone, PartialEq)]
pub struct Props {
    #[props(optional)]
    _loading: Option<bool>,
    _min: i32,
    _max: i32,
    #[props(optional)]
    default_value: Option<i32>,
    #[props(optional)]
    _onflipped: Option<EventHandler<i32>>,
}

pub fn get_default(props: Props) -> i32 {
    props.default_value.unwrap_or_default()
}

#[allow(non_snake_case)]
pub fn Slider(props: Props) -> Element {
    let mut _slider_value = use_signal(|| get_default(props));
    // TODO: Pending dioxus update for eval returning values
    rsx! {
        div {
            class: "slider",
        }
    }
}
