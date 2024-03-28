use common::{
    icons,
    language::get_local_text,
    state::State,
    warp_runner::{MultiPassCmd, WarpCmd},
    WARP_CMD_CH,
};
use dioxus::prelude::*;
use dioxus_desktop::{use_window, LogicalSize};
use futures::{channel::oneshot, StreamExt};
use kit::elements::{
    button::Button,
    input::{self, Options},
    label::Label,
    Appearance,
};

use crate::get_app_style;

use super::AuthPages;

enum SeedError {
    ValidationError,
    InvalidSeed,
}

impl SeedError {
    fn translation(&self) -> String {
        match self {
            SeedError::ValidationError => get_local_text("enter-seed-words.error-seed"),
            SeedError::InvalidSeed => get_local_text("enter-seed-words.invalid-seed"),
        }
    }
}

struct Cmd {
    seed_words: String,
    passphrase: String,
}

// styles for this layout are in layouts/style.scss
#[component]
pub fn Layout(pin: Signal<String>, page: Signal<AuthPages>) -> Element {
    let mut state = use_signal(|| State::load());
    let mut loading = use_signal(|| false);
    let mut input: Signal<Vec<_>> = use_signal(|| (0..12).map(|_| String::new()).collect());
    let mut seed_error = use_signal(|| None);
    let mut focus = use_signal(|| 0);

    let window = use_window();

    if !matches!(&*page.read(), AuthPages::Success(_)) {
        window.set_inner_size(LogicalSize {
            width: 500.0,
            height: 480.0,
        });
    }

    use_effect(move || {
        spawn(async move {
            let mut eval_result = eval(include_str!("./enter_seed_handler.js"));
            loop {
                if let Ok(val) = eval_result.recv().await {
                    let paste = val
                        .to_string()
                        .replace("\\\\", "\\")
                        .replace("\\r", "\r")
                        .replace("\\n", "\n");
                    let paste = &paste[1..(paste.len() - 1)]; // Trim the apostrophes from the input
                    if !paste.is_empty() {
                        let phrases = paste.lines().collect::<Vec<_>>();
                        for i in 0..12 {
                            if i < phrases.len() {
                                input.with_mut(|v: &mut Vec<String>| v[i] = phrases[i].into());
                            }
                        }
                    }
                }
            }
        });
    });
    // todo: show toasts to inform user of errors.
    let ch = use_coroutine(|mut rx: UnboundedReceiver<Cmd>| {
        to_owned![loading, page, seed_error];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(Cmd {
                seed_words,
                passphrase,
            }) = rx.next().await
            {
                let (tx, rx) = oneshot::channel();

                if let Err(e) =
                    warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::RecoverIdentity {
                        passphrase,
                        seed_words,
                        rsp: tx,
                    }))
                {
                    log::error!("failed to send multipass cmd: {e}");
                    continue;
                }

                let rsp = match rx.await {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("warp runner cmd cancelled: {e}");
                        continue;
                    }
                };

                match rsp {
                    Ok(ident) => {
                        page.set(AuthPages::Success(ident));
                    }
                    Err(e) => {
                        loading.set(false);
                        seed_error.set(Some(SeedError::InvalidSeed));
                        log::error!("warp runner cmd cancelled: {e}");
                        continue;
                    }
                };
            }
        }
    });

    rsx!(
        style { {get_app_style(&state())} },
        div {
            id: "enter-seed-words-layout",
            aria_label: "enter-seed-words-layout",
            Label {
                aria_label: "enter-seed-words".to_string(),
                text: get_local_text("enter-seed-words")
            },
            div {
                class: "instructions",
                aria_label: "instructions",
                {get_local_text("enter-seed-words.instructions")}
            },
            div {
                class: "seed-words",
                {(0..6).map(|idx|{
                    let idx = idx * 2;
                    let other = idx + 1;
                    rsx!(div {
                        class: "row",
                        div {
                            class: "col",
                            span {
                                aria_label: "seed-word-number-{(idx + 1).to_string()}",
                                class: "num disable-select",
                                {(idx + 1).to_string()}
                            },
                            input::Input {
                                aria_label: "recovery-seed-input-".to_string() + &(idx + 1).to_string(),
                                value: input.read()[idx].clone(),
                                select_on_focus: *focus.read() == idx,
                                focus: *focus.read() == idx, // select class gets removed on focus. this forces an update
                                placeholder: "".to_string(),
                                disable_onblur: true,
                                options: Options {
                                    clear_on_submit: false,
                                    ..Default::default()
                                },
                                onfocus: move |_|{
                                    *focus.write() = idx;
                                },
                                onchange: move |(x, is_valid): (String, bool)| {
                                    if x.is_empty() || seed_error.read().is_some() {
                                        seed_error.set(None);
                                    }
                                    if is_valid {
                                        input.with_mut(|v|v[idx] = x);
                                    } else{
                                        seed_error.set(Some(SeedError::ValidationError));
                                    }
                                },
                                onreturn: move |_| {
                                    let f = *focus.read();
                                    *focus.write() = (f + 1) % 12;
                                }
                            },
                        },
                        div {
                            class: "col",
                            span {
                                aria_label: "seed-word-number-{(other + 1).to_string()}",
                                class: "num disable-select",
                                {(other + 1).to_string()}
                            },
                            input::Input {
                                aria_label: "recovery-seed-input-".to_string() + &(other + 1).to_string(),
                                value: input.read()[other].clone(),
                                focus: *focus.read() == other,
                                select_on_focus: *focus.read() == other, // select class gets removed on focus. this forces an update
                                placeholder: "".to_string(),
                                disable_onblur: true,
                                options: Options {
                                    clear_on_submit: false,
                                    ..Default::default()
                                },
                                onfocus: move |_|{
                                    *focus.write() = other;
                                },
                                onchange: move |(x, is_valid): (String, bool)| {
                                    if x.is_empty() || seed_error.read().is_some() {
                                        seed_error.set(None);
                                    }
                                    if is_valid {
                                        input.with_mut(|v|v[other] = x);
                                    } else{
                                        seed_error.set(Some(SeedError::ValidationError));
                                    }
                                },
                                onreturn: move |_| {
                                    if other == 11 {
                                        loading.set(true);
                                        log::debug!("seed {}", input.read().join(" "));
                                        ch.send(Cmd {
                                            seed_words: input.read().join(" ").clone(),
                                            passphrase: pin.read().clone()
                                        });
                                    } else {
                                        let f = *focus.read();
                                        *focus.write() = (f + 1) % 12;
                                    }
                                }
                            },
                        }
                    })
                })}
            }
            {seed_error.as_ref().map(|e| rsx!(
                span {
                    aria_label: "input-error",
                    class: "error",
                    {e.translation()}
                }
            ))},
            div {
                class: "button-container",
                // todo: add 12 separate input boxes per figma
                Button {
                    aria_label: "back-button".to_string(),
                    text: get_local_text("uplink.go-back"),
                    icon: icons::outline::Shape::ChevronLeft,
                    onpress: move |_| page.set(AuthPages::CreateOrRecover),
                    appearance: Appearance::Secondary
                },
                Button {
                    aria_label: "recover-account-button".to_string(),
                    text: get_local_text("enter-seed-words.submit"),
                    disabled: loading(),
                    onpress: move |_| {
                        loading.set(true);
                        ch.send(Cmd {
                            seed_words: input.read().join(" ").clone(),
                            passphrase: pin.read().clone()
                        });
                    }
                }
            }
        }
    )
}
