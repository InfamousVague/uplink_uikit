use dioxus::prelude::*;
use dioxus_elements::input_data::keyboard_types::Code;

use crate::elements::input::{Input, Options, Size, SpecialCharsAction, Validation};

use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    #[props(optional)]
    open: Option<bool>,
    #[props(optional)]
    text: Option<String>,
    #[props(optional)]
    aria_label: Option<String>,
    #[props(optional)]
    disabled: Option<bool>,
    #[props(optional)]
    with_rename: Option<bool>,
    #[props(optional)]
    onrename: Option<EventHandler<(String, Code)>>,
    #[props(optional)]
    onpress: Option<EventHandler>,
    #[props(optional)]
    loading: Option<bool>,
}

pub fn get_aria_label(props: Props) -> String {
    props.aria_label.clone().unwrap_or_default()
}

pub fn emit(props: Props, s: String, key_code: Code) {
    if let Some(f) = props.onrename.as_ref() {
        f.call((s, key_code))
    }
}

pub fn emit_press(props: Props) {
    if let Some(f) = props.onpress.as_ref() {
        f.call(())
    }
}

#[allow(non_snake_case)]
pub fn Folder(props: Props) -> Element {
    let open = props.open.unwrap_or_default();
    let folder_name = props.text.clone().unwrap_or_default();
    let aria_label = get_aria_label(props.clone());
    let placeholder = folder_name.clone();
    let with_rename = props.with_rename.unwrap_or_default();
    let icon = if open { Icon::FolderOpen } else { Icon::Folder };
    let disabled = props.disabled.unwrap_or_default();

    let loading = props.loading.unwrap_or_default();

    let props_signal = use_signal(|| props.clone());

    if loading {
        rsx!(FolderSkeletal {})
    } else {
        rsx!(
            div {
                class: {
                    format_args!("folder {}", if disabled { "disabled" } else { "" })
                },
                aria_label: "{aria_label}",
                div {
                    class: "icon alignment",
                    onclick: move |_| emit_press(props_signal.read().clone()),
                    IconElement {
                        icon: icon,
                    },
                },
                {with_rename.then(||
                        rsx! (
                            Input {
                                aria_label: "folder-name-input".to_string(),
                                disabled: disabled,
                                placeholder: String::new(),
                                default_text: placeholder,
                                select_on_focus: true,
                                focus: true,
                                size: Size::Small,
                                validate_on_return_with_val_empty: true,
                                options: Options {
                                    react_to_esc_key: true,
                                    with_validation: Some(Validation {
                                        alpha_numeric_only: true,
                                        special_chars: Some((SpecialCharsAction::Block, vec!['\\', '/'])),
                                        min_length: Some(1),
                                        max_length: Some(64),
                                        ..Validation::default()
                                    }),
                                    ..Options::default()
                                },
                                onreturn: move |(s, is_valid, key_code)| {
                                    if is_valid || key_code == Code::Escape {
                                        emit(props_signal.read().clone(), s, key_code);
                                    }
                                }
                            }
                    )
                  )},
                {(!with_rename).then(|| rsx! (
                    label {
                        class: "folder-name item-alignment",
                        title: "{&folder_name}",
                        "{folder_name}"
                    }
                ))}
            }
        )
    }
}

#[allow(non_snake_case)]
pub fn FolderSkeletal() -> Element {
    rsx!(
        div {
            class: "folder",
            div {
                class: "icon skeletal-svg",
                IconElement {
                    icon: Icon::FolderArrowDown,
                },
            },
            div {
                class: "skeletal skeletal-bar"
            }
        }
    )
}
