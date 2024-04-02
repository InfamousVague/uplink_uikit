use std::backtrace::Backtrace;
use std::rc::Rc;

use super::*;

use crate::utils::auto_updater::DownloadState;
use chrono::Local;
use common::state::data_transfer::TransferTracker;
use common::state::ui::WindowMeta;
use common::state::State;
use common::STATIC_ARGS;
use dioxus_desktop::use_window;
use overlay::make_config;
use overlay::OverlayDom;
use warp::multipass;

pub fn use_warp_runner() {
    use_hook(|| {
        // Now turn on the warp runner and save it to the hook so it doesn't get dropped
        let mut runner = warp_runner::WarpRunner::new();
        runner.run();
        Rc::new(runner)
    });
}

pub(crate) fn use_bootstrap(identity: &multipass::identity::Identity) -> Option<Signal<State>> {
    let desktop = use_window();
    use_context_provider(|| Signal::new(DownloadState::default()));
    use_context_provider(|| Signal::new(components::settings::sidebar::Page::Profile));
    use_context_provider(|| Signal::new(TransferTracker::default()));
    use_context_provider(|| {
        let state = Signal::new(State::load());

        if STATIC_ARGS.use_mock {
            assert!(state().initialized);
        } else {
            state().set_own_identity(identity.clone().into());
        }

        // TODO: This overlay needs to be fixed in windows
        if cfg!(not(target_os = "windows")) && state().configuration.general.enable_overlay {
            let overlay_test = VirtualDom::new(OverlayDom);
            let window = desktop.new_window(overlay_test, make_config());
            state().ui.overlays.push(window);
        }

        let size = scaled_window_size(desktop.inner_size(), &desktop);
        // Update the window metadata now that we've created a window
        let window_meta = WindowMeta {
            focused: desktop.is_focused(),
            maximized: desktop.is_maximized(),
            minimized: desktop.is_minimized(),
            full_screen: state().ui.metadata.full_screen,
            minimal_view: size.width < 600,
        };
        state().ui.metadata = window_meta;
        state().set_warp_ch(WARP_CMD_CH.tx.clone());

        state
    });

    Some(use_context::<Signal<State>>())
}

pub fn set_app_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        let logs = logger::dump_logs();
        let crash_report = format!("{panic_info}\n{logs}\n");
        let backtrace = Backtrace::force_capture();
        let time = Local::now().format("%d-%m-%Y_%H-%M-%S").to_string();
        let path = STATIC_ARGS.crash_logs.join(format!("{}.log", time));
        let log = format!(
            "Uplink crashed:\n{}\nBackTrace:\n{}",
            crash_report, backtrace
        );
        println!("{crash_report}");
        if let Some(p) = path.parent() {
            let _ = fs::create_dir_all(p);
            match fs::write(&path, log) {
                Ok(_) => println!("The crash has been saved to {}", path.to_string_lossy()),
                Err(e) => println!("Unable to save crash log {}", e),
            }
        }
    }))
}

pub fn configure_logger(production_mode: bool, log_to_file: bool) {
    if log_to_file {
        logger::set_save_to_file(true);
    }

    if !production_mode {
        logger::set_write_to_stdout(true);
    }

    logger::init().expect("failed to init logger");

    ::log::debug!("starting uplink");
}

pub fn create_uplink_dirs() {
    // Guarantee all files in temp files directory will be deleted
    if STATIC_ARGS.temp_files.exists() {
        std::fs::remove_dir_all(&STATIC_ARGS.temp_files)
            .expect("Error removing temp files directory");
    }
    // Initializes the cache dir if needed
    std::fs::create_dir_all(&STATIC_ARGS.uplink_path).expect("Error creating Uplink directory");
    std::fs::create_dir_all(&STATIC_ARGS.warp_path).expect("Error creating Warp directory");
    std::fs::create_dir_all(&STATIC_ARGS.themes_path).expect("error creating themes directory");
    std::fs::create_dir_all(&STATIC_ARGS.fonts_path)
        .expect("error creating fonts themes directory");
    std::fs::create_dir_all(&STATIC_ARGS.temp_files)
        .expect("error creatings temporary files directory");
}

pub fn platform_quirks() {
    // Attempts to increase the file desc limit on unix-like systems
    // Note: Will be changed out in the future
    _ = fdlimit::raise_fd_limit();
}
