use std::fs;

use common::{
    get_images_dir,
    language::{get_local_text, get_local_text_with_args},
    state::{configuration::Configuration, State},
    warp_runner::TesseractCmd,
    STATIC_ARGS,
};
use dioxus::prelude::*;
use dioxus_desktop::use_window;
use futures::channel::oneshot;
use futures::StreamExt;
use kit::{
    components::context_menu::{ContextItem, ContextMenu},
    elements::{
        button::Button,
        input::{Input, Options, Validation},
        label::LabelWithEllipsis,
        tooltip::{ArrowPosition, Tooltip},
    },
};
use warp::multipass;

use tracing::log;

use common::icons::outline::Shape as Icon;
use common::{
    sounds,
    warp_runner::{MultiPassCmd, WarpCmd},
    WARP_CMD_CH,
};

use crate::{bootstrap::create_uplink_dirs, get_app_style, AuthPages};

#[derive(Clone)]
enum UnlockError {
    ValidationError,
    InvalidPin,
    Unknown,
}

impl UnlockError {
    fn translation(&self) -> String {
        match self {
            UnlockError::ValidationError => get_local_text("unlock.error-pin"),
            UnlockError::InvalidPin => get_local_text("unlock.invalid-pin"),
            UnlockError::Unknown => get_local_text("unlock.error-unknown-pin"),
        }
    }
}

// todo: go to the auth page if no account has been created
#[component]
pub fn Layout(page: Signal<AuthPages>, pin: Signal<String>) -> Element {
    log::trace!("rendering login entry point");
    let mut validation_failure: Signal<Option<UnlockError>> =
        use_signal(|| Some(UnlockError::ValidationError)); // By default no pin is an invalid pin.

    let mut error: Signal<Option<UnlockError>> = use_signal(|| None);
    let mut shown_error = use_signal(String::new);
    let desktop = use_window();

    let mut account_exists: Signal<Option<bool>> = use_signal(|| None);
    let mut cmd_in_progress = use_signal(|| false);
    let first_render = use_signal(|| true);
    let state = use_signal(State::load);
    let mut reset_input = use_signal(|| false);

    // On windows, is necessary use state on topbar controls, without using use_shared_state
    // So state is loaded there to use window_maximized and offer better UX
    if cfg!(target_os = "windows") && *first_render.read() {
        *first_render.write_silent() = false;
        state.write_silent().ui.window_maximized = false;
        let _ = state.write_silent().save();
    }

    // this will be needed later
    use_future(move || {
        to_owned![account_exists];
        async move {
            if account_exists().is_some() {
                println!("1 - Account exist is some");
                return;
            }
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            let (tx, rx) = oneshot::channel::<bool>();
            if let Err(e) =
                warp_cmd_tx.send(WarpCmd::Tesseract(TesseractCmd::AccountExists { rsp: tx }))
            {
                log::error!("failed to send warp command: {}", e);
                // returning true will prevent the account from being created
                return;
            }

            let exists = rx.await.unwrap_or(false);
            log::debug!("account_exists: {}", exists);
            println!("2 - Account exist is some");
            account_exists.set(Some(exists));
        }
    });

    let ch = use_coroutine(|mut rx: UnboundedReceiver<(String, Option<bool>)>| {
        to_owned![error, page, cmd_in_progress];
        async move {
            let config = Configuration::load_or_default();
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some((password, account)) = rx.next().await {
                let (tx, rx) =
                    oneshot::channel::<Result<multipass::identity::Identity, warp::error::Error>>();

                if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::TryLogIn {
                    passphrase: password,
                    rsp: tx,
                })) {
                    log::error!("failed to send warp command: {}", e);
                    cmd_in_progress.set(false);
                    continue;
                }

                let res = rx.await.expect("failed to get response from warp_runner");

                match res {
                    Ok(ident) => {
                        if config.audiovideo.interface_sounds {
                            sounds::Play(sounds::Sounds::On);
                        }

                        page.set(AuthPages::Success(ident));
                    }
                    Err(err) => match err {
                        warp::error::Error::DecryptionError => {
                            // check if account exists. can be the case when account got reset
                            if account.unwrap_or_default() {
                                // wrong password
                                error.set(Some(UnlockError::InvalidPin));
                                log::warn!("decryption error");
                            }
                        }
                        warp::error::Error::IdentityNotCreated => {
                            // this is supposed to fail.
                        }
                        _ => {
                            // unexpected
                            error.set(Some(UnlockError::Unknown));
                            log::error!("Login failed: {}", err);
                        }
                    },
                }
                cmd_in_progress.set(false);
            }
        }
    });

    // Set up validation options for the input field
    let pin_validation = Validation {
        // The input should have a maximum length of 32
        max_length: Some(32),
        // The input should have a minimum length of 4
        min_length: Some(4),
        // The input should only contain alphanumeric characters
        alpha_numeric_only: false,
        // The input should not contain any whitespace
        no_whitespace: true,
        // The input component validation is shared - if you need to allow just colons in, set this to true
        ignore_colons: false,
        // The input should allow any special characters
        // if you need special chars, select action to allow or block and pass a vec! with each char necessary, mainly if alpha_numeric_only is true
        special_chars: None,
    };

    println!(
        "3 - Account exist loading: {:?}",
        account_exists().is_none()
    );

    let loading = account_exists().is_none();

    let image_path = get_images_dir()
        .unwrap_or_default()
        .join("mascot")
        .join("idle_alt.png")
        .to_str()
        .map(|x| x.to_string())
        .unwrap_or_default();

    rsx!(
        style {{get_app_style(&state.read())}},
        div {
            id: "unlock-layout",
            aria_label: "unlock-layout",
            if loading {
                {rsx!(
                    div {
                        class: "skeletal-bars",
                        div {
                            class: "skeletal skeletal-bar",
                        },
                    }
                )}
            } else {
                {rsx! (
                    img {
                        class: "idle",
                        src: "{image_path}"
                    },
                    Input {
                        id: "unlock-input".to_owned(),
                        focus: true,
                        is_password: true,
                        icon: Icon::Key,
                        disable_onblur: true,
                        aria_label: "pin-input".to_string(),
                        disabled: loading || cmd_in_progress(),
                        placeholder: get_local_text("unlock.enter-pin"),
                        reset: reset_input,
                        options: Options {
                            with_validation: Some(pin_validation),
                            with_clear_btn: true,
                            with_label: if account_exists().unwrap_or_default()
                            {Some(get_welcome_message(&state.read()))}
                            else
                                {Some(get_local_text("unlock.create-password"))}, // TODO: Implement this.
                            ellipsis_on_label: Some(LabelWithEllipsis {
                                apply_ellipsis: true,
                                padding_right_for_ellipsis: 105,
                            }),
                            ..Default::default()
                        },
                        onchange: move |(val, validation_passed): (String, bool)| {
                            *pin.write_silent() = val.clone();
                            // Reset the error when the person changes the pin
                            if val.is_empty() || !shown_error().is_empty() {
                                shown_error.set(String::new());
                            }
                            if validation_passed {
                                let is_maximized = desktop.is_maximized();
                                state.write_silent().ui.window_maximized = is_maximized;
                                let _ = state.write_silent().save();
                                cmd_in_progress.set(true);
                                ch.send((val, account_exists()));
                                validation_failure.set(None);
                            } else {
                                validation_failure.set(Some(UnlockError::ValidationError));
                            }
                        },
                        onreturn: move |_| {
                                if let Some(validation_error) = validation_failure() {
                                    shown_error.set(validation_error.translation());
                                } else if let Some(e) = error() {
                                    shown_error.set(e.translation());
                                } else if !account_exists().unwrap_or_default()  {
                                    page.set(AuthPages::CreateOrRecover);
                                }
                                cmd_in_progress.set(false);
                        }
                    },
                   { (!shown_error().is_empty()).then(|| rsx!(
                        span {
                            class: "error",
                            "{shown_error}"
                        }
                    ))},
                    div {
                        class: "unlock-details",
                        span {
                            {get_local_text("unlock.notice")}
                        }
                    },
                    Button {
                        text: match account_exists().unwrap_or(true) {
                            true => if cmd_in_progress() {get_local_text("unlock.logging-in")} else {get_local_text("unlock.unlock-account")},
                            false => get_local_text("unlock.create-account"),
                        },
                        aria_label: "create-account-button".to_string(),
                        appearance: kit::elements::Appearance::Primary,
                        loading:  cmd_in_progress(),
                        disabled: cmd_in_progress() || validation_failure.read().is_some(),
                        onpress: move |_| {
                            // these are only for testing.
                            // page.set(AuthPages::CreateOrRecover);
                            // return;
                            let error_test: Option<UnlockError> = error.read().clone();
                            let validation_failure_test = validation_failure.read().clone();
                            if let Some(validation_error) = validation_failure_test {
                                shown_error.set(validation_error.translation());
                                reset_input.set(true);
                            } else if let Some(e) = error_test {
                                shown_error.set(e.translation());
                                reset_input.set(true);
                            } else {
                                page.set(AuthPages::CreateOrRecover);
                            }
                            cmd_in_progress.set(false);
                        }
                    },
                    ContextMenu {
                        key: "{key}-menu",
                        id: "unlock-context-menu".to_string(),
                        devmode: state.read().configuration.developer.developer_mode,
                        items: rsx!(
                            ContextItem {
                                icon: Icon::Trash,
                                danger: true,
                                aria_label: "account-reset".to_string(),
                                text: get_local_text("uplink.reset-account"),
                                onpress: move |_| {
                                    let _ = fs::remove_dir_all(&STATIC_ARGS.dot_uplink);
                                    page.set(AuthPages::EntryPoint);
                                    error.set(None);
                                    account_exists.set(Some(false));
                                    create_uplink_dirs();
                                }
                            },
                        ),
                        div {
                            class: "help-button",
                            Button {
                                aria_label: "help-button".to_string(),
                                appearance: kit::elements::Appearance::Secondary,
                                icon: Icon::QuestionMarkCircle,
                                tooltip: rsx!(
                                    Tooltip {
                                        arrow_position: ArrowPosition::Right,
                                        text: get_local_text("unlock.help"),
                                    }
                                ),
                            }
                        }
                    }
                )}
            }
        }
    )
}

fn get_welcome_message(state: &State) -> String {
    let name = match state.ui.cached_username.as_ref() {
        Some(name) => name.clone(),
        None => String::from("UNKNOWN"),
    };

    get_local_text_with_args("unlock.welcome", vec![("name", name)])
}
