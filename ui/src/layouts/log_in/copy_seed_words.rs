use std::time::Duration;

use arboard::Clipboard;
use common::{icons, language::get_local_text, state::State};
use dioxus::prelude::*;
use dioxus_desktop::{use_window, LogicalSize};
use kit::elements::{button::Button, label::Label, Appearance};
use tokio::time::sleep;

use super::AuthPages;
use crate::get_app_style;
use common::state::configuration::Configuration;
use common::{
    sounds,
    warp_runner::{MultiPassCmd, WarpCmd},
    WARP_CMD_CH,
};
use futures::channel::oneshot;
use futures::StreamExt;
use warp::multipass;

// styles for this layout are in layouts/style.scss
#[component]
pub fn Layout(page: Signal<AuthPages>, username: String, pin: String) -> Element {
    let state = use_signal(State::load);
    let window = use_window();
    let words: Signal<Option<(String, Vec<String>)>> = use_signal(|| None);

    if !matches!(&*page.read(), AuthPages::Success(_)) {
        window.set_inner_size(LogicalSize {
            width: 500.0,
            height: 480.0,
        });
    }

    use_resource(move || async move {
        let mnemonic = warp::crypto::keypair::generate_mnemonic_phrase(
            warp::crypto::keypair::PhraseType::Standard,
        )
        .into_phrase();
        (words.set(Some((
            mnemonic.clone(),
            mnemonic
                .split_ascii_whitespace()
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
        ))),)
    });

    rsx!(
        style {{get_app_style(&state.read())}},
        div {
            id: "copy-seed-words-layout",
            aria_label: "copy-seed-words-layout",
            div {
                class: "instructions-important",
                {get_local_text("copy-seed-words.instructions")}
            },
            Label {
                aria_label: "copy-seed-words".to_string(),
                text: get_local_text("copy-seed-words")
            },
            if let Some((seed_words, words)) = words() {
                {rsx!(SeedWords { page: page.clone(), username: username.clone(), pin: pin.clone(), seed_words: seed_words.clone(), words: words.clone() })}
            }
        }
    )
}

#[component]
fn SeedWords(
    page: Signal<AuthPages>,
    username: String,
    pin: String,
    seed_words: String,
    words: Vec<String>,
) -> Element {
    let copied = use_signal(|| false);
    let loading = use_signal(|| false);

    use_future(move || async move {
        if *copied.read() {
            sleep(Duration::from_secs(3)).await;
            *copied.write() = false;
        }
    });

    let ch = use_coroutine(|mut rx: UnboundedReceiver<()>| {
        to_owned![page, loading, username, pin, seed_words];
        async move {
            let config = Configuration::load_or_default();
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(()) = rx.next().await {
                loading.set(true);
                let (tx, rx) =
                    oneshot::channel::<Result<multipass::identity::Identity, warp::error::Error>>();

                if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::CreateIdentity {
                    username: username.clone(),
                    tesseract_passphrase: pin.clone(),
                    seed_words: seed_words.clone(),
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
    rsx! {
        {loading().then(|| rsx!(
            div {
                class: "overlay-load-shadow",
            },
        ))},
        div {
            class: format_args!("seed-words {}", if loading() {"progress"} else {""}),
            {words.chunks_exact(2).enumerate().map(|(idx, vals)| rsx! {
                div {
                    class: "row",
                    div {
                        class: "col",
                        span {
                            aria_label: "seed-word-number-{((idx * 2) + 1).to_string()}",
                            class: "num disable-select",
                            {((idx * 2) + 1).to_string()}
                        },
                        span {
                            aria_label: "seed-word-value-{((idx * 2) + 1).to_string()}",
                            class: "val",
                            {vals.first().cloned().unwrap_or_default()}
                        }
                    },
                    div {
                        class: "col",
                        span {
                            aria_label: "seed-word-number-{((idx * 2) + 2).to_string()}",
                            class: "num disable-select",
                            {((idx * 2) + 2).to_string()}
                        },
                        span {
                            aria_label: "seed-word-value-{((idx * 2) + 2).to_string()}",
                            class: "val",
                            {vals.get(1).cloned().unwrap_or_default()}
                        }
                    }
                }
            })}
        },
        div {
            class: "controls",
            Button {
                text: get_local_text("uplink.copy-seed"),
                aria_label: "copy-seed-button".to_string(),
                icon: icons::outline::Shape::BookmarkSquare,
                onpress: move |_| {
                    match Clipboard::new() {
                        Ok(mut c) => {
                            match c.set_text(words.join("\n").to_string()) {
                                Ok(_) => *copied.write() = true,
                                Err(e) => log::warn!("Unable to set text to clipboard: {e}"),
                            }
                        },
                        Err(e) => {
                            log::warn!("Unable to create clipboard reference: {e}");
                        }
                    };
                },
                appearance: Appearance::Secondary
            }
        }
        div {
            class: "controls",
            Button {
                text: get_local_text("uplink.go-back"),
                disabled: loading(),
                aria_label: "back-button".into(),
                icon: icons::outline::Shape::ChevronLeft,
                onpress: move |_| page.set(AuthPages::CreateOrRecover),
                appearance: Appearance::Secondary
            },
            Button {
                aria_label: "i-saved-it-button".into(),
                disabled: loading(),
                loading: loading(),
                text: get_local_text("copy-seed-words.finished"),
                onpress: move |_| {
                    ch.send(());
                }
            }
        }
        {copied.read().then(||{
            rsx!(div{
                class: "copied-toast",
                {get_local_text("uplink.copied-seed")}
            })
        })}
    }
}
