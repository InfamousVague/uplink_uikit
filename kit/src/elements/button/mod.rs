use dioxus::{events::MouseEvent, prelude::*};

use crate::elements::Appearance;

use common::icons::outline::Shape as Icon;

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    loading: Option<bool>,
    onpress: Option<EventHandler<MouseEvent>>,
    text: Option<String>,
    tooltip: Option<Element>,
    aria_label: Option<String>,
    icon: Option<Icon>,
    disabled: Option<bool>,
    appearance: Option<Appearance>,
    with_badge: Option<String>,
    small: Option<bool>,
    with_title: Option<bool>,
    with_progress: Option<i8>,
}

/// Generates the appearance for the button.
/// This will be overwritten if the button is disabled.
pub fn get_appearance(props: Props) -> Appearance {
    // If the button is disabled, we can short circuit this and just provide the disabled appearance.
    if let Some(is_disabled) = props.disabled {
        if is_disabled {
            return Appearance::Disabled;
        }
    }
    props.appearance.unwrap_or(Appearance::Default)
}

/// Returns a button element generated based on given props.
///
/// # Examples
/// ```no_run
/// use kit::{Icon, tooltip::{Tooltip, ArrowPosition}, components::nav::{Nav, Route}};
///
/// Button {
///     appearance: Appearance::Primary,
///     icon: Icon::Cog6Tooth,
///     tooltip: rsx!(
///         Tooltip {
///             arrow_position: ArrowPosition::Bottom,
///             text: String::from("Settings")
///         }
///     )),
/// },
/// ```
#[allow(non_snake_case)]
pub fn Button(props: Props) -> Element {
    let text = props.text.clone().unwrap_or_default();
    let text2 = text.clone();
    let aria_label = props.aria_label.clone().unwrap_or_default();
    let badge = props.with_badge.clone().unwrap_or_default();
    let disabled = props.disabled.unwrap_or_default();
    let appearance = get_appearance(props.clone());
    let small = props.small.unwrap_or_default();
    let title = if props.with_title.unwrap_or(true) {
        text.clone()
    } else {
        String::new()
    };

    let mut tooltip_visible = use_signal(|| false);
    let progress = props.with_progress.unwrap_or(-1);

    let button_class = format!(
        "btn appearance-{} btn-{} {} {}",
        appearance,
        if disabled { "btn-disabled" } else { "" },
        if text.is_empty() { "no-text" } else { "" },
        if props.loading.unwrap_or(false) {
            "progress"
        } else {
            ""
        }
    );

    let show_icon = props.loading.unwrap_or_default() || props.icon.is_some();
    let props_signal = use_signal(|| props.clone());

    rsx!(
        div {
            class: {
                format_args!("btn-wrap {}", if small { "small" } else { "" })
            },
            onmouseenter: move |_| {
                if props_signal().tooltip.is_some() {
                     tooltip_visible.set(true);
                }
            },
            onmouseleave: move |_| {
                if props_signal().tooltip.is_some() {
                     tooltip_visible.set(false);
                }
            },
            if *tooltip_visible.read() {
                {props_signal().tooltip.as_ref().map(|tooltip| {
                    rsx!(
                       {tooltip}
                    )
                })}
            }
            {(!badge.is_empty()).then(|| rsx!(
                span {
                    aria_label: "Button Badge",
                    class: "badge",
                    "{badge}"
                }
            ))},
            button {
                aria_label: "{aria_label}",
                name: "{aria_label}",
                title: "{title}",
                disabled: if disabled { "true" } else { "false" },
                class: "{button_class}",
                // Optionally pass through click events.
                onclick: move |e| {
                    if !props.disabled.unwrap_or_default() {
                        let _ = props.onpress.as_ref().map(|f| f.call(e));
                    }
                },
                if progress >= 0 {
                        div {
                            class: "circular-progress",
                            background: format_args!("conic-gradient(var(--circular-indicator) calc({} * 1%), var(--circular-bar) 0)", progress),
                        }
                }
                if show_icon {
                        // for props, copy the defaults passed in by IconButton
                        common::icons::Icon {
                            ..common::icons::IconProps {
                                class: props.loading.unwrap_or_default().then_some("spin-container-for-button".to_string()),
                                size: 20,
                                fill:"currentColor".to_string(),
                                icon: if props.loading.unwrap_or_default() {Icon::Loader} else {props.icon.unwrap()},
                                disabled:  props.disabled.unwrap_or_default(),
                                disabled_fill: "#9CA3AF".to_string()
                            },
                        }
                }
                // We only need to include the text if it contains something.
                {(!text.is_empty()).then(|| rsx!(div {
                    class: "btn-text",
                    cursor: if disabled {"unset"} else {"pointer"},
                    "{text2}"
                }))},
            },
        },
    )
}
