use dioxus::prelude::*;

// Remember: owned props must implement PartialEq!
#[derive(Props, Clone, PartialEq)]
pub struct Props {
    #[props(optional)]
    _loading: Option<bool>,
    #[props(optional)]
    disabled: Option<bool>,
    #[props(optional)]
    active: Option<bool>,
    #[props(optional)]
    onflipped: Option<EventHandler<bool>>,
}

/// Tells the parent the switch was interacted with.
pub fn emit(props: &Props, state: bool) {
    match &props.onflipped {
        Some(f) => f.call(state),
        None => {}
    }
}

/// Determines the default state
pub fn default_state(props: &Props) -> bool {
    match &props.active {
        Some(active) => *active,
        None => false,
    }
}

#[allow(non_snake_case)]
pub fn Switch(props: Props) -> Element {
    let checked_state = default_state(&props);
    let disabled = props.disabled.unwrap_or_default();

    rsx! {
        label {
            class: {
                format_args!("switch {}", if disabled { "disabled" } else { "" })
            },
            aria_label: "Switch Slider",
            input {
                aria_label: "switch-slider-value",
                disabled: "{disabled}",
                "type": "checkbox",
                checked: "{checked_state}",
                oninput: move |e| emit(&props, e.data.value() == "true")
            },
            span { class: "slider" }
        }
    }
}
