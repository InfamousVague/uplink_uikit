//TODO: Look into complex types from clippy in regards to props attr macro.
//      Low priority and can be ignored
#![allow(clippy::type_complexity)]
#![cfg_attr(feature = "production_mode", windows_subsystem = "windows")]
#![allow(non_snake_case)]
// the above macro will make uplink be a "window" application instead of a  "console" application for Windows.

use clap::Parser;
use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;
use common::language::{get_local_text, get_local_text_with_args};
use common::notifications::{NotificationAction, NOTIFICATION_LISTENER};
use common::profile_update_channel::PROFILE_CHANNEL_LISTENER;
use common::state::data_transfer::{TrackerType, TransferTracker};
use common::state::settings::GlobalShortcut;
use common::state::ui::Layout;
use common::state::ToastNotification;
use common::warp_runner::ui_adapter::MessageEvent;
use common::warp_runner::WarpEvent;
use common::{get_extras_dir, warp_runner, STATIC_ARGS, WARP_CMD_CH, WARP_EVENT_CH};

use dioxus::prelude::*;
use dioxus_desktop::tao::dpi::{LogicalPosition, PhysicalPosition};

use dioxus_desktop::{
    tao::{dpi::LogicalSize, event::WindowEvent},
    use_window,
};

use dioxus_router::prelude::{use_navigator, Outlet, Routable, Router};
use extensions::UplinkExtension;
use futures::channel::oneshot;
use futures::StreamExt;
use kit::components::context_menu::{ContextItem, ContextMenu};
use kit::components::topbar_controls::TopbarControls;
use kit::elements::button::Button;
use kit::elements::tooltip::ArrowPosition;
use kit::elements::Appearance;
use kit::layout::modal::Modal;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use tokio::sync::broadcast::error::RecvError;

use std::collections::HashMap;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use std::sync::Arc;

use crate::components::debug_logger::DebugLogger;
use crate::components::file_transfer::FileTransferModal;
use crate::components::toast::Toast;
use crate::components::topbar::release_info::Release_Info;
use crate::layouts::community::CommunityLayout;
use crate::layouts::friends::FriendsLayout;
use crate::layouts::loading::{use_loaded_assets, LoadingWash};
use crate::layouts::log_in::{AuthGuard, AuthPages};
use crate::layouts::settings::SettingsLayout;
use crate::layouts::storage::files_layout::FilesLayout;
use crate::misc_scripts::*;
use crate::utils::async_task_queue::{ListenerAction, ACTION_LISTENER};
use crate::utils::keyboard::shortcut_handlers::audio::ToggleType;
use crate::utils::keyboard::KeyboardShortcuts;
use dioxus_desktop::wry::application::event::Event as WryEvent;
use dioxus_desktop::{use_wry_event_handler, DesktopService, PhysicalSize};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};
use tracing::log::{self};

use muda::AboutMetadata;
use muda::Menu;
use muda::PredefinedMenuItem;
use muda::Submenu;

use crate::utils::auto_updater::{
    DownloadProgress, DownloadState, SoftwareDownloadCmd, SoftwareUpdateCmd,
};

use crate::layouts::chats::ChatLayout;
use crate::window_manager::WindowManagerCmdChannels;
use common::{
    state::{storage, ui::WindowMeta, Action, State},
    warp_runner::{ConstellationCmd, RayGunCmd, WarpCmd},
};
use std::panic;

use kit::STYLE as UIKIT_STYLES;
pub const APP_STYLE: &str = include_str!("./compiled_styles.css");
mod bootstrap;
mod components;
mod extension_browser;
mod layouts;
mod logger;
mod misc_scripts;
mod overlay;
mod utils;
mod webview_config;
mod window_builder;
mod window_manager;

pub static OPEN_DYSLEXIC: &str = include_str!("./open-dyslexic.css");

// used to close the popout player, among other things
pub static WINDOW_CMD_CH: Lazy<WindowManagerCmdChannels> = Lazy::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    WindowManagerCmdChannels {
        tx,
        rx: Arc::new(Mutex::new(rx)),
    }
});

pub fn main_lib() {
    // 1. fix random system quirks
    bootstrap::platform_quirks();

    // 2. configure logging via the cli
    let args = common::Args::parse();
    bootstrap::configure_logger(args.production_mode, args.log_to_file);

    // 3. Make sure that if the app panics we can catch it
    bootstrap::set_app_panic_hook();

    // 4. Make sure all system dirs are ready
    bootstrap::create_uplink_dirs();

    // mac needs the menu built a certain way.
    // the main_menu must not be dropped before launch_cfg is called.
    let main_menu = Menu::new();
    let app_menu = Submenu::new("Uplink", true);
    let edit_menu = Submenu::new("Edit", true);
    let window_menu = Submenu::new("Window", true);

    let _ = app_menu.append_items(&[
        &PredefinedMenuItem::about("About".into(), Some(AboutMetadata::default())),
        &PredefinedMenuItem::quit(None),
    ]);
    // add native shortcuts to `edit_menu` menu
    // in macOS native item are required to get keyboard shortcut
    // to works correctly
    let _ = edit_menu.append_items(&[
        &PredefinedMenuItem::undo(None),
        &PredefinedMenuItem::redo(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
    ]);

    let _ = window_menu.append_items(&[
        &PredefinedMenuItem::minimize(None),
        //&PredefinedMenuItem::zoom(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::fullscreen(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::close_window(None),
    ]);

    let _ = main_menu.append_items(&[&app_menu, &edit_menu, &window_menu]);

    #[cfg(target_os = "macos")]
    {
        main_menu.init_for_nsapp();
    }

    // 5. Finally, launch the app
    dioxus_desktop::launch_cfg(app, webview_config::webview_config())
}

#[allow(clippy::enum_variant_names)]
#[derive(Routable, Clone, Eq, PartialEq)]
pub enum UplinkRoute {
    // We want to wrap every router in a layout that renders the content via an outlet
    #[layout(app_layout)]
    //
    //
    #[redirect("/", || UplinkRoute::ChatLayout {})]
    #[route("/chat")]
    ChatLayout {},

    #[route("/settings")]
    SettingsLayout {},

    #[route("/friends")]
    FriendsLayout {},

    #[route("/files")]
    FilesLayout {},

    #[route("/community")]
    CommunityLayout {},
}

fn app(cx: Scope) -> Element {
    // 1. Make sure the warp engine is turned on before doing anything
    bootstrap::use_warp_runner(cx);

    // 2. Guard the app with the auth
    let auth = use_state(cx, || AuthPages::EntryPoint);
    let AuthPages::Success(identity) = auth.get() else {
        return render! {
        KeyboardShortcuts {
            is_on_auth_pages: true,
            on_global_shortcut: move |shortcut| {
                match shortcut {
                    GlobalShortcut::OpenCloseDevTools => utils::keyboard::shortcut_handlers::dev::open_close_dev_tools(cx.scope),
                    GlobalShortcut::Unknown => log::error!("Unknown `Shortcut` called!"),
                    _ => log::info!("Just Open Dev Tools shortcut works on Auth Pages!"),
                }
                log::debug!("shortcut called {:?}", shortcut);
            }
        },
        AuthGuard { page: auth.clone() }};
    };

    // 3. Make sure global context is setup before rendering anything downstream
    bootstrap::use_bootstrap(cx, identity)?;

    // 4. Throw up a loading screen until our assets are ready
    if use_loaded_assets(cx).value().is_none() {
        return render! { LoadingWash {} };
    }

    // 5. Finally, render the app
    render! {
        Router::<UplinkRoute>{}
    }
}

// This needs to be in a layout since the notification listener needs a handle to the router
// Eventually this restriction will be lifted once global contexts in dioxus are global accessible
fn app_layout(cx: Scope) -> Element {
    log::trace!("rendering app");

    // terminate the logger thread when the app exits.
    cx.use_hook(|| LogDropper {});

    use_auto_updater(cx)?;
    use_app_coroutines(cx)?;
    use_router_notification_listener(cx)?;

    let state = use_shared_state::<State>(cx)?;

    render! {
        AppStyle {}
        div { id: "app-wrap",
            Titlebar {},
            KeyboardShortcuts {
                on_global_shortcut: move |shortcut| {
                    match shortcut {
                        GlobalShortcut::ToggleMute => utils::keyboard::shortcut_handlers::audio::toggle(state.clone(), cx, ToggleType::Mute),
                        GlobalShortcut::ToggleDeafen => utils::keyboard::shortcut_handlers::audio::toggle(state.clone(), cx, ToggleType::Deafen),
                        GlobalShortcut::IncreaseFontSize => utils::keyboard::shortcut_handlers::font::increase_size(state.clone()),
                        GlobalShortcut::DecreaseFontSize => utils::keyboard::shortcut_handlers::font::decrease_size(state.clone()),
                        GlobalShortcut::OpenCloseDevTools => utils::keyboard::shortcut_handlers::dev::open_close_dev_tools(cx),
                        GlobalShortcut::ToggleDevmode => utils::keyboard::shortcut_handlers::dev::toggle_devmode(state.clone()),
                        GlobalShortcut::SetAppVisible => utils::keyboard::shortcut_handlers::navigation::set_app_visible(cx),
                        GlobalShortcut::Unknown => log::error!("Unknown `Shortcut` called!")
                    }
                    log::debug!("shortcut called {:?}", shortcut);
                }
            },
            Toasts {},
            Outlet::<UplinkRoute>{},
            AppLogger {},
            PrismScripts {},
        },
    }
}

fn AppStyle(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;
    render! {
        style { get_app_style(&state.read()) },
    }
}

pub fn get_app_style(state: &State) -> String {
    let mut font_style = String::new();
    if let Some(font) = state.ui.font.clone() {
        font_style = format!(
            "
        @font-face {{
            font-family: CustomFont;
            src: url('{}');
        }}
        body,
        html {{
            font-family: CustomFont, sans-serif;
        }}
        ",
            font.path
        );
    }

    // this gets rendered at the bottom. this way you don't have to scroll past all the use_futures to see what this function renders

    // render the Uplink app
    let open_dyslexic = if state.configuration.general.dyslexia_support {
        OPEN_DYSLEXIC
    } else {
        ""
    };

    let font_scale = format!("html {{ font-size: {}rem; }}", state.settings.font_scale());

    let theme = state
        .ui
        .theme
        .as_ref()
        .map(|theme| theme.styles.clone())
        .unwrap_or_default();

    let accent_color = state.ui.accent_color;
    let accent_color = if let Some(color) = accent_color {
        format!(
            ":root {{
                    --primary: rgb({},{},{});
                    --text-color-primary: {};
                }}",
            color.0,
            color.1,
            color.2,
            get_text_color(color.0, color.1, color.2)
        )
    } else {
        "".into()
    };

    format!("{UIKIT_STYLES} {APP_STYLE} {PRISM_STYLE} {PRISM_THEME} {theme} {accent_color} {font_style} {open_dyslexic} {font_scale}")
}

// Decide if text should be dark or bright
fn get_text_color(r: u8, g: u8, b: u8) -> &'static str {
    // See https://en.wikipedia.org/wiki/Relative_luminance
    let luminance: f64 = 0.2126729 * (r as f64 / 255.).powf(2.2)
        + 0.7151522 * (g as f64 / 255.).powf(2.2)
        + 0.0721750 * (b as f64 / 255.).powf(2.2);
    if luminance > 0.5 {
        "var(--text-color-dark)"
    } else {
        "var(--text-color)"
    }
}

fn use_auto_updater(cx: &ScopeState) -> Option<()> {
    let download_state = use_shared_state::<DownloadState>(cx)?;
    let updater_ch = use_coroutine(cx, |mut rx: UnboundedReceiver<SoftwareUpdateCmd>| {
        to_owned![download_state];
        async move {
            while let Some(mut ch) = rx.next().await {
                while let Some(percent) = ch.0.recv().await {
                    if percent >= download_state.read().progress + 5_f32 {
                        download_state.write().progress = percent;
                    }
                }
                download_state.write().stage = DownloadProgress::Finished;
            }
        }
    });

    let _download_ch = use_coroutine(cx, |mut rx: UnboundedReceiver<SoftwareDownloadCmd>| {
        to_owned![updater_ch];
        async move {
            while let Some(dest) = rx.next().await {
                let (tx, rx) = mpsc::unbounded_channel::<f32>();
                updater_ch.send(SoftwareUpdateCmd(rx));
                match utils::auto_updater::download_update(dest.0.clone(), tx).await {
                    Ok(downloaded_version) => {
                        log::debug!("downloaded version {downloaded_version}");
                    }
                    Err(e) => {
                        log::error!("failed to download update: {e}");
                    }
                }
            }
        }
    });

    Some(())
}

fn use_app_coroutines(cx: &ScopeState) -> Option<()> {
    let desktop = use_window(cx);
    let state = use_shared_state::<State>(cx)?;

    // don't fetch stuff from warp when using mock data
    let items_init = use_ref(cx, || STATIC_ARGS.use_mock);

    // `use_future`s
    // all of Uplinks periodic tasks are located here. it's a lot to read but
    // it's better to have them in one place. this makes it a lot easier to find them.
    // there are 2 categories of tasks: warp tasks and UI tasks
    //
    // warp tasks
    // handle warp events
    // initialize friends: load from warp and store in State
    // initialize conversations: same
    //
    // UI tasks
    // clear toasts
    // update message timestamps
    // control child windows
    // clear typing indicator
    //
    // misc
    // when a task requires the UI be updated, `needs_update` is set.
    // when mock data is used, friends and conversations are generated randomly,
    //     not loaded from Warp. however, warp_runner continues to operate normally.
    //

    // There is currently an issue in Tauri/Wry where the window size is not reported properly.
    // Thus we bind to the resize event itself and update the size from the webview.
    let webview = desktop.webview.clone();
    let first_resize = use_ref(cx, || true);
    use_wry_event_handler(cx, {
        to_owned![state, desktop, first_resize];
        move |event, _| match event {
            WryEvent::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                //log::trace!("FOCUS CHANGED {:?}", *focused);
                if state.read().ui.metadata.focused != *focused {
                    state.write().ui.metadata.focused = *focused;

                    if *focused {
                        state.write().ui.notifications.clear_badge();
                        let _ = state.write().save();
                    }
                }
            }
            WryEvent::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => state
                .write()
                .mutate(Action::ClearAllPopoutWindows(desktop.clone())),
            WryEvent::WindowEvent {
                event: WindowEvent::Moved(_),
                ..
            } => {
                // Dont use the arg provided by the WindowEvent as its not right on mac
                let position =
                    scaled_window_position(desktop.outer_position().unwrap_or_default(), &desktop);
                state.write_silent().ui.window_position = Some((position.x, position.y));
                let _ = state.write().save();
            }
            WryEvent::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                let current_position =
                    scaled_window_position(desktop.outer_position().unwrap_or_default(), &desktop);
                let (pos_x, pos_y) = state
                    .read()
                    .ui
                    .window_position
                    .unwrap_or(current_position.into());
                let (width, height) = state.read().ui.window_size.unwrap_or((950, 600));
                if *first_resize.read() {
                    if state.read().ui.metadata.full_screen {
                        desktop.set_fullscreen(true);
                    } else {
                        desktop.set_inner_size(LogicalSize::new(width, height));
                        desktop.set_maximized(state.read().ui.metadata.maximized);
                    }
                    desktop.set_outer_position(LogicalPosition::new(pos_x, pos_y));
                    *first_resize.write_silent() = false;
                }
                let size = scaled_window_size(webview.inner_size(), &desktop);
                let metadata = state.read().ui.metadata.clone();
                let new_metadata = WindowMeta {
                    focused: desktop.is_focused(),
                    maximized: desktop.is_maximized(),
                    minimized: desktop.is_minimized(),
                    full_screen: desktop.fullscreen().is_some(),
                    minimal_view: size.width < 600,
                };
                let mut changed = false;
                if metadata != new_metadata {
                    state.write_silent().ui.sidebar_hidden = new_metadata.minimal_view;
                    state.write_silent().ui.metadata = new_metadata;
                    changed = true;
                }
                if size.width != width || size.height != height {
                    state.write_silent().ui.window_size = Some((size.width, size.height));
                    let _ = state.write_silent().save();
                    changed = true;
                }
                if current_position.x != pos_x || current_position.y != pos_y {
                    state.write_silent().ui.window_position =
                        Some((current_position.x, current_position.y));
                    changed = true;
                }
                if changed {
                    let _ = state.write().save();
                }
            }
            _ => {}
        }
    });

    // update state in response to warp events
    use_future(cx, (), |_| {
        to_owned![state];
        let schedule: Arc<dyn Fn(ScopeId) + Send + Sync> = cx.schedule_update_any();
        async move {
            // don't process warp events until friends and chats have been loaded
            while !state.read().initialized {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            let mut ch = WARP_EVENT_CH.tx.subscribe();
            log::trace!("starting warp_runner use_future");
            // it should be sufficient to lock once at the start of the use_future. this is the only place the channel should be read from. in the off change that
            // the future restarts (it shouldn't), the lock should be dropped and this wouldn't block.
            while let Ok(evt) = ch.recv().await {
                // Update only relevant components for attachment progress events
                if let WarpEvent::Message(MessageEvent::AttachmentProgress {
                    progress,
                    location,
                    conversation_id,
                    msg,
                }) = evt
                {
                    if state.write_silent().update_outgoing_messages(
                        conversation_id,
                        msg,
                        location,
                        progress,
                    ) {
                        state.write();
                    } else {
                        let read = state.read();
                        if read
                            .get_active_chat()
                            .map(|c| c.id.eq(&conversation_id))
                            .unwrap_or_default()
                        {
                            //Update the component only instead of whole state
                            if let Some(v) = read.scope_ids.pending_message_component {
                                schedule(ScopeId(v))
                            }
                        }
                    }
                } else {
                    state.write().process_warp_event(evt);
                }
            }
        }
    });

    // focus handler for notifications
    use_future(cx, (), |_| {
        to_owned![desktop];
        async move {
            let channel = common::notifications::FOCUS_SCHEDULER.rx.clone();
            let mut ch = channel.lock().await;
            while (ch.recv().await).is_some() {
                desktop.set_focus();
            }
        }
    });

    // Listen to profile updates
    use_future(cx, (), |_| {
        to_owned![state];
        async move {
            while !state.read().initialized {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            let channel = PROFILE_CHANNEL_LISTENER.rx.clone();
            let mut ch = channel.lock().await;
            while let Some(action) = ch.recv().await {
                let mut id = state.read().get_own_identity();
                let did = action.did;
                if did.eq(&id.did_key()) {
                    if let Some(picture) = action.picture.as_ref() {
                        id.set_profile_picture(picture);
                    }
                    if let Some(banner) = action.banner.as_ref() {
                        id.set_profile_banner(banner);
                    }
                    state.write().set_own_identity(id);
                } else {
                    state.write().update_identity_with(did, |id| {
                        if let Some(picture) = action.picture.as_ref() {
                            id.set_profile_picture(picture);
                        }
                        if let Some(banner) = action.banner.as_ref() {
                            id.set_profile_banner(banner);
                        }
                    });
                }
            }
        }
    });

    let file_tracker = use_shared_state::<TransferTracker>(cx).unwrap();

    // Listen to async tasks actions that should be handled on main thread
    use_future(cx, (), |_| {
        to_owned![state, file_tracker];
        let schedule: Arc<dyn Fn(ScopeId) + Send + Sync> = cx.schedule_update_any();
        async move {
            let channel = ACTION_LISTENER.rx.clone();
            let mut ch = channel.lock().await;
            while let Some(action) = ch.recv().await {
                let transfer = !matches!(
                    action,
                    ListenerAction::ToastAction {
                        title: _,
                        content: _,
                        icon: _,
                        timeout: _
                    }
                );
                match action {
                    ListenerAction::ToastAction {
                        title,
                        content,
                        icon,
                        timeout,
                    } => {
                        state.write().mutate(Action::AddToastNotification(
                            ToastNotification::init(title, content, icon, timeout),
                        ));
                    }
                    ListenerAction::TransferProgress {
                        id,
                        download,
                        progression,
                    } => {
                        file_tracker.write_silent().update_file_upload(
                            id,
                            progression,
                            if download {
                                TrackerType::FileDownload
                            } else {
                                TrackerType::FileUpload
                            },
                        );
                    }
                    ListenerAction::FinishTransfer { id, download } => {
                        file_tracker.write_silent().remove_file_upload(
                            id,
                            if download {
                                TrackerType::FileDownload
                            } else {
                                TrackerType::FileUpload
                            },
                        );
                    }
                    ListenerAction::PauseTransfer { id, download } => {
                        file_tracker.write_silent().pause_file_upload(
                            id,
                            if download {
                                TrackerType::FileDownload
                            } else {
                                TrackerType::FileUpload
                            },
                        )
                    }
                    ListenerAction::CancelTransfer { id, download } => {
                        file_tracker.write_silent().cancel_file_upload(
                            id,
                            if download {
                                TrackerType::FileDownload
                            } else {
                                TrackerType::FileUpload
                            },
                        );
                    }
                }
                if transfer {
                    if let Some(v) = state.read().scope_ids.file_transfer {
                        schedule(ScopeId(v))
                    }
                    if let Some(v) = state.read().scope_ids.file_transfer_icon {
                        schedule(ScopeId(v))
                    }
                }
            }
        }
    });

    // clear toasts
    use_future(cx, (), |_| {
        to_owned![state];
        async move {
            loop {
                sleep(Duration::from_secs(1)).await;
                if !state.read().has_toasts() {
                    continue;
                }
                log::trace!("decrement toasts");
                state.write().decrement_toasts();
            }
        }
    });

    //Update active call
    use_future(cx, (), |_| {
        to_owned![state];
        async move {
            loop {
                sleep(Duration::from_secs(1)).await;
                if state.write_silent().ui.call_info.update_active_call() {
                    state.notify_consumers();
                }
            }
        }
    });

    // clear typing indicator
    use_future(cx, (), |_| {
        to_owned![state];
        async move {
            loop {
                sleep(Duration::from_secs(STATIC_ARGS.typing_indicator_timeout)).await;
                if state.write_silent().clear_typing_indicator(Instant::now()) {
                    log::trace!("clear typing indicator");
                    state.write();
                }
            }
        }
    });

    // periodically refresh message timestamps and friend's status messages
    use_future(cx, (), |_| {
        to_owned![state];
        async move {
            loop {
                // simply triggering an update will refresh the message timestamps
                sleep(Duration::from_secs(60)).await;
                log::trace!("refresh timestamps");
                state.write();
            }
        }
    });

    // check for updates
    use_future(cx, (), |_| {
        to_owned![state];
        async move {
            loop {
                let latest_release = match utils::auto_updater::check_for_release().await {
                    Ok(opt) => match opt {
                        Some(r) => r,
                        None => {
                            sleep(Duration::from_secs(3600 * 24)).await;
                            continue;
                        }
                    },
                    Err(e) => {
                        log::error!("failed to check for release: {e}");
                        sleep(Duration::from_secs(3600 * 24)).await;
                        continue;
                    }
                };
                if state.read().settings.update_dismissed == Some(latest_release.tag_name.clone()) {
                    sleep(Duration::from_secs(3600 * 24)).await;
                    continue;
                }
                state.write().update_available(latest_release.tag_name);
                sleep(Duration::from_secs(3600 * 24)).await;
            }
        }
    });

    // control child windows
    use_future(cx, (), |_| {
        to_owned![desktop, state];
        async move {
            let window_cmd_rx = WINDOW_CMD_CH.rx.clone();
            let mut ch = window_cmd_rx.lock().await;
            while let Some(cmd) = ch.recv().await {
                window_manager::handle_cmd(state.clone(), cmd, desktop.clone()).await;
            }
        }
    });

    // init state from warp
    // also init extensions
    use_future(cx, (), |_| {
        to_owned![state];
        async move {
            if state.read().initialized {
                return;
            }

            // this is technically bad because it blocks the async runtime
            match get_extensions() {
                Ok(ext) => {
                    state.write().mutate(Action::RegisterExtensions(ext));
                }
                Err(e) => {
                    log::error!("failed to get extensions: {e}");
                }
            }
            log::debug!(
                "Loaded {} extensions.",
                state.read().ui.extensions.values().count()
            );

            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            let res = loop {
                let (tx, rx) = oneshot::channel();
                if let Err(e) =
                    warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::InitializeWarp { rsp: tx }))
                {
                    log::error!("failed to send command to initialize warp {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                let res = rx.await.expect("failed to get response from warp_runner");

                let res = match res {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("failed to initialize warp: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                break res;
            };

            state
                .write()
                .init_warp(res.friends, res.chats, res.converted_identities);
        }
    });

    // initialize files
    use_future(cx, (), |_| {
        to_owned![items_init, state];
        async move {
            if *items_init.read() {
                return;
            }
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            let (tx, rx) = oneshot::channel::<Result<storage::Storage, warp::error::Error>>();

            if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                ConstellationCmd::GetItemsFromCurrentDirectory { rsp: tx },
            )) {
                log::error!("failed to initialize Files {}", e);
                return;
            }

            let res = rx.await.expect("failed to get response from warp_runner");

            log::trace!("init items");
            match res {
                Ok(storage) => state.write().storage = storage,
                Err(e) => {
                    log::error!("init items failed: {}", e);
                }
            }

            *items_init.write() = true;
        }
    });

    // detect when new extensions are placed in the "extensions" folder, and load them.
    use_future(cx, (), |_| {
        to_owned![state];
        async move {
            let (tx, mut rx) = futures::channel::mpsc::unbounded();
            let handler = move |res| {
                _ = tx.unbounded_send(res);
            };

            let mut watcher = match RecommendedWatcher::new(
                handler,
                notify::Config::default().with_poll_interval(Duration::from_secs(1)),
            ) {
                Ok(watcher) => watcher,
                Err(e) => {
                    log::error!("{e}");
                    return;
                }
            };

            // Add a path to be watched. All files and directories at that path and
            // below will be monitored for changes.
            if let Err(e) = watcher.watch(
                STATIC_ARGS.extensions_path.as_path(),
                RecursiveMode::Recursive,
            ) {
                log::error!("{e}");
                return;
            }

            while let Some(event) = rx.next().await {
                let event = match event {
                    Ok(event) => event,
                    Err(e) => {
                        log::error!("{e}");
                        continue;
                    }
                };

                log::debug!("{event:?}");
                match get_extensions() {
                    Ok(ext) => {
                        state.write().mutate(Action::RegisterExtensions(ext));
                    }
                    Err(e) => {
                        log::error!("failed to get extensions: {e}");
                    }
                }
            }
        }
    });

    Some(())
}

fn get_update_icon(cx: Scope) -> Element {
    log::trace!("rendering get_update_icon");
    let state = use_shared_state::<State>(cx)?;
    let download_state = use_shared_state::<DownloadState>(cx)?;
    let desktop = use_window(cx);
    let _download_ch = use_coroutine_handle::<SoftwareDownloadCmd>(cx)?;

    let new_version = match state.read().settings.update_available.as_ref() {
        Some(u) => u.clone(),
        None => return cx.render(rsx!("")),
    };

    let update_msg =
        get_local_text_with_args("uplink.update-available", vec![("version", new_version)]);
    let downloading_msg = get_local_text_with_args(
        "uplink.update-downloading",
        vec![("progress", download_state.read().progress as u32)],
    );
    let downloaded_msg = get_local_text("uplink.update-downloaded");

    let stage = download_state.read().stage;
    match stage {
        DownloadProgress::Idle => cx.render(rsx!(
            ContextMenu {
                key: "update-available-menu",
                id: "update-available-menu".to_string(),
                devmode: state.read().configuration.developer.developer_mode,
                items: cx.render(rsx!(
                    ContextItem {
                        aria_label: "update-menu-dismiss".into(),
                        text: get_local_text("uplink.update-menu-dismiss"),
                        onpress: move |_| {
                            state.write().mutate(Action::DismissUpdate);
                        }
                    },
                    ContextItem {
                        aria_label: "update-menu-download".into(),
                        text: get_local_text("uplink.update-menu-download"),
                        onpress: move |_| {
                            download_state.write().stage = DownloadProgress::PickFolder;

                        }
                    }
                )),
                div {
                    id: "update-available",
                    aria_label: "update-available",
                    onclick: move |_| {
                        download_state.write().stage = DownloadProgress::PickFolder;

                    },
                    IconElement {
                        icon: common::icons::solid::Shape::ArrowDownCircle,
                    },
                    "{update_msg}",
                }
            }
        )),
        DownloadProgress::PickFolder => cx.render(rsx!(get_download_modal {
            on_dismiss: move |_| {
                download_state.write().stage = DownloadProgress::Idle;
            },
            // is never used
            // on_submit: move |dest: PathBuf| {
            //     download_state.write().stage = DownloadProgress::Pending;
            //     download_state.write().destination = Some(dest.clone());
            //     download_ch.send(SoftwareDownloadCmd(dest));
            // }
        })),
        DownloadProgress::_Pending => cx.render(rsx!(div {
            id: "update-available",
            class: "topbar-item",
            aria_label: "update-available",
            "{downloading_msg}"
        })),
        DownloadProgress::Finished => {
            cx.render(rsx!(div {
                id: "update-available",
                class: "topbar-item",
                aria_label: "update-available",
                onclick: move |_| {
                    // be sure to update this before closing the app
                    state.write().mutate(Action::DismissUpdate);
                    if let Some(dest) = download_state.read().destination.clone() {
                        std::thread::spawn(move ||  {

                            let cmd = if cfg!(target_os = "windows") {
                                "explorer"
                            } else if cfg!(target_os = "linux") {
                                "xdg-open"
                            } else if cfg!(target_os = "macos") {
                                "open"
                            } else {
                               eprintln!("unknown OS type. failed to open files browser");
                               return;
                            };
                            Command::new(cmd)
                            .arg(dest)
                            .spawn()
                            .unwrap();
                        });
                        desktop.close();
                    } else {
                        log::error!("attempted to download update without download location");
                    }
                    download_state.write().destination = None;
                    download_state.write().stage = DownloadProgress::Idle;
                },
                "{downloaded_msg}"
            }))
        }
    }
}

#[component(no_case_check)]
pub fn get_download_modal<'a>(
    cx: Scope<'a>,
    //on_submit: EventHandler<'a, PathBuf>,
    on_dismiss: EventHandler<'a, ()>,
) -> Element<'a> {
    let download_location: &UseState<Option<PathBuf>> = use_state(cx, || None);

    let dl = download_location.current();
    let _disp_download_location = dl
        .as_ref()
        .clone()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_default();

    cx.render(rsx!(Modal {
        onclose: move |_| on_dismiss.call(()),
        open: true,
        transparent: false,
        close_on_click_inside_modal: true,
        children: cx.render(rsx!(
            div {
            class: "download-modal disp-flex col",
            h1 {
                get_local_text("updates.title")
            },
            ul {
                class: "instruction-list",
                li {
                    get_local_text("updates.instruction1")
                },
                li {
                    Button {
                        text: get_local_text("updates.download-label"),
                        aria_label: get_local_text("updates.download-label"),
                        appearance: Appearance::Secondary,
                        onpress: |_| {
                            let _ = open::that("https://github.com/Satellite-im/Uplink/releases/latest");
                        }
                    }
                },
                li {
                    get_local_text("updates.instruction2")
                },
                li {
                    get_local_text("updates.instruction3")
                },
                li {
                    get_local_text("updates.instruction4")
                }
            },
            p {
                get_local_text("updates.instruction5")
            },
            // dl.as_ref().clone().map(|dest| rsx!(
            //     Button {
            //         text: "download installer".into(),
            //         onpress: move |_| {
            //            on_submit.call(dest.clone());
            //         }
            //     }
            // ))
        }
        ))
    }))
}

fn AppLogger(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;

    if !state.read().initialized {
        return cx.render(rsx!(()));
    }

    cx.render(rsx!(state
        .read()
        .configuration
        .developer
        .developer_mode
        .then(|| rsx!(DebugLogger {}))))
}

fn Toasts(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;
    cx.render(rsx!(state.read().ui.toast_notifications.iter().map(
        |(id, toast)| {
            rsx!(Toast {
                id: *id,
                with_title: toast.title.clone(),
                with_content: toast.content.clone(),
                icon: toast.icon.unwrap_or(Icon::InformationCircle),
                appearance: Appearance::Secondary,
            },)
        }
    )))
}

fn Titlebar(cx: Scope) -> Element {
    let desktop = use_window(cx);

    cx.render(rsx!(
        div {
            class: "titlebar disable-select",
            Release_Info{},
            div {
                class: "draggable-topbar",
                onmousedown: move |_| { desktop.drag(); },
            },
            span {
                class: "inline-controls",
                get_update_icon{},
                TopbarControls {}
            },
        },
    ))
}

fn use_router_notification_listener(cx: &ScopeState) -> Option<()> {
    // this use_future replaces the notification_action_handler.
    let state = use_shared_state::<State>(cx)?;
    let navigator = use_navigator(cx);
    use_future(cx, (), |_| {
        to_owned![state, navigator];
        async move {
            let mut ch = NOTIFICATION_LISTENER.tx.subscribe();
            log::trace!("starting notification action listener");
            loop {
                let cmd = match ch.recv().await {
                    Ok(cmd) => cmd,
                    Err(RecvError::Closed) => {
                        log::debug!("RecvError::Closed while reading from NOTIFICATION_LISTENER");
                        return;
                    }
                    _ => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                log::debug!("handling notification action {:#?}", cmd);
                match cmd {
                    NotificationAction::DisplayChat(uuid) => {
                        state.write_silent().mutate(Action::ChatWith(&uuid, true));
                        navigator.replace(UplinkRoute::ChatLayout {});
                    }
                    NotificationAction::FriendListPending => {
                        // the FriendsLayout subscribes to these events and sets the layout accordingly.
                        // in this case, the layout would be FriendRoute::Pending
                        navigator.replace(UplinkRoute::FriendsLayout {});
                    }
                    _ => {}
                }
            }
        }
    });

    Some(())
}

fn get_extensions() -> Result<HashMap<String, UplinkExtension>, Box<dyn std::error::Error>> {
    fs::create_dir_all(&STATIC_ARGS.extensions_path)?;
    let mut extensions = HashMap::new();

    let mut add_to_extensions = |dir: fs::ReadDir| -> Result<(), Box<dyn std::error::Error>> {
        for entry in dir {
            let path = entry?.path();
            log::debug!("Found extension: {:?}", path);

            match UplinkExtension::new(path.clone()) {
                Ok(ext) => {
                    if ext.cargo_version() != extensions::CARGO_VERSION
                        || ext.rustc_version() != extensions::RUSTC_VERSION
                    {
                        log::warn!("failed to load extension: {:?} due to rustc/cargo version mismatch. cargo version: {}, rustc version: {}", &path, ext.cargo_version(), ext.rustc_version());
                        continue;
                    }
                    log::debug!("Loaded extension: {:?}", &path);
                    extensions.insert(ext.details().meta.name.into(), ext);
                }
                Err(e) => {
                    log::error!("Error loading extension: {:?}", e);
                }
            }
        }

        Ok(())
    };

    let user_extension_dir = fs::read_dir(&STATIC_ARGS.extensions_path)?;
    add_to_extensions(user_extension_dir)?;

    if STATIC_ARGS.production_mode {
        let uplink_extenions_path = common::get_extensions_dir()?;
        let uplink_extensions_dir = fs::read_dir(uplink_extenions_path)?;
        add_to_extensions(uplink_extensions_dir)?;
    }

    Ok(extensions)
}

fn scaled_window_size(
    inner: PhysicalSize<u32>,
    desktop: &std::rc::Rc<DesktopService>,
) -> PhysicalSize<u32> {
    if cfg!(target_os = "macos") {
        // On Mac window sizes are kinda funky.
        // They are scaled with the window scale factor so they dont correspond to app pixels
        let logical: LogicalSize<f64> = (inner.width as f64, inner.height as f64).into();
        let scale = desktop.webview.window().scale_factor();
        logical.to_physical(1_f64 / scale)
    } else {
        inner
    }
}

fn scaled_window_position(
    position: PhysicalPosition<i32>,
    desktop: &std::rc::Rc<DesktopService>,
) -> PhysicalPosition<i32> {
    if cfg!(target_os = "macos") {
        // On Mac window the positions are kinda funky.
        // They are scaled with the window scale factor so they dont correspond to actual position
        let logical: LogicalPosition<f64> = (position.x as f64, position.y as f64).into();
        let scale = desktop.webview.window().scale_factor();
        logical.to_physical(1_f64 / scale)
    } else {
        position
    }
}

#[component]
fn AppNav<'a>(
    cx: Scope,
    active: UplinkRoute,
    onnavigate: Option<EventHandler<'a, ()>>,
    tooltip_direction: Option<ArrowPosition>,
) -> Element<'a> {
    use kit::components::nav::Route as UIRoute;

    let state = use_shared_state::<State>(cx)?;
    let tracker = use_shared_state::<TransferTracker>(cx)?;
    state.write_silent().scope_ids.file_transfer_icon = Some(cx.scope_id().0);
    let navigator = use_navigator(cx);
    let pending_friends = state.read().friends().incoming_requests.len();
    let unreads: u32 = state
        .read()
        .chats_sidebar()
        .iter()
        .map(|c| c.unreads())
        .sum();
    let file_progress = tracker.read().total_progress();
    let file_progress_ctx = file_progress >= 0 && state.read().ui.current_layout != Layout::Storage;

    let chat_route = UIRoute {
        to: "/chat",
        name: get_local_text("uplink.chats"),
        icon: Icon::ChatBubbleBottomCenterText,
        with_badge: if unreads > 0 {
            Some(unreads.to_string())
        } else {
            None
        },
        context_items: (unreads > 0).then(|| {
            cx.render(rsx!(ContextItem {
                aria_label: "clear-unreads".into(),
                text: get_local_text("uplink.clear-unreads"),
                onpress: move |_| {
                    state.write().mutate(Action::ClearAllUnreads);
                }
            },))
        }),
        ..UIRoute::default()
    };
    let settings_route = UIRoute {
        to: "/settings",
        name: get_local_text("settings.settings"),
        icon: Icon::Cog6Tooth,
        ..UIRoute::default()
    };
    let friends_route = UIRoute {
        to: "/friends",
        name: get_local_text("friends.friends"),
        icon: Icon::Users,
        with_badge: if pending_friends > 0 {
            Some(pending_friends.to_string())
        } else {
            None
        },
        loading: None,
        ..UIRoute::default()
    };
    let files_route = UIRoute {
        to: "/files",
        name: get_local_text("files.files"),
        icon: Icon::Folder,
        progress_bar: Some(file_progress),
        context_items: file_progress_ctx.then(|| {
            cx.render(rsx!(FileTransferModal {
                state: state,
                modal: true
            }))
        }),
        ..UIRoute::default()
    };
    let _routes = vec![chat_route, files_route, friends_route, settings_route];

    render!(kit::components::nav::Nav {
        routes: _routes,
        active: match active {
            UplinkRoute::ChatLayout {} => "/chat",
            UplinkRoute::SettingsLayout {} => "/settings",
            UplinkRoute::FriendsLayout {} => "/friends",
            UplinkRoute::FilesLayout {} => "/files",
            _ => "",
        },
        onnavigate: move |r| {
            if let Some(f) = onnavigate {
                f.call(());
            }

            let new_layout = match r {
                "/chat" => UplinkRoute::ChatLayout {},
                "/settings" => UplinkRoute::SettingsLayout {},
                "/friends" => UplinkRoute::FriendsLayout {},
                "/files" => UplinkRoute::FilesLayout {},
                _ => UplinkRoute::ChatLayout {},
            };

            navigator.replace(new_layout);
        },
        tooltip_direction: tooltip_direction.unwrap_or(ArrowPosition::Bottom),
    })
}

struct LogDropper {}

impl Drop for LogDropper {
    fn drop(&mut self) {
        // this terminates the logger thread
        logger::set_save_to_file(false);
    }
}
