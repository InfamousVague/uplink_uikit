use common::{icons::outline::Shape as Icon, language::get_local_text, state::State};
use dioxus::prelude::*;
use kit::{
    components::context_menu::{ContextItem, ContextMenu},
    elements::tooltip::Tooltip,
};

#[derive(Props, Clone, PartialEq)]
pub struct FileLocationProps {
    id: String,
    update_script: Signal<String>,
    on_press_storage: EventHandler<()>,
    on_press_local_disk: EventHandler<()>,
}

#[allow(non_snake_case)]
pub fn FileLocation(props: FileLocationProps) -> Element {
    let state = use_context::<Signal<State>>();
    let id = props.id.clone();
    let update_script_signal = props.update_script;

    let _ = use_future(move || async move {
        let script = update_script_signal.read();
        if !script.is_empty() {
            let _ = eval(script.to_string().as_str());
        }
    });

    let are_files_been_uploaded = !state.read().storage.files_in_queue_to_upload.is_empty();

    rsx!(ContextMenu {
        id: format!("{id}"),
        devmode: state.read().configuration.developer.developer_mode,
        items: rsx!(
            ContextItem {
                icon: Icon::Plus,
                aria_label: "attach-files-from-local-disk-into-chat".to_string(),
                text: get_local_text("files.attach-files-from-local-disk"),
                onpress: move |_| {
                    props.on_press_local_disk.call(());
                }
            },
            ContextItem {
                icon: Icon::FolderOpen,
                aria_label: "attach-files-from-storage-into-chat".to_string(),
                disabled: are_files_been_uploaded,
                text: get_local_text("files.attach-files-from-storage"),
                onpress: move |_| {
                    props.on_press_storage.call(());
                },
                tooltip: if are_files_been_uploaded {
                    rsx!(Tooltip {
                        arrow_position: kit::elements::tooltip::ArrowPosition::Right,
                        text: get_local_text("files.upload-in-progress-please-wait"),
                    })
                } else {
                    None
                },
            }
        )
    })
}
