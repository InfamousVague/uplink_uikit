//! This was made for the chatbar but it turns out that a contenteditable div is needed to render markdown. This is a temporary solution.
//! this could be merged with kit/src/elements/input and make the input element use a textarea based on a property.
//! that might helpful if a textarea needed to perform input validation.

use dioxus::prelude::*;
use dioxus_html::input_data::keyboard_types::Code;
use dioxus_html::input_data::keyboard_types::Modifiers;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tracing::log;
use uuid::Uuid;

// "{\"Input\":\"((?:.|\n)*)\"}"
static INPUT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\{\"Input\":\"((?:.|\n)+)\"}"#).unwrap());

#[derive(Clone, Copy, PartialEq)]
pub enum Size {
    Small,
    Normal,
}

#[derive(Debug, Clone, Deserialize)]
pub enum JSTextData {
    Init,
    Input(String),
    Cursor(i64),
    KeyPress(Code),
    Submit,
}

impl Size {
    fn get_height(&self) -> &str {
        match self {
            Size::Small => "0",
            _ => "",
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    #[props(default = "".to_owned())]
    id: String,
    #[props(default = false)]
    ignore_focus: bool,
    #[props(default = false)]
    loading: bool,
    #[props(default = "".to_owned())]
    placeholder: String,
    #[props(default = 1025)]
    max_length: i32,
    #[props(default = Size::Normal)]
    size: Size,
    #[props(default = "".to_owned())]
    aria_label: String,
    onchange: EventHandler<(String, bool)>,
    onreturn: EventHandler<(String, bool, Code)>,
    oncursor_update: Option<EventHandler<(String, i64)>>,
    onkeyup: Option<EventHandler<Code>>,
    on_paste_keydown: Option<EventHandler<Event<KeyboardData>>>,
    value: String,
    #[props(default = false)]
    is_disabled: bool,
    #[props(default = false)]
    show_char_counter: bool,
    #[props(default = false)]
    prevent_up_down_arrows: bool,
    onup_down_arrow: Option<EventHandler<Code>>,
}

#[allow(non_snake_case)]
pub fn Input(props: Props) -> Element {
    log::trace!("render input");
    let mut left_shift_pressed = use_hook(|| CopyValue::new(false));
    let mut right_shift_pressed = use_hook(|| CopyValue::new(false));
    let mut enter_pressed = use_hook(|| CopyValue::new(false));
    let mut numpad_enter_pressed = use_hook(|| CopyValue::new(false));

    let Props {
        id,
        ignore_focus: _,
        loading,
        placeholder,
        max_length,
        size,
        aria_label,
        onchange,
        onreturn,
        oncursor_update,
        onkeyup,
        on_paste_keydown,
        value,
        is_disabled,
        show_char_counter,
        prevent_up_down_arrows,
        onup_down_arrow,
    } = props.clone();

    let mut sig_id = use_signal(|| {
        if id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            id.clone()
        }
    });
    let mut text_value = use_signal(|| value.clone());

    // If the id changed update the signal
    if !id.is_empty() && sig_id() != id {
        sig_id.set(id);
    }

    let focus_script = if props.ignore_focus {
        String::new()
    } else {
        include_str!("./focus.js").replace("$UUID", &sig_id())
    };

    let _ = eval(&focus_script);

    let script = include_str!("./script.js")
        .replace("$UUID", &sig_id())
        .replace("$MULTI_LINE", &format!("{}", true));
    let disabled = loading || is_disabled;

    let sync = include_str!("./sync_data.js").replace("$UUID", &sig_id());

    use_effect(use_reactive(&value, move |value| {
        if show_char_counter {
            let _ = eval(&sync.replace("$TEXT", &value));
        }
        if !text_value.peek().eq(&value) {
            text_value.set(value);
            sig_id.write();
        }
    }));

    let mut update_cursor = use_signal(|| false);
    let do_cursor_update = oncursor_update.is_some();
    use_effect(move || {
        if *update_cursor.read() {
            // let cursor_script = include_str!("./cursor_script.js").replace("$ID", &sig_id.peek());
            // spawn(async move {
            //     if let Some(e) = oncursor_update {
            //         let eval_result = eval(&cursor_script);
            //         if let Ok(val) = eval_result.join().await {
            //             // For some reason calling this and spamming keys makes the input update incorrectly
            //             e.call((text_value.peek().clone(), val.as_i64().unwrap_or_default()));
            //         }
            //     }
            // });
            *update_cursor.write() = false;
        }
    });

    rsx! (
        div {
            id: "input-group-{sig_id}",
            class: "input-group",
            aria_label: "input-group",
            div {
                class: format_args!("input {}", if disabled { "disabled" } else { "" }),
                height: "{size.get_height()}",
                textarea {
                    key: "textarea-key-{sig_id}",
                    class: format_args!("{} {}", "input_textarea", if prevent_up_down_arrows {"up-down-disabled"} else {""}),
                    id: "{sig_id}",
                    aria_label: "{aria_label}",
                    disabled: "{disabled}",
                    value: "{text_value.peek()}",
                    maxlength: "{max_length}",
                    placeholder: format_args!("{}", if is_disabled {"".to_string()} else { placeholder }),
                    onblur: move |_| {
                        onreturn.call((text_value.peek().to_string(), false, Code::Enter));
                    },
                    oninput: {
                        move |evt| {
                            let current_val = evt.value().clone();
                            *text_value.write() = current_val.clone();
                            onchange.call((current_val, true));
                            if do_cursor_update {
                                *update_cursor.write() = true;
                            }
                        }
                    },
                    onkeyup: move |evt| {
                        match evt.code() {
                            Code::ShiftLeft => *left_shift_pressed.write() = false,
                            Code::ShiftRight => *right_shift_pressed.write() = false,
                            Code::Enter => *enter_pressed.write() = false,
                            Code::NumpadEnter => *numpad_enter_pressed.write() = false,
                            _ => {}
                        };
                        if let Some(e) = onkeyup {
                            e.call(evt.code());
                        }
                    },
                    onmousedown: {
                        move |_| {
                            if do_cursor_update {
                                *update_cursor.write() = true;
                            }
                        }
                    },
                    onkeydown: {
                        move |evt| {
                            // HACK(Linux): Allow copy and paste files for Linux
                            if cfg!(target_os = "linux") && evt.code() == Code::KeyV && evt.modifiers() == Modifiers::CONTROL {
                                if let Some(e) = on_paste_keydown {
                                    e.call(evt.clone());
                                }
                            }
                            // special codepath to handle onreturn
                            let old_enter_pressed = *enter_pressed.read();
                            let old_numpad_enter_pressed = *numpad_enter_pressed.read();
                            match evt.code() {
                                Code::ShiftLeft => if !*left_shift_pressed.read() { *left_shift_pressed.write() = true; },
                                Code::ShiftRight => if !*right_shift_pressed.read() { *right_shift_pressed.write() = true; },
                                Code::Enter => if !*enter_pressed.read() { *enter_pressed.write() = true; } ,
                                Code::NumpadEnter => if !*numpad_enter_pressed.read() { *numpad_enter_pressed.write() = true; },
                                _ => {}
                            };
                            // write_silent() doesn't update immediately. if the enter key is pressed, have to check the evt code
                            let enter_toggled = !old_enter_pressed && matches!(evt.code(), Code::Enter);
                            let numpad_enter_toggled = !old_numpad_enter_pressed && matches!(evt.code(), Code::NumpadEnter);
                            if (enter_toggled || numpad_enter_toggled) && !(*right_shift_pressed.read() || *left_shift_pressed.read())
                            {
                                onreturn.call((text_value.peek().clone(), true, evt.code()));
                            }

                            // special codepath to handle the arrow keys
                            let arrow = match evt.code() {
                                Code::ArrowDown|Code::ArrowUp => {
                                    if let Some(e) = onup_down_arrow {
                                        e.call(evt.code());
                                    };
                                    true
                                }
                                Code::ArrowLeft|Code::ArrowRight => {
                                    true
                                }
                                _ => {
                                    false
                                }
                            };
                            if do_cursor_update && arrow {
                                *update_cursor.write() = true;
                            }
                        }
                    }
                }
                if show_char_counter {
                        div {
                            class: "input-char-counter",
                            p {
                                key: "{sig_id}-char-counter",
                                id: "{sig_id}-char-counter",
                                aria_label: "input-char-counter",
                                class: "char-counter-p-element",
                                {format!("{}", text_value.peek().len())},
                            },
                            p {
                                key: "{sig_id}-char-max-length",
                                id: "{sig_id}-char-max-length",
                                class: "char-counter-p-element",
                               { format!("/{}", max_length - 1)},
                            }
                        }
                }
            },
        }
        script { {script} },
        script { {focus_script} }
    )
}

// Input using a rich editor making markdown changes visible
#[allow(non_snake_case)]
pub fn InputRich(props: Props) -> Element {
    log::trace!("render input");
    let Props {
        id,
        ignore_focus: _,
        loading,
        placeholder,
        max_length,
        size,
        aria_label,
        onchange,
        onreturn,
        oncursor_update,
        onkeyup,
        on_paste_keydown,
        value,
        is_disabled,
        show_char_counter,
        prevent_up_down_arrows,
        onup_down_arrow,
    } = props.clone();

    let mut sig_id = use_signal(|| {
        if id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            id.clone()
        }
    });
    let mut text_value = use_signal(|| value.clone());

    // If the id changed update the signal
    if !id.is_empty() && sig_id() != id {
        sig_id.set(id);
    }

    let script = include_str!("./script.js")
        .replace("$UUID", &sig_id())
        .replace("$MULTI_LINE", &format!("{}", true));
    let disabled = loading || is_disabled;

    // Sync changes to the editor
    use_effect(use_reactive(
        (&value, &placeholder, &disabled),
        move |(value, placeholder, disabled)| {
            let sync_script = include_str!("./sync_data.js").replace("$UUID", &sig_id());
            let update_text = !text_value.peek().eq(&value);
            let _ = eval(
                &sync_script
                    .clone()
                    .replace("$UPDATE", &update_text.to_string())
                    .replace(
                        "$TEXT",
                        &value
                            .replace('\\', "\\\\")
                            .replace('"', "\\\"")
                            .replace('\n', "\\n"),
                    )
                    .replace("$PLACEHOLDER", &placeholder)
                    .replace("$DISABLED", &disabled.to_string()),
            );
            if update_text {
                text_value.set(value);
            }
        },
    ));

    use_effect(move || {
        log::trace!("initializing markdown editor");
        let rich_editor: String = include_str!("./rich_editor_handler.js")
            .replace("$EDITOR_ID", &sig_id.peek())
            .replace("$AUTOFOCUS", &(!props.ignore_focus).to_string())
            .replace("$INIT", &value.replace('"', "\\\"").replace('\n', "\\n"));
        spawn(async move {
            let mut eval_result = eval(&rich_editor);
            loop {
                if let Ok(val) = eval_result.recv().await {
                    let input = INPUT_REGEX.captures(val.as_str().unwrap_or_default());
                    // Instead of escaping all needed chars just try extract the input string
                    let data = if let Some(capt) = input {
                        let txt = capt.get(1).map(|t| t.as_str()).unwrap_or_default();
                        Ok(JSTextData::Input(txt.to_string()))
                    } else {
                        serde_json::from_str::<JSTextData>(val.as_str().unwrap_or_default())
                    };
                    match data {
                        Ok(data) => match data {
                            JSTextData::Input(txt) => {
                                text_value.set(txt.clone());
                                onchange.call((txt, true))
                            }
                            JSTextData::Cursor(cursor) => {
                                if let Some(e) = oncursor_update {
                                    e.call((text_value.peek().clone(), cursor));
                                }
                            }
                            JSTextData::Submit => {
                                onreturn.call((text_value.peek().clone(), true, Code::Enter));
                            }
                            JSTextData::KeyPress(code) => {
                                if matches!(code, Code::ArrowDown | Code::ArrowUp) {
                                    if let Some(e) = onup_down_arrow {
                                        e.call(code);
                                    };
                                }
                            }
                            JSTextData::Init => {
                                let focus_script =
                                    include_str!("./focus.js").replace("$UUID", &sig_id.peek());
                                let _ = eval(&focus_script);
                            }
                        },
                        Err(e) => {
                            log::error!("failed to deserialize message: {}: {}", val, e);
                        }
                    }
                }
            }
        });
    });
    rsx! (
        div {
            id: "input-group-{sig_id}",
            class: "input-group",
            aria_label: "input-group",
            div {
                class: format_args!("input {}", if disabled { "disabled" } else { "" }),
                height: "{size.get_height()}",
                textarea {
                    key: "textarea-key-{sig_id}",
                    class: format_args!("{} {}", "input_textarea", if prevent_up_down_arrows {"up-down-disabled"} else {""}),
                    id: "{sig_id}",
                    aria_label: "{aria_label}",
                    disabled: "{disabled}",
                    maxlength: "{max_length}",
                    placeholder: format_args!("{}", if is_disabled {"".to_string()} else { placeholder }),
                    onblur: move |_| {
                        onreturn.call((text_value.peek().to_string(), false, Code::Enter));
                    },
                    onkeyup: move |evt| {
                        if let Some(e) = onkeyup {
                            e.call(evt.code());
                        }
                    },
                    onkeydown: move |evt| {
                        // Note for some reason arrow key events are not forwarded to here
                        // HACK(Linux): Allow copy and paste files for Linux
                        if cfg!(target_os = "linux") && evt.code() == Code::KeyV && evt.modifiers() == Modifiers::CONTROL {
                            if let Some(e) = on_paste_keydown {
                                e.call(evt.clone());
                            }
                        }
                    }
                }
                if show_char_counter {
                        div {
                            class: "input-char-counter",
                            p {
                                key: "{sig_id}-char-counter",
                                id: "{sig_id}-char-counter",
                                aria_label: "input-char-counter",
                                class: "char-counter-p-element",
                                {format!("{}", text_value.peek().chars().count())},
                            },
                            p {
                                key: "{sig_id}-char-max-length",
                                id: "{sig_id}-char-max-length",
                                class: "char-counter-p-element",
                                {format!("/{}", max_length - 1)},
                            }
                        }
                }
            },
        }
        script { script },
    )
}
