use dioxus::prelude::*;

#[derive(Props)]
pub struct Props<'a> {
    onpress: EventHandler<'a>,
    active: bool,
    color: (u8, u8, u8),
}

#[allow(non_snake_case)]
pub fn ColorSwatch<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let active = cx
        .props
        .active
        .then(|| "active".to_string())
        .unwrap_or_default();

    cx.render(rsx!(div {
        class: "color-swatch {active}",
        aria_label:
            "color-swatch-button-rgb-{cx.props.color.0}-{cx.props.color.1}-{cx.props.color.2}",
        style: "background-color: rgb({cx.props.color.0}, {cx.props.color.1}, {cx.props.color.2})",
        onclick: |_| cx.props.onpress.call(()),
    }))
}
