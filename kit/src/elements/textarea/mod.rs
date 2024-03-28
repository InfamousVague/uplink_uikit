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
    let left_shift_pressed = use_signal(|| false);
    let right_shift_pressed = use_signal(|| false);
    let enter_pressed = use_signal(|| false);
    let numpad_enter_pressed = use_signal(|| false);
    let cursor_position = use_signal(|| None);

    let Props {
        id: _,
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

    let id = if props.id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        props.id.clone()
    };
    let id2 = id.clone();
    let id_char_counter = id.clone();
    let focus_script = if props.ignore_focus {
        String::new()
    } else {
        include_str!("./focus.js").replace("$UUID", &id)
    };

    let _ = eval(&focus_script);

    let script = include_str!("./script.js")
        .replace("$UUID", &id)
        .replace("$MULTI_LINE", &format!("{}", true));
    let disabled = loading || is_disabled;

    let sync = use_signal(|| include_str!("./sync_data.js").replace("$UUID", &id));
    let clear_counter_script =
        r#"document.getElementById('$UUID-char-counter').innerText = "0";"#.replace("$UUID", &id);

    let cursor_script = include_str!("./cursor_script.js").replace("$ID", &id2);

    let text_value = use_signal(|| value.clone());
    let value_signal = use_signal(|| value.clone());

    let _ = use_resource(move || {
        to_owned![cursor_position, show_char_counter];
        async move {
            *cursor_position.write_silent() = Some(value_signal.read().chars().count() as i64);
            *text_value.write_silent() = text_value.read().clone();
            if show_char_counter {
                let _ = eval(&sync().replace("$TEXT", &text_value.read()));
            }
        }
    });

    let do_cursor_update = oncursor_update.is_some();

    if let Some(val) = cursor_position.write_silent().take() {
        if let Some(e) = oncursor_update {
            e.call((text_value.read().clone(), val));
        }
    }

    let placeholder = use_signal(|| placeholder.clone());
    let placeholder_clone = placeholder;

    rsx! (
        div {
            id: "input-group-{id}",
            class: "input-group",
            aria_label: "input-group",
            div {
                class: format_args!("input {}", if disabled { "disabled" } else { "" }),
                height: "{size.get_height()}",
                textarea {
                    key: "textarea-key-{id}",
                    class: format_args!("{} {}", "input_textarea", if prevent_up_down_arrows {"up-down-disabled"} else {""}),
                    id: "{id}",
                    aria_label: "{aria_label}",
                    disabled: "{disabled}",
                    value: "{text_value.read()}",
                    maxlength: "{max_length}",
                    placeholder: format_args!("{}", if is_disabled {"".to_string()} else {placeholder_clone()}),
                    onblur: move |_| {
                        onreturn.call((text_value.read().to_string(), false, Code::Enter));
                    },
                    oninput: {
                        to_owned![cursor_script];
                        move |evt| {
                            let current_val = evt.value().clone();
                            *text_value.write_silent() = current_val.clone();
                            onchange.call((current_val, true));
                            to_owned![cursor_script, cursor_position];
                            async move {
                                if do_cursor_update {
                                    let eval_result = eval(&cursor_script);
                                        if let Ok(val) = eval_result.join().await {
                                            *cursor_position.write() = Some(val.as_i64().unwrap_or_default());
                                        }
                                }
                            }
                        }
                    },
                    onkeyup: move |evt| {
                        match evt.code() {
                            Code::ShiftLeft => *left_shift_pressed.write_silent() = false,
                            Code::ShiftRight => *right_shift_pressed.write_silent() = false,
                            Code::Enter => *enter_pressed.write_silent() = false,
                            Code::NumpadEnter => *numpad_enter_pressed.write_silent() = false,
                            _ => {}
                        };
                        if let Some(e) = onkeyup {
                            e.call(evt.code());
                        }
                    },
                    onmousedown: {
                        to_owned![cursor_script];
                        move |_| {
                            to_owned![cursor_script, cursor_position];
                            async move {
                                if do_cursor_update {
                                    let eval_result = eval(&cursor_script);
                                        if let Ok(val) = eval_result.join().await {
                                            *cursor_position.write() = Some(val.as_i64().unwrap_or_default());
                                        }
                                }
                            }
                        }
                    },
                    onkeydown: {
                        to_owned![eval, cursor_script];
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
                                Code::ShiftLeft => if !*left_shift_pressed.read() { *left_shift_pressed.write_silent() = true; },
                                Code::ShiftRight => if !*right_shift_pressed.read() { *right_shift_pressed.write_silent() = true; },
                                Code::Enter => if !*enter_pressed.read() { *enter_pressed.write_silent() = true; } ,
                                Code::NumpadEnter => if !*numpad_enter_pressed.read() { *numpad_enter_pressed.write_silent() = true; },
                                _ => {}
                            };
                            // write_silent() doesn't update immediately. if the enter key is pressed, have to check the evt code
                            let enter_toggled = !old_enter_pressed && matches!(evt.code(), Code::Enter);
                            let numpad_enter_toggled = !old_numpad_enter_pressed && matches!(evt.code(), Code::NumpadEnter);
                            if (enter_toggled || numpad_enter_toggled) && !(*right_shift_pressed.read() || *left_shift_pressed.read())
                            {
                                 if show_char_counter {
                                        let _ = eval(&clear_counter_script);
                                    }
                                    onreturn.call((text_value.read().clone(), true, evt.code()));
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
                            to_owned![eval, cursor_script, cursor_position];
                            async move {
                                if do_cursor_update && arrow {
                                    let eval_result = eval(&cursor_script);
                                        if let Ok(val) = eval_result.join().await {
                                            *cursor_position.write() = Some(val.as_i64().unwrap_or_default());
                                        }
                                }
                            }
                        }
                    }
                }
                if show_char_counter {
                        div {
                            class: "input-char-counter",
                            p {
                                key: "{id_char_counter}-char-counter",
                                id: "{id_char_counter}-char-counter",
                                aria_label: "input-char-counter",
                                class: "char-counter-p-element",
                                {format!("{}", text_value.read().len())},
                            },
                            p {
                                key: "{id_char_counter}-char-max-length",
                                id: "{id_char_counter}-char-max-length",
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
    let listener_data = use_signal(|| None);

    let Props {
        id: _,
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

    let id = if props.id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        props.id.clone()
    };
    let id2 = id.clone();
    let id_char_counter = id.clone();

    let script = include_str!("./script.js")
        .replace("$UUID", &id)
        .replace("$MULTI_LINE", &format!("{}", true));
    let disabled = loading || is_disabled;

    let text_value = use_signal(|| value.clone());
    let sync_script = use_signal(|| include_str!("./sync_data.js").replace("$UUID", &id));

    let value = use_signal(|| value.clone());
    let placeholder = use_signal(|| placeholder.clone());

    // Sync changed to the editor
    let _ = use_resource(move || {
        to_owned![sync_script, text_value, disabled];
        async move {
            let update = !text_value.read().eq(&value());
            let _ = eval(
                &sync_script
                    .read()
                    .clone()
                    .replace("$UPDATE", &update.to_string())
                    .replace(
                        "$TEXT",
                        &value()
                            .replace('\\', "\\\\")
                            .replace('"', "\\\"")
                            .replace('\n', "\\n"),
                    )
                    .replace("$PLACEHOLDER", &placeholder())
                    .replace("$DISABLED", &disabled.to_string()),
            );
        }
    });

    // TODO(MIGRATION_0.5): Before it was use_effect, verify if it keeps the same behavior
    use_future(move || {
        to_owned![listener_data];
        let rich_editor: String = include_str!("./rich_editor_handler.js")
            .replace("$EDITOR_ID", &id2)
            .replace("$AUTOFOCUS", &(!props.ignore_focus).to_string())
            .replace("$INIT", &value().replace('"', "\\\"").replace('\n', "\\n"));
        async move {
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
                        Ok(data) => {
                            let new =
                                listener_data.with(
                                    |current: &Option<Vec<JSTextData>>| match current {
                                        Some(pending) => {
                                            let mut pending = pending.clone();
                                            pending.push(data);
                                            pending
                                        }
                                        None => vec![data],
                                    },
                                );
                            *listener_data.write() = Some(new)
                        }
                        Err(e) => {
                            log::error!("failed to deserialize message: {}: {}", val, e);
                        }
                    }
                }
            }
        }
    });

    if let Some(pending) = listener_data.peek().clone().take() {
        pending.iter().for_each(|val| match val.to_owned() {
            JSTextData::Input(txt) => {
                *text_value.write_silent() = txt.clone();
                onchange.call((txt, true))
            }
            JSTextData::Cursor(cursor) => {
                if let Some(e) = oncursor_update {
                    e.call((text_value.read().clone(), cursor));
                }
            }
            JSTextData::Submit => {
                onreturn.call((text_value.read().clone(), true, Code::Enter));
            }
            JSTextData::KeyPress(code) => {
                if matches!(code, Code::ArrowDown | Code::ArrowUp) {
                    if let Some(e) = onup_down_arrow {
                        e.call(code);
                    };
                }
            }
        });
    }

    rsx! (
        div {
            id: "input-group-{id}",
            class: "input-group",
            aria_label: "input-group",
            div {
                class: format_args!("input {}", if disabled { "disabled" } else { "" }),
                height: "{size.get_height()}",
                textarea {
                    key: "textarea-key-{id}",
                    class: format_args!("{} {}", "input_textarea", if prevent_up_down_arrows {"up-down-disabled"} else {""}),
                    id: "{id}",
                    aria_label: "{aria_label}",
                    disabled: "{disabled}",
                    maxlength: "{max_length}",
                    placeholder: format_args!("{}", if is_disabled {"".to_string()} else {placeholder()}),
                    onblur: move |_| {
                        onreturn.call((text_value.read().to_string(), false, Code::Enter));
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
                                key: "{id_char_counter}-char-counter",
                                id: "{id_char_counter}-char-counter",
                                aria_label: "input-char-counter",
                                class: "char-counter-p-element",
                                {format!("{}", text_value.read().chars().count())},
                            },
                            p {
                                key: "{id_char_counter}-char-max-length",
                                id: "{id_char_counter}-char-max-length",
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
