use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;

use dioxus::prelude::*;

use crate::elements::button::Button;
use crate::elements::Appearance;

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    initial_value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
    with_buttons: Option<bool>,
    onchange: EventHandler<f32>,
    no_num: Option<bool>,
    icon_left: Option<Icon>,
    icon_right: Option<Icon>,
    aria_label: Option<String>,
    disabled: Option<bool>,
}

#[allow(non_snake_case)]
pub fn Range(props: Props) -> Element {
    let mut internal_state = use_signal(|| props.initial_value);
    let value = use_signal(|| props.initial_value);

    use_effect(move || {
        internal_state.set(*value.read());
    });
    let step = props.step.unwrap_or(1_f32);
    let aria_label = props.aria_label.clone().unwrap_or_default();

    let with_buttons = props.with_buttons.unwrap_or_default();

    let on_change = props.onchange;
    let on_change1 = props.onchange;
    let on_change2 = props.onchange;

    rsx!(
        div {
            class: "range",
            aria_label: "{aria_label}",
            if with_buttons {
                Button {
                    icon: Icon::Minus,
                    appearance: Appearance::PrimaryAlternative,
                    disabled: props.disabled.unwrap_or_default(),
                    aria_label: "decrease_range_value_button".to_string(),
                    onpress: move |_| {
                        if *internal_state.read() > props.min {
                            let value: f32 = *internal_state.read() - step;
                            let rounded_value = (value * 10.0).round() / 10.0;
                            internal_state.set(rounded_value);
                            on_change.call(*internal_state.read());
                        }
                    }
                }
            } else {
                    {props.icon_left.is_some().then(||
                        rsx!{IconElement {
                            icon: props.icon_left.unwrap_or(Icon::NoSymbol),
                            size: 16,
                        }}
                )}
            }
            input {
                "type": "range",
                min: "{props.min}",
                max: "{props.max}",
                aria_label: "range-input",
                step: "{step}",
                value: "{internal_state}",
                disabled: props.disabled.unwrap_or_default(),
                oninput: move |event| {
                    internal_state.set(event.value().parse().unwrap_or_default());
                    on_change1.call(event.value().parse().unwrap_or_default());
                },
            },
            if with_buttons {
                Button {
                    icon: Icon::Plus,
                    appearance: Appearance::PrimaryAlternative,
                    aria_label: "increase_range_value_button".to_string(),
                    onpress: move |_| {
                        if *internal_state.read() < props.max {
                            let value: f32 = *internal_state.read() + step;
                            let rounded_value = (value * 10.0).round() / 10.0;
                            internal_state.set(rounded_value);
                            on_change2.call(*internal_state.read());
                        }
                    }
                }
            } else {
                {
                    props.icon_right.is_some().then(|| rsx! {
                        IconElement {
                            icon: props.icon_right.unwrap_or(Icon::NoSymbol),
                            size: 16,
                        }
                    })
                }
            }
            {(!props.no_num.unwrap_or_default()).then(||rsx!(
                p {
                    aria_label: "range-value",
                    class: "range-value",
                    "{internal_state.read()}"
                }
            ))}
        }
    )
}
