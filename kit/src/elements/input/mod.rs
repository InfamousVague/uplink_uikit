use std::collections::HashSet;

use common::language::{get_local_text, get_local_text_with_args};
use common::utils::lifecycle::use_component_lifecycle;
use dioxus::prelude::*;
use dioxus_html::input_data::keyboard_types::Code;
use uuid::Uuid;

pub type ValidationError = String;
use crate::elements::label::Label;
use crate::elements::loader::Loader;
use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;

use super::label::LabelWithEllipsis;

/// This vector of special chars must be used to decide which char can or cannot be allowed in the input field.
/// Just use this if quantity of chars you want to block and allow are similar.
/// If not, is best to use SpecialCharsAction to pass small vecs.
///
/// ## Example:
/// ```no_run
/// let chars_to_remove = vec!['\\', '/', ';', ':', '\'', '\"', ',', '<', '>', '.', '/', '?', '~', '_'];
/// let mut special_chars = SPECIAL_CHARS.to_vec();
/// special_chars = special_chars
///    .iter()
///    .filter(|&&c| !chars_to_remove.contains(&c))
///    .cloned()
///    .collect();
/// rsx! (
/// Input {
///  ...
/// options: Options {
///    with_validation: Some(Validation {
///        alpha_numeric_only: true,
///        special_chars_allowed: Some(special_chars),
///        ..Validation::default()
///    }),
///    ..Options::default()
/// }
/// ...
/// }
/// ```
pub static SPECIAL_CHARS: &[char] = &[
    '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '-', '+', '=', '{', '}', '[', ']', '|', '\\',
    ';', ':', '\'', '\"', ',', '<', '>', '.', '/', '?', '~', '_',
];

#[derive(PartialEq, Clone)]
pub enum SpecialCharsAction {
    Allow,
    Block,
}

#[derive(Default, Clone)]
pub struct Validation {
    pub max_length: Option<i32>,
    pub min_length: Option<i32>,
    pub alpha_numeric_only: bool,
    pub ignore_colons: bool,
    pub no_whitespace: bool,
    /// Decide if allow or block some chars, to keeping block any special char
    /// just pass None as value
    ///
    /// ### Example
    ///
    /// ```no_run
    ///  options: Options {
    ///        react_to_esc_key: true,
    ///     with_validation: Some(Validation {
    ///             alpha_numeric_only: true,
    ///             special_chars: Some((SpecialCharsAction::Block, vec!['\\', '/'])),
    ///             ..Validation::default()
    ///         }),
    ///         ..Options::default()
    ///     }
    /// ```
    pub special_chars: Option<(SpecialCharsAction, Vec<char>)>,
}

#[derive(Clone)]
pub struct Options {
    pub with_validation: Option<Validation>,
    pub replace_spaces_underscore: bool,
    pub disabled: bool,
    pub with_clear_btn: bool,
    pub clear_btn_icon: Icon,
    pub clear_on_submit: bool,
    pub with_label: Option<String>,
    pub ellipsis_on_label: Option<LabelWithEllipsis>,
    pub react_to_esc_key: bool,
    pub clear_validation_on_submit: bool,
    pub clear_validation_on_no_chars: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            with_validation: None,
            replace_spaces_underscore: false,
            disabled: false,
            with_clear_btn: false,
            clear_btn_icon: Icon::Backspace,
            clear_on_submit: true,
            with_label: None,
            ellipsis_on_label: None,
            react_to_esc_key: false,
            clear_validation_on_submit: false,
            clear_validation_on_no_chars: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Size {
    Small,
    Normal,
}

impl Size {
    fn get_height(&self) -> &str {
        match self {
            Size::Small => "0",
            _ => "",
        }
    }
}

#[derive(Props, Clone)]
pub struct Props {
    #[props(default = "".to_owned())]
    id: String,
    #[props(default = false)]
    focus: bool,
    focus_just_on_render: Option<bool>,
    loading: Option<bool>,
    placeholder: String,
    max_length: Option<i32>,
    #[props(default = Size::Normal)]
    size: Size,
    default_text: Option<String>,
    aria_label: Option<String>,
    is_password: Option<bool>,
    disabled: Option<bool>,
    #[props(optional)]
    icon: Option<Icon>,
    #[props(optional)]
    value: Option<String>,
    options: Option<Options>,
    select_on_focus: Option<bool>,
    onchange: Option<EventHandler<(String, bool)>>,
    onreturn: Option<EventHandler<(String, bool, Code)>>,
    onfocus: Option<EventHandler<()>>,
    reset: Option<Signal<bool>>,
    #[props(default = false)]
    disable_onblur: bool,
    #[props(default = false)]
    validate_on_return_with_val_empty: bool,
}

impl PartialEq for Props {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.focus == other.focus
            && self.loading == other.loading
            && self.placeholder == other.placeholder
            && self.max_length == other.max_length
            && self.size == other.size
            && self.default_text == other.default_text
            && self.aria_label == other.aria_label
            && self.is_password == other.is_password
            && self.disabled == other.disabled
            && self.icon == other.icon
            && self.value == other.value
            && self.select_on_focus == other.select_on_focus
            && self.onchange == other.onchange
            && self.onreturn == other.onreturn
            && self.onfocus == other.onfocus
            && self.reset == other.reset
            && self.disable_onblur == other.disable_onblur
            && self.validate_on_return_with_val_empty == other.validate_on_return_with_val_empty
    }
}

fn emit(props: Props, s: String, is_valid: bool) {
    if let Some(f) = &props.onchange {
        f.call((s, is_valid));
    }
}

fn emit_return(props: Props, s: String, is_valid: bool, key_code: Code) {
    if let Some(f) = &props.onreturn {
        f.call((s, is_valid, key_code));
    }
}

// warning: this function wasn't used so I'm assuming it will only be called if the input is validated.
#[allow(unused)]
fn submit(props: Props, s: String) {
    if let Some(f) = &props.onreturn {
        f.call((s, true, Code::Enter));
    }
}

fn validate_no_whitespace(val: &str) -> Option<ValidationError> {
    if val.contains(char::is_whitespace) {
        return Some(get_local_text("warning-messages.spaces-not-allowed"));
    }
    None
}

// Default to requiring alpha-numeric inputs, unless ignore_colon override is set on the input field
fn validate_alphanumeric(
    val: &str,
    ignore_colon: bool,
    special_characters: Option<(SpecialCharsAction, Vec<char>)>,
) -> Option<ValidationError> {
    let mut val = val.to_string();
    if ignore_colon {
        val.retain(|c| c != ':');
    }

    if let Some((action, chars)) = special_characters {
        let mut special_chars_allowed = SPECIAL_CHARS.to_vec();
        if action == SpecialCharsAction::Block {
            special_chars_allowed = special_chars_allowed
                .iter()
                .filter(|&&c| !chars.contains(&c))
                .cloned()
                .collect();
        } else {
            special_chars_allowed = chars;
        }
        for s in special_chars_allowed {
            val.retain(|c| c != s);
        }
    }

    if !val.chars().all(char::is_alphanumeric) {
        let invalid_chars = val.chars().filter(|x| !char::is_alphanumeric(*x));
        let mut s: HashSet<char> = HashSet::new();
        let mut t = String::new();
        for x in invalid_chars {
            if s.insert(x) {
                t.push(x);
            }
        }
        return Some(get_local_text_with_args(
            "warning-messages.disallowed-characters",
            vec![("chars", t)],
        ));
    }

    None
}

pub fn validate_min_max(val: &str, min: Option<i32>, max: Option<i32>) -> Option<ValidationError> {
    let max = max.unwrap_or_default() as usize;
    let min = min.unwrap_or_default() as usize;

    // Ensure the maximum value isn't the default
    // then make sure the value's length is less than or equal to the max
    if max > 0 && val.len() > max {
        return Some(get_local_text_with_args(
            "warning-messages.maximum-of",
            vec![("num", max)],
        ));
    }

    // Ensure the minimum is not the default value
    // then make sure the value's length is greater than or equal to the minimum
    if min > 0 && val.len() < min {
        return Some(if min > 1 {
            get_local_text_with_args("warning-messages.please-enter-at-least", vec![("num", min)])
        } else {
            get_local_text("warning-messages.please-enter-at-least-one")
        });
    }

    None
}

pub fn get_icon(props: Props) -> Icon {
    props.icon.unwrap_or(Icon::QuestionMarkCircle)
}

pub fn get_aria_label(props: Props) -> String {
    props.aria_label.clone().unwrap_or_default()
}

pub fn get_label(props: Props) -> String {
    let options = props.options.clone().unwrap_or_default();
    options.with_label.unwrap_or_default()
}

pub fn validate(props: Props, val: &str) -> Option<ValidationError> {
    let mut error: Option<ValidationError> = None;

    let options = props.options.clone().unwrap_or_default();

    let validation = options.with_validation.unwrap_or_default();

    if validation.alpha_numeric_only
        && validate_alphanumeric(
            val,
            validation.ignore_colons,
            validation.special_chars.clone(),
        )
        .is_some()
    {
        error = validate_alphanumeric(val, validation.ignore_colons, validation.special_chars);
    }

    if validation.no_whitespace && validate_no_whitespace(val).is_some() {
        error = validate_no_whitespace(val);
    }

    if (validation.max_length.is_some() || validation.min_length.is_some())
        && validate_min_max(val, validation.min_length, validation.max_length).is_some()
    {
        error = validate_min_max(val, validation.min_length, validation.max_length);
    }

    error
}

#[allow(non_snake_case)]
pub fn Input(props: Props) -> Element {
    let props_signal = use_signal(|| props.clone());
    // Input element needs an id. Create a new one if an id wasn't specified
    let input_id = if props.id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        props.id.clone()
    };
    let focus_script = include_str!("./script.js").replace("$UUID", &input_id);
    let focus_script2 = focus_script.clone();
    let mut error = use_signal(|| String::from(""));
    let mut val = use_signal(|| props.default_text.clone().unwrap_or_default());
    let max_length = props.max_length.unwrap_or(std::i32::MAX);
    let min_length = props.max_length.unwrap_or(0);
    let options = props.options.clone().unwrap_or_default();
    let should_validate = options.with_validation.is_some();
    let mut valid = use_signal(|| false);
    let onblur_active = !props.disable_onblur;

    let loading_class = match props.loading.unwrap_or(false) {
        true => "progress",
        false => "",
    };
    if let Some(value) = &props.value {
        if value.clone() != *val.read() {
            val.set(value.clone());
        }
    }

    let mut reset_fn = move || {
        *val.write() = "".into();
        error.set("".into());
        valid.set(false);
    };
    if let Some(mut hook) = &props.reset {
        let should_reset = *hook.read();
        if should_reset {
            reset_fn();
            hook.set(false);
        }
    }

    let apply_validation_class = should_validate;
    let aria_label = get_aria_label(props_signal.read().clone());
    let label = get_label(props_signal.read().clone());

    let disabled = props.disabled.unwrap_or_default() || props.loading.unwrap_or(false);

    let typ = props
        .is_password
        .and_then(|b| b.then_some("password"))
        .unwrap_or("text");

    let focus_signal = use_signal(|| props.focus);
    let focus_script_signal = use_signal(|| focus_script.clone());

    use_effect(move || {
        if *focus_signal.read() {
            let _ = eval(&focus_script_signal.read());
        }
    });

    let focus_just_on_render = props.focus_just_on_render.unwrap_or_default();
    use_component_lifecycle(
        move || {
            if focus_just_on_render {
                let _ = eval(&focus_script2);
            }
        },
        || {},
    );

    rsx! (
        div {
            class: {
                format_args!("input-group {}", if disabled { "disabled" } else { " "})
            },
            {(!label.is_empty()).then(|| rsx! (
                Label {
                    text: label,
                    label_with_ellipsis: options.ellipsis_on_label.unwrap_or_default(),
                }
            ))}
            div {
                class: {
                    format_args!("input {}", if *valid.read() && apply_validation_class { "input-success" } else if !error.read().is_empty() && apply_validation_class { "input-warning" } else { "" })
                },
                height: props_signal.read().clone().size.get_height(),
                // If an icon was provided, render it before the input.
                {(props_signal.read().clone().icon.is_some()).then(|| rsx!(
                    span {
                        class: "icon",
                        IconElement {
                            icon: get_icon(props_signal.read().clone())
                        }
                    }
                ))},
                input {
                    id: "{input_id}",
                    class: format_args!("{} {}", loading_class, if props.select_on_focus.unwrap_or_default() {"select"} else {""}),
                    aria_label: "{aria_label}",
                    spellcheck: "{false}",
                    disabled: "{disabled}",
                    value: "{val.read()}",
                    maxlength: "{max_length}",
                    "type": "{typ}",
                    placeholder: "{props.placeholder}",
                    placeholder: "{props.placeholder}",
                    onfocus: move |_| {
                        if let Some(e) = &props_signal.read().clone().onfocus {
                            e.call(())
                        }
                    },
                    onblur: move |_| {
                        if onblur_active {
                            emit_return(props_signal(), val.peek().to_string(), *valid.read(), Code::Enter);
                            if options.clear_on_submit {
                                reset_fn();
                            } else if options.clear_validation_on_submit {
                                valid.set(false);
                            }
                        }
                    },
                    oninput: move |evt| {
                        let current_val = evt.value().clone();

                        *val.write_silent() = current_val.clone();

                        let is_valid = if should_validate {
                            let validation_result = validate(props_signal(), &current_val).unwrap_or_default();
                            valid.set(validation_result.is_empty());
                            error.set(validation_result);
                            evt.stop_propagation();
                            *valid.read()
                        } else {
                            true
                        };
                        emit(props_signal(), current_val, is_valid);
                    },
                    // after a valid submission, don't keep the input box green.
                    onkeyup: move |evt| {
                        if val.read().to_string().is_empty() && options.clear_validation_on_no_chars {
                            reset_fn();
                        }

                        if evt.code() == Code::Enter || evt.code() == Code::NumpadEnter {
                            if props.validate_on_return_with_val_empty && val.read().to_string().is_empty() {
                                let is_valid = if should_validate {
                                    let validation_result = validate(props_signal(), "").unwrap_or_default();
                                    valid.set(validation_result.is_empty());
                                    error.set(validation_result);
                                    *valid.read()
                                } else {
                                    true
                                };
                                emit(props_signal(), "".to_owned(), is_valid);
                            } else {
                            emit_return(props_signal(), val.read().to_string(), *valid.read(), evt.code());
                            if options.clear_on_submit {
                                reset_fn();
                            } else if options.clear_validation_on_submit {
                                valid.set(false);
                            }
                        }
                        } else if options.react_to_esc_key && evt.code() == Code::Escape {
                            emit_return(props_signal(), "".to_owned(), min_length == 0, evt.code());
                            if options.clear_on_submit {
                                reset_fn();
                           }
                        }
                    }
                },
                {(options.with_clear_btn && !val.read().is_empty() && !disabled).then(move || rsx!(
                    div {
                        class: "clear-btn",
                        onclick: move |_| {
                            *val.write_silent() = String::new();
                            if should_validate {
                                let validation_result = validate(props_signal(), "").unwrap_or_default();
                                valid.set(validation_result.is_empty());
                                error.set(validation_result);
                            }

                            if options.clear_validation_on_no_chars {
                                reset_fn();
                            }
                            // re-focus the input after clearing it
                            let _ = eval(&focus_script);
                            emit(props_signal(), String::new(), *valid.read());
                        },
                        IconElement {
                            icon: options.clear_btn_icon
                        }
                    }
                ))},
                {props_signal().loading.unwrap_or(false).then(move || rsx!(
                    Loader { spinning: true },
                ))},
            },
            {(!error.read().is_empty()).then(|| rsx!(
                p {
                    class: "error",
                    aria_label: "input-error",
                    "{error}"
                }
            ))}
        }
    )
}
