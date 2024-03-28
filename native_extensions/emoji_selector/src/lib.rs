use common::language::get_local_text;
use common::warp_runner::{RayGunCmd, WarpCmd};
use common::{
    icons::outline::Shape as Icon,
    state::{scope_ids::ScopeIds, ui::EmojiDestination, Action, State},
};
use dioxus::prelude::*;
use emojis::{Group, UnicodeVersion};
use extensions::{export_extension, Details, Extension, Location, Meta, Type};
use futures::StreamExt;
use kit::components::invisible_closer::InvisibleCloser;
use kit::elements::textarea;
use kit::{
    components::nav::{Nav, Route},
    elements::{button::Button, label::Label},
};
use once_cell::sync::Lazy;
use regex::Regex;
use tracing::log;
use uuid::Uuid;
use warp::raygun::ReactionState;
// These two lines are all you need to use your Extension implementation as a shared library
static EXTENSION: Lazy<EmojiSelector> = Lazy::new(|| EmojiSelector {});
static EMOJI_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(":[^:]{2,}:?$").unwrap());
export_extension!(EXTENSION);

pub struct EmojiSelector;

fn group_to_str(group: emojis::Group) -> String {
    match group {
        Group::SmileysAndEmotion => "Smileys & Emotion".into(),
        Group::PeopleAndBody => "People & Body".into(),
        Group::AnimalsAndNature => "Animals & Nature".into(),
        Group::FoodAndDrink => "Food & Drink".into(),
        Group::TravelAndPlaces => "Travel & Places".into(),
        Group::Activities => "Activities".into(),
        Group::Objects => "Objects".into(),
        Group::Symbols => "Symbols".into(),
        Group::Flags => "Flags".into(),
    }
}

fn is_supported(unicode_version: UnicodeVersion) -> bool {
    let (major, minor, _) = std::char::UNICODE_VERSION;
    unicode_version.major() <= major as u32 && unicode_version.minor() <= minor as u32
}

#[component(no_case_check)]
fn build_nav() -> Element {
    let routes = vec![
        Route {
            to: "Smileys & Emotion",
            name: group_to_str(Group::SmileysAndEmotion),
            icon: Icon::FaceSmile,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
        Route {
            to: "People & Body",
            name: group_to_str(Group::PeopleAndBody),
            icon: Icon::Users,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
        Route {
            to: "Animals & Nature",
            name: group_to_str(Group::AnimalsAndNature),
            icon: Icon::Leaf,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
        Route {
            to: "Food & Drink",
            name: group_to_str(Group::FoodAndDrink),
            icon: Icon::Cake,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
        Route {
            to: "Travel & Places",
            name: group_to_str(Group::TravelAndPlaces),
            icon: Icon::BuildingStorefront,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
        Route {
            to: "Activities",
            name: group_to_str(Group::Activities),
            icon: Icon::Basketball,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
        Route {
            to: "Objects",
            name: group_to_str(Group::Objects),
            icon: Icon::Clock,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
        Route {
            to: "Symbols",
            name: group_to_str(Group::Symbols),
            icon: Icon::CpuChip,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
        Route {
            to: "Flags",
            name: group_to_str(Group::Flags),
            icon: Icon::Flag,
            with_badge: None,
            loading: None,
            ..Route::default()
        },
    ];

    let scroll_script = r#"
        var emoji_scrolling_element = document.getElementById('scrolling');
        const emoji_group_element = document.getElementById('$EMOJI_CONTAINER');
        emoji_group_element.scrollIntoView({ behavior: 'smooth', block: 'start' });
    "#;

    rsx!(Nav {
        routes: routes.clone(),
        active: routes[0].to,
        onnavigate: move |r| {
            let scroll_script = scroll_script.to_string().replace("$EMOJI_CONTAINER", r);
            let _ = eval(&scroll_script);
        }
    })
}

#[derive(Debug)]
enum Command {
    React(Uuid, Uuid, String),
}

#[component(no_case_check)]
fn render_selector(mouse_over_emoji_button: Signal<bool>, nav: Element) -> Element {
    let mut state = use_context::<Signal<State>>();
    let mouse_over_emoji_selector = use_signal(|| false);
    let mut emoji_suggestions = use_signal(Vec::new);

    let focus_script = r#"
            var emoji_selector = document.getElementById('emoji_selector');
            emoji_selector.focus();
        "#;

    let focus_script_signal = use_signal(|| focus_script.to_string());

    let ch = use_coroutine(|mut rx: UnboundedReceiver<Command>| {
        to_owned![state];
        async move {
            let warp_cmd_tx = state.read().get_warp_ch();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    Command::React(conversation_id, message_id, emoji) => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::React {
                            conversation_id,
                            message_id,
                            reaction_state: ReactionState::Add,
                            emoji,
                            rsp: tx,
                        })) {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        if let Err(e) = res {
                            log::error!("failed to add/remove reaction: {e}");
                        }
                    }
                }
            }
        }
    });

    rsx! (
        InvisibleCloser {
            onclose: move |_|{
                state.write().mutate(Action::SetEmojiDestination(
                    Some(common::state::ui::EmojiDestination::Chatbar),
                ));
                if !*mouse_over_emoji_button.read() && !*mouse_over_emoji_selector.read() {
                    state.write().mutate(Action::SetEmojiPickerVisible(false));
                }
            }
        }
        div {
            onmouseenter: move |_| {
                *mouse_over_emoji_selector.write_silent() = true;
            },
            onmouseleave: move |_| {
                *mouse_over_emoji_selector.write_silent() = false;
                let _ = eval(focus_script);
            },
            id: "emoji_selector",
            aria_label: "emoji-selector",
            tabindex: "0",
            div {
                class: "search-input disable-select",
                textarea::Input {
                    placeholder: get_local_text("uplink.search-placeholder"),
                    key: "{emoji-search-input}",
                    id: String::from("emoji-search-input"),
                    loading: false,
                    ignore_focus: false,
                    show_char_counter: false,
                    aria_label: "emoji-search-input".to_string(),
                    value: String::new(),
                    onreturn:  |_| {},
                    onchange: move |_| {},
                    onkeyup: move |_| {},
                    prevent_up_down_arrows: !emoji_suggestions.is_empty(),
                    oncursor_update: move |(v, p): (String, i64)| {
                        let mut sub: String = v.chars().take(p as usize).collect();
                        sub = if !sub.starts_with(':') {
                            format!(":{}", sub)
                        } else {
                            sub
                        };
                        let capture = EMOJI_REGEX.captures(&sub);
                        match capture {
                            Some(emoji) => {
                                let emoji = &emoji[0];
                                if emoji.contains(char::is_whitespace) {
                                    emoji_suggestions.set(vec![]);
                                    return;
                                }
                                let alias = emoji.replace(':', "");
                                    emoji_suggestions
                                        .set(state.read().ui.emojis.get_matching_emoji(&alias, false));
                            }
                            None => emoji_suggestions.set(vec![]),
                        }
                    },
                    }
                },
            div {
                id: "scrolling",
                padding_top: if !emoji_suggestions.is_empty() {"4px"} else {""},
                if !emoji_suggestions.is_empty() {
                    {
                        rsx!({emoji_suggestions().iter().cloned().map(|(emoji, _)| {
                        rsx!(
                            div {
                                aria_label: emoji.as_str(),
                                class: "emoji",
                                onclick: move |_| select_emoji_to_send(state, emoji.to_string(), &ch),
                                {emoji.as_str()}
                            }
                        )
                    })})}
                } else {
                    {rsx! ({emojis::Group::iter().map(|group| {
                        let name: String = group_to_str(group);
                        rsx!(
                            div {
                                id: "{group_to_str(group)}",
                                Label {
                                    text: name
                                },
                            }
                            div {
                                class: "emojis-container",
                                aria_label: "emojis-container",
                                {group.emojis().filter(|emoji|is_supported(emoji.unicode_version())).map(|emoji| {
                                    rsx!(
                                        div {
                                            aria_label: emoji.as_str(),
                                            class: "emoji",
                                            onclick: move |_| select_emoji_to_send(state, emoji.to_string(), &ch),
                                            {emoji.as_str()}
                                        }
                                    )
                                })}
                            }
                        )
                    })})}
                }
            }
            {nav}
        },
        script { {focus_script_signal()} },
    )
}

// this avoid a BorrowMut error. needs an argument to make the curly braces syntax work
#[component(no_case_check)]
fn render_1(_unused: bool) -> Element {
    let mut state = use_context::<Signal<State>>();
    let mouse_over_emoji_button = use_signal(|| false);
    let visible = state.read().ui.emoji_picker_visible;
    log::debug!("vis {}", visible);

    use_effect(move || {
        to_owned![state];
        spawn(async move {
            state.write_silent().ui.emojis.register_emoji_filter(
                String::from("emoji_picker"),
                |pattern, exact| {
                    emojis::Group::iter()
                        .flat_map(|group| group.emojis())
                        .filter_map(|emoji| {
                            emoji
                                .shortcodes()
                                .find(|short| {
                                    (exact && (*short).eq(pattern))
                                        || (!exact && (*short).starts_with(pattern))
                                })
                                .map(|short| (emoji.to_string(), short.to_string()))
                        })
                        .collect()
                },
            )
        });
    });

    rsx!(
        // If enabled, render the selector popup.
        {
            visible.then(|| {
                rsx!(
                    render_selector {
                        mouse_over_emoji_button: mouse_over_emoji_button,
                        nav: rsx!(build_nav {})
                    },
                    div {
                        onmouseenter: move |_| {
                            *mouse_over_emoji_button.write_silent() = true;
                        },
                        onmouseleave: move |_| {
                            *mouse_over_emoji_button.write_silent() = false;
                        },
                        // Render standard (required) button to toggle.
                        Button {
                            aria_label: "send-emoji-button".to_string(),
                            icon: Icon::FaceSmile,
                            onpress: move |_| {
                                state.write().mutate(Action::SetEmojiPickerVisible(!visible));
                            }
                        },
                    },
                )
            })
        }
    )
}

impl Extension for EmojiSelector {
    fn details(&self) -> Details {
        Details {
            location: Location::Chatbar,
            ext_type: Type::IconLaunched,
            meta: Meta {
                name: "emoji_selector",
                pretty_name: "Emoji Selector",
                description:
                    "Browse the standard unicode library of emoji's and send them to friends.",
                author: "Satellite <devs@satellite.im>",
            },
        }
    }

    fn stylesheet(&self) -> String {
        include_str!("./style.css").to_string()
    }

    fn render(&self, runtime: std::rc::Rc<Runtime>) -> Element {
        // TODO(Migration_0.5): Verify this item later
        // use_hook(move || RuntimeGuard::new(runtime.clone()));
        let _guard = RuntimeGuard::new(runtime.clone());
        let styles = self.stylesheet();
        rsx!(
            style { "{styles}" },
            {rsx!(
               render_1{_unused: true}
            )}
        )
    }
}

fn select_emoji_to_send(mut state: Signal<State>, emoji: String, ch: &Coroutine<Command>) {
    let destination = state
        .read()
        .ui
        .emoji_destination
        .clone()
        .unwrap_or(EmojiDestination::Chatbar);
    match destination {
        EmojiDestination::Chatbar => {
            // If we're on an active chat, append the emoji to the end of the chat message.
            let c = match state.read().get_active_chat() {
                Some(c) => c,
                None => {
                    log::warn!("can't send emoji to chatbar - no active chat");
                    return;
                }
            };
            let draft: String = c.draft.unwrap_or_default();
            let new_draft = format!("{draft}{emoji}");
            state
                .write_silent()
                .mutate(Action::SetChatDraft(c.id, new_draft));
            if let Some(scope_id_usize) = state.read().scope_ids.chatbar {
                needs_update_any(ScopeIds::scope_id_from_usize(scope_id_usize));
            };
        }
        EmojiDestination::Message(conversation_uuid, message_uuid) => {
            ch.send(Command::React(
                conversation_uuid,
                message_uuid,
                emoji.to_string(),
            ));
            state
                .write_silent()
                .mutate(Action::SetEmojiDestination(Some(EmojiDestination::Chatbar)));
            state.write_silent().mutate(Action::AddReaction(
                conversation_uuid,
                message_uuid,
                emoji.to_string(),
            ));
        }
    }
    // Hide the selector when clicking an emoji
    state.write().mutate(Action::SetEmojiPickerVisible(false));
}
