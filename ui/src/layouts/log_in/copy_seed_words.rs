use std::time::Duration;

use arboard::Clipboard;
use common::{icons, language::get_local_text, state::State};
use dioxus::prelude::*;
use dioxus_desktop::use_window;
use kit::elements::{button::Button, label::Label, Appearance};
use tokio::time::sleep;

use super::AuthPages;
use crate::get_app_style;
use crate::layouts::log_in::update_window_size;
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
pub fn Layout(cx: Scope, page: UseState<AuthPages>, username: String, pin: String) -> Element {
    let state = use_ref(cx, State::load);
    let window = use_window(cx);

    if !matches!(&*page.current(), AuthPages::Success(_)) {
        update_window_size(window, 500.0, 500.0);
    }

    let words = use_future(cx, (), |_| async move {
        let mnemonic = warp::crypto::keypair::generate_mnemonic_phrase(
            warp::crypto::keypair::PhraseType::Standard,
        )
        .into_phrase();
        (
            mnemonic.clone(),
            mnemonic
                .split_ascii_whitespace()
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
        )
    });

    cx.render(rsx!(
        style {get_app_style(&state.read())},
        div {
            id: "copy-seed-words-layout",
            aria_label: "copy-seed-words-layout",
            div {
                class: "instructions-important",
                get_local_text("copy-seed-words.instructions")
            },
            Label {
                aria_label: "copy-seed-words".into(),
                text: get_local_text("copy-seed-words")
            },
            if let Some((seed_words, words)) = words.value() {
                rsx!{ SeedWords { page: page.clone(), username: username.clone(), pin: pin.clone(), seed_words: seed_words.clone(), words: words.clone() } }
            }
        }
    ))
}

#[component]
fn SeedWords(
    cx: Scope,
    page: UseState<AuthPages>,
    username: String,
    pin: String,
    seed_words: String,
    words: Vec<String>,
) -> Element {
    let copied = use_ref(cx, || false);
    let loading = use_state(cx, || false);

    use_future(cx, copied, |current| async move {
        if *current.read() {
            sleep(Duration::from_secs(3)).await;
            *current.write() = false;
        }
    });

    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<()>| {
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
    render! {
        loading.get().then(|| rsx!(
            div {
                class: "overlay-load-shadow",
            },
        )),
        div {
            class: format_args!("seed-words {}", if *loading.get() {"progress"} else {""}),
            words.chunks_exact(2).enumerate().map(|(idx, vals)| rsx! {
                div {
                    class: "row",
                    div {
                        class: "col",
                        span {
                            aria_label: "seed-word-number-{((idx * 2) + 1).to_string()}",
                            class: "num disable-select", ((idx * 2) + 1).to_string()
                        },
                        span {
                            aria_label: "seed-word-value-{((idx * 2) + 1).to_string()}",
                            class: "val", vals.first().cloned().unwrap_or_default()
                        }
                    },
                    div {
                        class: "col",
                        span {
                            aria_label: "seed-word-number-{((idx * 2) + 2).to_string()}",
                            class: "num disable-select", ((idx * 2) + 2).to_string()
                        },
                        span {
                            aria_label: "seed-word-value-{((idx * 2) + 2).to_string()}",
                            class: "val", vals.get(1).cloned().unwrap_or_default()
                        }
                    }
                }
            })
        },
        div {
            class: "controls",
            Button {
                text: get_local_text("uplink.copy-seed"),
                aria_label: "copy-seed-button".into(),
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
                disabled: *loading.get(),
                aria_label: "back-button".into(),
                icon: icons::outline::Shape::ChevronLeft,
                onpress: move |_| page.set(AuthPages::CreateOrRecover),
                appearance: Appearance::Secondary
            },
            Button {
                aria_label: "i-saved-it-button".into(),
                disabled: *loading.get(),
                loading: *loading.get(),
                text: get_local_text("copy-seed-words.finished"),
                onpress: move |_| {
                    ch.send(());
                }
            }
        }
        copied.read().then(||{
            rsx!(div{
                class: "copied-toast",
                get_local_text("uplink.copied-seed")
            })
        })
    }
}
