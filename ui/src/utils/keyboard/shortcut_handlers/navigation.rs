use dioxus_desktop::use_window;

/// The functionality will operate correctly only when the application is not in fullscreen mode.
///
/// In fullscreen mode, activating the shortcut for the first time will minimize the application.
///
/// Upon a second activation, the shortcut will then execute the intended action of hiding the application.
pub fn use_set_app_visible() {
    let window = use_window();

    window.set_fullscreen(false);

    if !window.is_focused() && !window.is_minimized() {
        window.set_focus();
    } else if !window.is_visible() || window.is_minimized() {
        window.set_minimized(false);
        window.set_visible(true);
        window.set_focus();
    } else if window.is_visible() || !window.is_minimized() {
        window.set_minimized(true);
        window.set_visible(false);
    }
}
