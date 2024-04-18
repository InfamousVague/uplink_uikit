use std::ffi::OsStr;

use common::return_correct_icon;
use dioxus::prelude::*;
use dioxus_elements::input_data::keyboard_types::Code;

use crate::elements::input::{Input, Options, Size, SpecialCharsAction, Validation};
use dioxus_html::input_data::keyboard_types::Modifiers;

use common::icons::Icon as IconElement;
use common::{icons::outline::Shape as Icon, is_video};

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    text: String,
    #[props(optional)]
    thumbnail: Option<String>,
    #[props(optional)]
    disabled: Option<bool>,
    #[props(optional)]
    aria_label: Option<String>,
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

pub fn get_file_extension(file_name: String) -> String {
    // don't append a '.' to a file name if it has no extension
    std::path::Path::new(&file_name)
        .extension()
        .and_then(OsStr::to_str)
        .map(|s| format!(".{s}"))
        .unwrap_or_default()
}

#[allow(non_snake_case)]
pub fn File(props: Props) -> Element {
    let props_signal = use_signal(|| props.clone());
    let file_extension = get_file_extension(props.text.clone());
    let file_name = props.text.clone();
    let file_name2 = file_name.clone();

    let aria_label = get_aria_label(props_signal.read().clone());
    let placeholder = file_name.clone();
    let with_rename = props.with_rename.unwrap_or_default();
    let disabled = props.disabled.unwrap_or_default();
    let thumbnail = props.thumbnail.clone().unwrap_or_default();
    let is_video = is_video(&props.text.clone());

    let loading = props.loading.unwrap_or_default();

    if loading {
        rsx!(FileSkeletal {})
    } else {
        rsx!(
            div {
                class: {
                    format_args!("file {}", if disabled { "disabled" } else { "" })
                },
                aria_label: "{aria_label}",
                onclick: move |mouse_event_data| {
                    if mouse_event_data.modifiers() != Modifiers::CONTROL {
                        emit_press(props_signal.read().clone());
                    }
                },
                div {
                    class: "icon alignment",
                    if thumbnail.is_empty() {
                            label {
                                class: "file-type",
                                {file_extension.clone().replace('.', "")}
                            }
                            div {
                                height: "80px",
                                width: "80px",
                                margin: "30px 0",
                                IconElement {
                                    icon: return_correct_icon(&file_name2.clone())
                                }
                            }
                    } else {
                        img {
                            class: "thumbnail-container",
                            height: if is_video {"50px"} else {""},
                            width: if is_video {"100px"} else {""},
                            src: "{thumbnail}",
                        }
                    }
                },
                {with_rename.then(||
                    rsx! (
                        div {
                            margin_top: "12px",
                        },
                        Input {
                                aria_label: "file-name-input".to_string(),
                                disabled: disabled,
                                placeholder: String::new(),
                                default_text: placeholder,
                                select_on_focus: true,
                                focus: true,
                                size: Size::Small,
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
                                // todo: use is_valid
                                onreturn: move |(s, is_valid, key_code)| {
                                    if is_valid || key_code == Code::Escape  {
                                        let new_name = format!("{}{}", s, file_extension);
                                        emit(props_signal.read().clone(), new_name, key_code)
                                    }
                                }
                            }
                        )
                  )},
                {(!with_rename).then(|| rsx! (
                    label {
                        class: "file-name item-alignment",
                        padding_top: "8px",
                        height: "24px",
                        title: "{&file_name}",
                        "{file_name}"
                    }
                ))}
            }
        )
    }
}

#[allow(non_snake_case)]
pub fn FileSkeletal() -> Element {
    rsx!(
        div {
            class: "file alignment",
            div {
                class: "icon skeletal-svg",
                IconElement {
                    icon: Icon::DocumentText,
                },
            },
            div {
                class: "skeletal skeletal-bar"
            }
        }
    )
}

#[cfg(test)]
mod test {
    pub use super::*;

    #[test]
    fn test_get_file_extension1() {
        let input = String::from("image.jpeg");
        let file_extension = get_file_extension(input);
        assert_eq!(file_extension, ".jpeg");
    }

    #[test]
    fn test_get_file_extension2() {
        let input = String::from("image.png");
        let file_extension = get_file_extension(input);
        assert_eq!(file_extension, ".png");
    }

    #[test]
    fn test_get_file_extension3() {
        let input = String::from("file.txt");
        let file_extension = get_file_extension(input);
        assert_eq!(file_extension, ".txt");
    }

    #[test]
    fn test_get_file_extension4() {
        let input = String::from("file.txt.exe");
        let file_extension = get_file_extension(input);
        assert_eq!(file_extension, ".exe");
    }

    #[test]
    fn test_get_file_extension5() {
        let input = String::from("file");
        let file_extension = get_file_extension(input);
        assert_eq!(file_extension, "");
    }
}
