mod data;
mod presentation;
mod scripts;
pub use presentation::sidebar::Sidebar as ChatSidebar;
use presentation::welcome::Welcome;

use std::path::PathBuf;

use crate::{
    layouts::{chats::presentation::chat::Compose, slimbar::SlimbarLayout},
    utils::{
        clipboard::clipboard_data::get_files_path_from_clipboard,
        get_drag_event,
        verify_valid_paths::{decoded_pathbufs, verify_paths},
    },
};

use common::{
    language::{get_local_text, get_local_text_with_args},
    state::{ui, Action, State},
};
use dioxus::prelude::*;
use dioxus_desktop::{use_window, wry::FileDropEvent, DesktopContext};
use dioxus_html::input_data::keyboard_types::Code;
use dioxus_html::input_data::keyboard_types::Modifiers;

pub const FEEDBACK_TEXT_SCRIPT: &str = r#"
    const feedback_element = document.getElementById('overlay-text');
    feedback_element.textContent = '$TEXT';
"#;

pub const ANIMATION_DASH_SCRIPT: &str = r#"
    var dashElement = document.getElementById('dash-element')
    dashElement.style.animation = "border-dance 0.5s infinite linear"
"#;

pub const SELECT_CHAT_BAR: &str = r#"
    var chatBar = document.getElementsByClassName('chatbar')[0].getElementsByClassName('input_textarea')[0]
    chatBar.focus()
"#;

pub const OVERLAY_SCRIPT: &str = r#"
    var chatLayout = document.getElementById('chat-layout')

    var IS_DRAGGING = $IS_DRAGGING

    var overlayElement = document.getElementById('overlay-element')

    if (IS_DRAGGING) {
    chatLayout.classList.add('hover-effect')
    overlayElement.style.display = 'flex'
    } else {
    chatLayout.classList.remove('hover-effect')
    overlayElement.style.display = 'none'
    }
"#;

#[allow(non_snake_case)]
pub fn ChatLayout() -> Element {
    let mut state = use_context::<Signal<State>>();
    let mut first_render = use_signal(|| true);

    state.write_silent().ui.current_layout = ui::Layout::Welcome;

    let is_minimal_view = state.read().ui.is_minimal_view();
    let sidebar_hidden = state.read().ui.sidebar_hidden;
    let show_welcome = state.read().chats().active.is_none();

    if *first_render.read() && is_minimal_view {
        state.write().mutate(Action::SidebarHidden(true));
        first_render.set(false);
    }
    let drag_event: Signal<Option<FileDropEvent>> = use_signal(|| None);
    let window = use_window();
    let show_slimbar = state.read().show_slimbar();

    // #[cfg(target_os = "windows")]
    let _ = use_resource(move || {
        to_owned![state, window, drag_event];
        async move {
            // ondragover function from div does not work on windows
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if let FileDropEvent::Hovered { .. } = get_drag_event::get_drag_event() {
                    drop_and_attach_files(&window, &drag_event, state).await;
                }
            }
        }
    });

    // HACK: When enter in chats with notification, for some reason app is crashing
    // this a hack solution to clear notifications and not crash app if user change to chats page
    // TODO(Lucas): Uncomment this code later
    // if !state.read().ui.toast_notifications.is_empty() {
    //     state.write().ui.toast_notifications.clear();
    // }

    rsx!(
        div {
            id: "chat-layout",
            aria_label: "chat-layout",
            tabindex: "0",
            onkeydown: move |e: Event<KeyboardData>| {
                // HACK(Linux): Allow copy and paste files for Linux
                if cfg!(target_os = "linux") {
                    let keyboard_data = e;
                    if keyboard_data.code() == Code::KeyV
                        && keyboard_data.modifiers() == Modifiers::CONTROL
                    {
                        spawn({
                            to_owned![state];
                            async move {
                                let files_local_path = tokio::task::spawn_blocking(|| {
                                    get_files_path_from_clipboard().unwrap_or_default()
                                })
                                .await
                                .expect("Should succeed");
                                let active_chat_id = state.read().get_active_chat().map(|f| f.id).unwrap_or_default();
                                state
                                    .write()
                                    .mutate(Action::AppendChatAttachments(active_chat_id, files_local_path));
                            }
                        });
                }
                }
            },
            div {
                id: "drag-drop-element",
            }
            div {
                id: "overlay-element",
                class: "overlay-element",
                div {id: "dash-element", class: "dash-background active-animation"},
                p {id: "overlay-text0", class: "overlay-text"},
                p {id: "overlay-text", class: "overlay-text"}
            },
            if show_slimbar & !is_minimal_view {
                {rsx!(
                    SlimbarLayout { active: crate::UplinkRoute::ChatLayout{} },
                )}
            },
            // todo: consider showing a welcome screen if the sidebar is to be shown but there are no conversations in the sidebar. this case arises when
            // creating a new account on a mobile device.
            ChatSidebar {
                active_route: crate::UplinkRoute::ChatLayout {},
            },
            {show_welcome.then(|| rsx!(Welcome {}))},
            {(!show_welcome && (sidebar_hidden  || !state.read().ui.is_minimal_view())).then(|| rsx!(Compose {}))}
        }
    )
}

async fn drop_and_attach_files(
    window: &DesktopContext,
    drag_event: &Signal<Option<FileDropEvent>>,
    mut state: Signal<State>,
) {
    let new_files = drag_and_drop_function(window, drag_event).await;
    let chat_uuid = state
        .read()
        .get_active_chat()
        .map(|f| f.id)
        .unwrap_or_default();
    state
        .write()
        .mutate(Action::AppendChatAttachments(chat_uuid, new_files));
}

// Like ui::src:layout::storage::drag_and_drop_function
async fn drag_and_drop_function(
    window: &DesktopContext,
    drag_event: &Signal<Option<FileDropEvent>>,
) -> Vec<PathBuf> {
    *drag_event.write_silent() = Some(get_drag_event::get_drag_event());
    let mut new_files_to_upload = Vec::new();
    let _ = eval(ANIMATION_DASH_SCRIPT);
    loop {
        let file_drop_event = get_drag_event::get_drag_event();
        match file_drop_event {
            FileDropEvent::Hovered { paths, .. } => {
                if verify_paths(&paths) {
                    let mut script = OVERLAY_SCRIPT.replace("$IS_DRAGGING", "true");
                    let feedback_script = &FEEDBACK_TEXT_SCRIPT.replace(
                        "$TEXT",
                        &(if paths.len() > 1 {
                            get_local_text_with_args(
                                "files.files-to-upload",
                                vec![("num", paths.len())],
                            )
                        } else {
                            get_local_text("files.one-file-to-upload")
                        })
                        .to_string(),
                    );
                    script.push_str(feedback_script);
                    let _ = eval(&script);
                }
            }
            FileDropEvent::Dropped { paths, .. } => {
                if verify_paths(&paths) {
                    *drag_event.write_silent() = None;
                    new_files_to_upload = decoded_pathbufs(paths);
                    let mut script = OVERLAY_SCRIPT.replace("$IS_DRAGGING", "false");
                    script.push_str(SELECT_CHAT_BAR);
                    window.set_focus();
                    let _ = eval(&script);
                    break;
                }
            }
            _ => {
                *drag_event.write_silent() = None;
                let script = OVERLAY_SCRIPT.replace("$IS_DRAGGING", "false");
                let _ = eval(&script);
                break;
            }
        };
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    *drag_event.write_silent() = None;
    new_files_to_upload
}
