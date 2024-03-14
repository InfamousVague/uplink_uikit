use dioxus::prelude::*;

#[derive(Props, Clone)]
pub struct Props {
    onpress: EventHandler,
    active: bool,
    color: (u8, u8, u8),
}

#[allow(non_snake_case)]
pub fn ColorSwatch(props: Props) -> Element {
    let active = props
        .active
        .then(|| "active".to_string())
        .unwrap_or_default();

    rsx!(div {
        class: "color-swatch {active}",
        style: "background-color: rgb({props.color.0}, {props.color.1}, {props.color.2})",
        onclick: |_| props.onpress.call(()),
    })
}
