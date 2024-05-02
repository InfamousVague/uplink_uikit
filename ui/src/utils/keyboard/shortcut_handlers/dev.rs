use common::state::{action::ConfigAction, Action, State};
use dioxus::prelude::*;
use dioxus_desktop::use_window;

pub fn use_open_close_dev_tools() {
    let window = use_window();
    if window.webview.is_devtools_open() {
        window.webview.close_devtools();
    } else {
        window.webview.open_devtools();
    }
}
pub fn toggle_devmode(mut state: Signal<State>) {
    let devmode = state.peek().configuration.peek().developer.developer_mode;
    state
        .write()
        .mutate(Action::Config(ConfigAction::SetDevModeEnabled(!devmode)));
}
