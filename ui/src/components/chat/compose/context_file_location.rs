use common::icons::outline::Shape as Icon;
use dioxus::prelude::*;
use dioxus_desktop::use_eval;
use kit::components::context_menu::{ContextItem, ContextMenu};

#[derive(Props)]
pub struct FileLocationProps<'a> {
    id: &'a String,
    update_script: &'a UseState<String>,
    on_press_storage: EventHandler<'a, ()>,
    on_press_local_disk: EventHandler<'a, ()>,
}

#[allow(non_snake_case)]
pub fn FileLocationContext<'a>(cx: Scope<'a, FileLocationProps<'a>>) -> Element<'a> {
    let id = cx.props.id.clone();
    let eval = use_eval(cx);
    use_future(cx, cx.props.update_script, |update_script| {
        to_owned![eval];
        async move {
            let script = update_script.get();
            if !script.is_empty() {
                eval(script.to_string());
            }
        }
    });

    cx.render(rsx!(ContextMenu {
        id: format!("{id}"),
        items: cx.render(rsx!(
            ContextItem {
                icon: Icon::ComputerDesktop,
                aria_label: "quick-profile-self-edit".into(),
                text: "Local Disk".into(),
                onpress: move |_| {
                    cx.props.on_press_local_disk.call(());
                }
            },
            hr {},
            ContextItem {
                icon: Icon::Cloud,
                aria_label: "quick-profile-self-edit".into(),
                text: "Uplink Storage".into(),
                onpress: move |_| {
                    cx.props.on_press_storage.call(());
                }
            }
        ))
    }))
}
