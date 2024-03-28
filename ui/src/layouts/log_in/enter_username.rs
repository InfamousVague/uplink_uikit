use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use common::state::configuration::Configuration;
use common::{
    sounds,
    warp_runner::{MultiPassCmd, WarpCmd},
    WARP_CMD_CH,
};
use dioxus::prelude::*;
use dioxus_desktop::{use_window, LogicalSize};
use futures::channel::oneshot;
use futures::StreamExt;
use kit::elements::label::Label;
use kit::elements::{
    button::Button,
    input::{Input, Options, Validation},
};
use tracing::log;
use warp::multipass;

use crate::AuthPages;

pub const MIN_USERNAME_LEN: i32 = 4;
pub const MAX_USERNAME_LEN: i32 = 32;

struct CreateAccountCmd {
    username: String,
    passphrase: String,
    seed_words: String,
}

#[component]
pub fn Layout(page: Signal<AuthPages>, pin: Signal<String>, seed_words: Signal<String>) -> Element {
    log::trace!("rendering enter username layout");
    let window = use_window();
    let mut loading = use_signal(|| false);

    if !matches!(&*page.read(), AuthPages::Success(_)) {
        window.set_inner_size(LogicalSize {
            width: 500.0,
            height: 250.0,
        });
    }

    let mut username = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut button_disabled = use_signal(|| true);

    let username_validation = Validation {
        // The input should have a maximum length of 32
        max_length: Some(MAX_USERNAME_LEN),
        // The input should have a minimum length of 4
        min_length: Some(MIN_USERNAME_LEN),
        // The input should only contain alphanumeric characters
        alpha_numeric_only: true,
        // The input should not contain any whitespace
        no_whitespace: true,
        // The input component validation is shared - if you need to allow just colons in, set this to true
        ignore_colons: false,
        // The input should allow any special characters
        // if you need special chars, select action to allow or block and pass a vec! with each char necessary, mainly if alpha_numeric_only is true
        special_chars: None,
    };

    let ch = use_coroutine(|mut rx: UnboundedReceiver<CreateAccountCmd>| {
        to_owned![page];
        async move {
            let config = Configuration::load_or_default();
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(CreateAccountCmd {
                username,
                passphrase,
                seed_words,
            }) = rx.next().await
            {
                loading.set(true);
                let (tx, rx) =
                    oneshot::channel::<Result<multipass::identity::Identity, warp::error::Error>>();

                if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::CreateIdentity {
                    username,
                    tesseract_passphrase: passphrase,
                    seed_words,
                    rsp: tx,
                })) {
                    log::error!("failed to send warp command: {}", e);
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
                    // todo: notify user
                    Err(e) => log::error!("create identity failed: {}", e),
                }
            }
        }
    });

    rsx!(
        {loading().then(|| rsx!(
            div {
                class: "overlay-load-shadow",
            },
        ))},
        div {
            id: "unlock-layout",
            class: format_args!("{}", if loading() {"progress"} else {""}),
            aria_label: "unlock-layout",
            Label {
                text: get_local_text("auth.enter-username")
            },
            div {
                class: "instructions",
                aria_label: "instructions",
                {get_local_text("auth.enter-username-subtext")}
            },
            Input {
                id: "username-input".to_owned(),
                focus: true,
                is_password: false,
                icon: Icon::Identification,
                aria_label: "username-input".to_string(),
                disable_onblur: true,
                disabled: loading(),
                placeholder: get_local_text("auth.enter-username"),
                options: Options {
                    with_validation: Some(username_validation),
                    with_clear_btn: true,
                    clear_on_submit: false,
                    ..Default::default()
                },
                onchange: move |(val, is_valid): (String, bool)| {
                    let should_disable = !is_valid;
                    if button_disabled() != should_disable {
                        button_disabled.set(should_disable);
                    }
                    username.set(val);
                },
                onreturn: move |_| {
                    if !button_disabled() {
                        ch.send(CreateAccountCmd {
                            username: username().to_string(),
                            passphrase: pin.read().to_string(),
                            seed_words: seed_words.read().to_string()
                        });
                    }
                }
            },
            Button {
                text:  get_local_text("unlock.create-account"),
                aria_label: "create-account-button".to_string(),
                appearance: kit::elements::Appearance::Primary,
                loading: loading(),
                disabled: button_disabled() || loading(),
                onpress: move |_| {
                    ch.send(CreateAccountCmd {
                        username: username().to_string(),
                        passphrase: pin.read().to_string(),
                        seed_words: seed_words.read().to_string()
                    });
                }
            }
        }
    )
}
