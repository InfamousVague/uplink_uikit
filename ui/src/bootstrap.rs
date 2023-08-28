use super::*;

use crate::utils::auto_updater::DownloadState;
use common::state::ui::WindowMeta;
use common::state::State;
use common::STATIC_ARGS;
use dioxus_desktop::use_window;
use overlay::make_config;
use overlay::OverlayDom;
use warp::multipass;

pub fn use_warp_runner(cx: &ScopeState) {
    cx.use_hook(|| {
        // Now turn on the warp runner and save it to the hook so it doesn't get dropped
        let mut runner = warp_runner::WarpRunner::new();
        runner.run();
        runner
    });
}

pub(crate) fn use_boostrap<'a>(
    cx: &'a ScopeState,
    identity: &multipass::identity::Identity,
) -> Option<&'a UseSharedState<State>> {
    let desktop = use_window(cx);
    use_shared_state_provider(cx, DownloadState::default);
    use_shared_state_provider(cx, || {
        let mut state = State::load();

        if STATIC_ARGS.use_mock {
            assert!(state.initialized);
        } else {
            state.set_own_identity(identity.clone().into());
        }

        // TODO: This overlay needs to be fixed in windows
        if cfg!(not(target_os = "windows")) && state.configuration.general.enable_overlay {
            let overlay_test = VirtualDom::new(OverlayDom);
            let window = desktop.new_window(overlay_test, make_config());
            state.ui.overlays.push(window);
        }

        let size = desktop.webview.inner_size();
        // Update the window metadata now that we've created a window
        let window_meta = WindowMeta {
            focused: desktop.is_focused(),
            maximized: desktop.is_maximized(),
            minimized: desktop.is_minimized(),
            minimal_view: size.width < get_window_minimal_width(desktop),
        };
        state.ui.metadata = window_meta;
        state.set_warp_ch(WARP_CMD_CH.tx.clone());

        state
    });

    use_shared_state::<State>(cx)
}

pub fn set_app_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        let intro = match panic_info.payload().downcast_ref::<&str>() {
            Some(s) => format!("panic occurred: {s:?}"),
            None => "panic occurred".into(),
        };
        let location = match panic_info.location() {
            Some(loc) => format!(" at file {}, line {}", loc.file(), loc.line()),
            None => "".into(),
        };

        let logs = logger::dump_logs();
        let crash_report = format!("{intro}{location}\n{logs}\n");
        println!("{crash_report}");
    }))
}

pub fn configure_logger(profile: Option<LogProfile>) {
    let max_log_level = if let Some(profile) = profile {
        match profile {
            LogProfile::Debug => {
                logger::set_write_to_stdout(true);
                LevelFilter::Debug
            }
            LogProfile::Trace => {
                logger::set_display_trace(true);
                logger::set_write_to_stdout(true);
                LevelFilter::Trace
            }
            LogProfile::Trace2 => {
                logger::set_display_warp(true);
                logger::set_display_trace(true);
                logger::set_write_to_stdout(true);
                LevelFilter::Trace
            }
            _ => LevelFilter::Debug,
        }
    } else {
        LevelFilter::Debug
    };

    logger::init_with_level(max_log_level).expect("failed to init logger");

    ::log::debug!("starting uplink");
}

pub fn create_uplink_dirs() {
    // Initializes the cache dir if needed
    std::fs::create_dir_all(&STATIC_ARGS.uplink_path).expect("Error creating Uplink directory");
    std::fs::create_dir_all(&STATIC_ARGS.warp_path).expect("Error creating Warp directory");
    std::fs::create_dir_all(&STATIC_ARGS.themes_path).expect("error creating themes directory");
    std::fs::create_dir_all(&STATIC_ARGS.fonts_path).expect("error fonts themes directory");
}

pub fn platform_quirks() {
    // Attempts to increase the file desc limit on unix-like systems
    // Note: Will be changed out in the future
    _ = fdlimit::raise_fd_limit();
}
