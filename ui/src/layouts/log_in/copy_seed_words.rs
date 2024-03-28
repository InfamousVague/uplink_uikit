use std::time::Duration;

use arboard::Clipboard;
use common::{icons, language::get_local_text, state::State};
use dioxus::prelude::*;
use dioxus_desktop::{use_window, LogicalSize};
use kit::elements::{button::Button, label::Label, Appearance};
use tokio::time::sleep;

use crate::get_app_style;

use super::AuthPages;

// styles for this layout are in layouts/style.scss
#[component]
pub fn Layout(page: Signal<AuthPages>, seed_words: Signal<String>) -> Element {
    let state = use_signal(State::load);
    let window = use_window();

    if !matches!(&*page.read(), AuthPages::Success(_)) {
        window.set_inner_size(LogicalSize {
            width: 500.0,
            height: 480.0,
        });
    }

    let words = use_resource(move || {
        to_owned![seed_words];
        async move {
            let mnemonic = warp::crypto::keypair::generate_mnemonic_phrase(
                warp::crypto::keypair::PhraseType::Standard,
            )
            .into_phrase();

            seed_words.set(mnemonic.clone());
            mnemonic
                .split_ascii_whitespace()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
        }
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
            if let Some(words) = words.value()() {
                {rsx!(SeedWords { page: page, words: words.clone() })}
            }
        }
    )
}

#[component]
fn SeedWords(page: Signal<AuthPages>, words: Vec<String>) -> Element {
    let mut copied = use_signal(|| false);
    use_resource(move || async move {
        if *copied.read() {
            sleep(Duration::from_secs(3)).await;
            *copied.write() = false;
        }
    });
    rsx! {
        div {
            class: "seed-words",
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
                aria_label: "back-button".to_string(),
                icon: icons::outline::Shape::ChevronLeft,
                onpress: move |_| page.set(AuthPages::CreateOrRecover),
                appearance: Appearance::Secondary
            },
            Button {
                aria_label: "i-saved-it-button".to_string(),
                text: get_local_text("copy-seed-words.finished"),
                onpress: move |_| {
                    page.set(AuthPages::EnterUserName);
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
