use common::state::{Identity, State};
use dioxus::prelude::*;
use dioxus_elements::input_data::keyboard_types::Code;
use uuid::Uuid;
use warp::constellation::file::File;

use crate::{
    components::{
        embeds::file_embed::FileEmbed, message::format_text, message_typing::MessageTyping,
        user_image::UserImage,
    },
    elements::{button::Button, label::Label, textarea, Appearance},
};

use common::{icons, language::get_local_text, warp_runner::thumbnail_to_base64};
pub type To = &'static str;

pub enum SuggestionType {
    None,
    // Emoji suggestions. First is the string that was matched. Second is the emojis matched
    Emoji(String, Vec<(String, String)>),
    // Username tag suggestions. First is the string that was matched. Second is the users that matched
    Tag(String, Vec<Identity>),
}

impl SuggestionType {
    fn get_replacement_for_index(&self, index: usize) -> (String, String) {
        match self {
            SuggestionType::None => (String::new(), String::new()),
            SuggestionType::Emoji(pattern, v) => (pattern.clone(), v[index].0.clone()),
            SuggestionType::Tag(pattern, v) => (
                pattern.clone(),
                format!("{}#{}", v[index].username(), v[index].short_id()),
            ),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            SuggestionType::None => true,
            SuggestionType::Emoji(_, v) => v.is_empty(),
            SuggestionType::Tag(_, v) => v.is_empty(),
        }
    }
}
#[derive(Clone, PartialEq)]
pub struct Route {
    pub to: To,
    pub icon: icons::outline::Shape,
    pub name: &'static str,
}

#[derive(Default)]
pub struct ReplyInfo<'a> {
    pub user_image: Element<'a>,
    pub message: String,
}

#[derive(Props)]
pub struct Props<'a> {
    id: String,
    placeholder: String,
    typing_users: Vec<String>,
    with_replying_to: Option<Element<'a>>,
    with_file_upload: Option<Element<'a>>,
    extensions: Option<Element<'a>>,
    controls: Option<Element<'a>>,
    value: Option<String>,
    loading: Option<bool>,
    onchange: EventHandler<'a, String>,
    on_paste_keydown: Option<EventHandler<'a, Event<KeyboardData>>>,
    onreturn: EventHandler<'a, String>,
    #[props(default = false)]
    is_disabled: bool,
    ignore_focus: bool,
    suggestions: &'a SuggestionType,
    oncursor_update: Option<EventHandler<'a, (String, i64)>>,
    on_suggestion_click: Option<EventHandler<'a, (String, String, i64)>>,
    onup_down_arrow: Option<EventHandler<'a, Code>>,
}

#[derive(Props)]
pub struct ReplyProps<'a> {
    label: String,
    remote: Option<bool>,
    message: String,
    attachments: Option<Vec<File>>,
    onclose: EventHandler<'a>,
    children: Element<'a>,
    markdown: Option<bool>,
    transform_ascii_emojis: Option<bool>,
    state: &'a UseSharedState<State>,
    chat: Uuid,
}

#[allow(non_snake_case)]
pub fn Reply<'a>(cx: Scope<'a, ReplyProps<'a>>) -> Element<'a> {
    let remote = cx.props.remote.unwrap_or_default();
    let message = format_text(
        &cx.props.message,
        cx.props.markdown.unwrap_or_default(),
        cx.props.transform_ascii_emojis.unwrap_or_default(),
        Some((&cx.props.state.read(), &cx.props.chat, true)),
    );

    let has_attachments = cx
        .props
        .attachments
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    let attachment_list = cx.props.attachments.as_ref().map(|vec| {
        vec.iter().map(|file| {
            let key = file.id();
            rsx!(FileEmbed {
                key: "{key}",
                filename: file.name(),
                filesize: file.size(),
                thumbnail: thumbnail_to_base64(file),
                with_download_button: false,
                remote: remote,
                on_press: move |_| {},
            })
        })
    });

    cx.render(rsx! (
        div {
            class: "inline-reply",
            aria_label: "inline-reply",
            Label {
                text: cx.props.label.clone(),
                aria_label: "inline-reply-header".into(),
            },
            Button {
                small: true,
                aria_label: "close-reply".into(),
                appearance: Appearance::Secondary,
                icon: icons::outline::Shape::XMark,
                onpress: move |_| cx.props.onclose.call(()),
            },
            div {
                class: "content",
                aria_label: "content",
                remote.then(|| rsx!(&cx.props.children)),
                p {
                    class: {
                        format_args!("reply-text message {}", if remote { "remote" } else { "" })
                    },
                    aria_label: {
                        format_args!("reply-text-message{}", if remote { "-remote" } else { "" })
                    },
                    dangerous_inner_html: "{message}",
                    has_attachments.then(|| {
                        rsx!(
                            attachment_list.map(|list| {
                                rsx!( list )
                            })
                        )
                    })
                }
                (!remote).then(|| rsx!(&cx.props.children)),
            }

        }
    ))
}

#[allow(non_snake_case)]
pub fn Chatbar<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let controlled_input_id = &cx.props.id;
    let is_typing = !cx.props.typing_users.is_empty();
    let cursor_position = use_ref(cx, || None);
    let selected_suggestion: &UseRef<Option<usize>> = use_ref(cx, || None);
    let arrow_selected = use_ref(cx, || false);
    let is_suggestion_modal_closed: &UseRef<bool> = use_ref(cx, || false);
    let has_value = cx
        .props
        .value
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or_default();
    let eval = use_eval(cx);

    cx.render(rsx!(
        div {
            class: "chatbar disable-select",
            cx.props.with_replying_to.as_ref(),
            cx.props.with_file_upload.as_ref(),
            div{
                class: "chatbar-group",
                textarea::InputRich {
                    key: "{controlled_input_id}",
                    id: controlled_input_id.clone(),
                    loading: cx.props.loading.unwrap_or_default(),
                    placeholder: cx.props.placeholder.clone(),
                    ignore_focus: cx.props.ignore_focus,
                    show_char_counter: true,
                    value: if cx.props.is_disabled { get_local_text("messages.loading")} else { cx.props.value.clone().unwrap_or_default()},
                    onkeyup: move |keycode| {
                        if !*is_suggestion_modal_closed.read() && (keycode == Code::Escape || keycode == Code::Tab) {
                            if keycode == Code::Tab {
                                if let Some(i) = selected_suggestion.write_silent().take() {
                                    if let Some(e) = cx.props.on_suggestion_click.as_ref() {
                                        if let Some(p) = cursor_position.read().as_ref() {
                                            let (pattern, replacement) = cx.props.suggestions.get_replacement_for_index(i);
                                            e.call((replacement, pattern,*p));
                                        }
                                    }
                                }
                            }
                            is_suggestion_modal_closed.with_mut(|i| *i = true);
                        }
                    },
                    on_paste_keydown:  move |keyboard_event: Event<KeyboardData>| {
                        if let Some(e) = cx.props.on_paste_keydown.as_ref() {
                            e.call(keyboard_event);
                        }
                    },
                    onchange: move |(v, _)| {
                        cx.props.onchange.call(v);
                        *is_suggestion_modal_closed.write_silent() = false;
                    },
                    onreturn: move |(v, is_valid, _)| {
                        if let Some(i) = selected_suggestion.write_silent().take() {
                            if let Some(e) = cx.props.on_suggestion_click.as_ref() {
                                if let Some(p) = cursor_position.read().as_ref() {
                                    let (pattern, replacement) = cx.props.suggestions.get_replacement_for_index(i);
                                    e.call((replacement, pattern,*p));
                                    return;
                                }
                            }
                        }
                        if is_valid {
                            cx.props.onreturn.call(v);
                        }
                    },
                    oncursor_update: move |(v,p)| {
                        if let Some(e) = cx.props.oncursor_update.as_ref() {
                            e.call((v,p))
                        }
                        *cursor_position.write_silent() = Some(p)
                    },
                    is_disabled: cx.props.is_disabled,
                    prevent_up_down_arrows: !cx.props.suggestions.is_empty() || !has_value,
                    onup_down_arrow:
                        move |code| {
                            let amount = match cx.props.suggestions {
                                SuggestionType::None => 0,
                                SuggestionType::Emoji(_, v) => v.len(),
                                SuggestionType::Tag(_, v) => v.len(),
                            };
                            if amount == 0 {
                                *selected_suggestion.write_silent() = None;
                                if let SuggestionType::None = cx.props.suggestions {
                                    if let Some(e) = &cx.props.onup_down_arrow {
                                        e.call(code);
                                    }
                                }
                                return;
                            }
                            let current = &mut *selected_suggestion.write();
                            let selected_idx = if code == Code::ArrowDown {
                                match current.as_ref() {
                                    Some(v) => (v + 1) % amount,
                                    None => 0,
                                }
                            } else {
                                match current.as_ref() {
                                    Some(v) => (v + amount - 1) % amount,
                                    None => amount - 1,
                                }
                            };
                            *current = Some(selected_idx);
                            *arrow_selected.write() = true;
                            let _ = eval(&include_str!("./suggestion_scroll.js").replace("$NUM", &selected_idx.to_string()));
                        }
                },
                is_typing.then(|| {
                    rsx!(MessageTyping {
                        typing_users: cx.props.typing_users.clone()
                    })
                })
            }
            cx.props.extensions.as_ref(),
            div {
                class: "controls",
                cx.props.controls.as_ref()
            },
            (!cx.props.suggestions.is_empty() && !*is_suggestion_modal_closed.read()).then(||
                rsx!(SuggestionsMenu {
                suggestions: cx.props.suggestions,
                on_close: move |_| {
                    is_suggestion_modal_closed.with_mut(|i| *i = true);
                    *selected_suggestion.write() = None;
                },
                on_click: move |(emoji, pattern)| {
                    if let Some(e) = cx.props.on_suggestion_click.as_ref() {
                        if let Some(p) = cursor_position.read().as_ref() {
                            e.call((emoji, pattern, *p))
                        }
                    }
                },
                selected: selected_suggestion.clone(),
                arrow_selected: arrow_selected.clone(),
            })),
        }
    ))
}

#[derive(Props)]
pub struct SuggestionProps<'a> {
    suggestions: &'a SuggestionType,
    on_click: EventHandler<'a, (String, String)>,
    on_close: EventHandler<'a, ()>,
    selected: UseRef<Option<usize>>,
    arrow_selected: UseRef<bool>,
}

#[allow(non_snake_case)]
fn SuggestionsMenu<'a>(cx: Scope<'a, SuggestionProps<'a>>) -> Element<'a> {
    if cx.props.selected.read().is_none() {
        *cx.props.selected.write_silent() = Some(0);
    }

    let (label, suggestions): (_, Vec<_>) = match cx.props.suggestions {
        SuggestionType::None => return cx.render(rsx!(())),
        SuggestionType::Emoji(pattern, emojis) => {
            let component = emojis.iter().enumerate().map(|(num, (emoji,alias))| {
                rsx!(div {
                    class: format_args!("{} {}", "chatbar-suggestion", match cx.props.selected.read().as_ref() {
                        Some(v) => if *v == num {"chatbar-selected"} else {""},
                        None => "",
                    }),
                    aria_label: {
                        format_args!(
                            "emoji-suggested-{emoji}",
                        )
                    },
                    onclick: move |_| {
                        cx.props.on_click.call((emoji.clone(), pattern.clone()))
                    },
                    onmouseover: move |_| {
                        cx.props.arrow_selected.with_mut(|arrow|{
                            if !*arrow {
                                *cx.props.selected.write() = Some(num);
                            }
                            *arrow = false
                        });
                    },
                    format_args!("{emoji}  :{alias}:"),
                })
            }).collect();
            (get_local_text("messages.emoji-suggestion"), component)
        }
        SuggestionType::Tag(pattern, identities) => {
            let component = identities.iter().enumerate().map(|(num, id)| {
                let username = format!("{}#{}", id.username(), id.short_id());
                rsx!(div {
                    class: format_args!("{} {}", "chatbar-suggestion", match cx.props.selected.read().as_ref() {
                        Some(v) => if *v == num {"chatbar-selected"} else {""},
                        None => ""
                    }),
                    aria_label: {
                        format_args!(
                            "username-suggested-{username}",
                        )
                    },
                    onclick: move |_| {
                        cx.props.on_click.call((username.clone(), pattern.clone()))
                    },
                    onmouseover: move |_| {
                        cx.props.arrow_selected.with_mut(|arrow|{
                            if !*arrow {
                                *cx.props.selected.write() = Some(num);
                            }
                            *arrow = false
                        });
                    },
                    div {
                        class: "user-suggestion-profile",
                        UserImage {
                            platform: id.platform().into(),
                            status: id.identity_status().into(),
                            image: id.profile_picture()
                        }
                    }
                    format_args!("{username}"),
                })
            }).collect();
            (get_local_text("messages.username-suggestion"), component)
        }
    };
    cx.render(rsx!(div {
        id: "chatbar-suggestions",
        aria_label: "chatbar-suggestions-container",
        onmouseenter: move |_| {
            *cx.props.arrow_selected.write() = false;
        },
        div {
            class: "chatbar-suggestions-header",
            Label {
                text: label,
            },
            Button {
                small: true,
                aria_label: "chatbar-suggestion-close-button".into(),
                appearance: Appearance::Secondary,
                icon: icons::outline::Shape::XMark,
                onpress: move |_| cx.props.on_close.call(()),
            },
        }
        div {
            class: "chatbar-suggestion-list",
            suggestions.into_iter()
        }
    }))
}
