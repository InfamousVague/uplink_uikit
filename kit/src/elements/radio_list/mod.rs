use crate::elements::button::Button;
use crate::elements::Appearance;
use common::icons::outline::Shape as Icon;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    initial_value: String,
    values: Vec<String>,
    onchange: EventHandler<String>,
}

#[allow(non_snake_case)]
pub fn RadioList(props: Props) -> Element {
    let mut internal_state = use_signal(|| props.initial_value.clone());
    let onchange_clone = props.onchange;
    rsx!(
        div {
            class: "radio-list",
            for option in props.values {
                Button {
                    icon: if *internal_state.read() == *option { Icon::RadioSelected } else { Icon::Radio },
                    appearance: if *internal_state.read() == *option { Appearance::Primary } else { Appearance::Secondary },
                    text: option.clone(),
                    aria_label: format!("radio-option-{}", option),
                    onpress: move |_| {
                        internal_state.set(option.clone());
                        onchange_clone.clone().call(option.clone());
                    },
                }
            }
        }
    )
}
