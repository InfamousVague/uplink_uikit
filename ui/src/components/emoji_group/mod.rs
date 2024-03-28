use common::language::get_local_text;
// use common::{icons::outline::Shape as Icon, state::State};
use common::state::ui::EmojiDestination;
use common::state::State;
use common::{icons::outline::Shape as Icon, state::Action};
use dioxus::prelude::*;
use kit::elements::tooltip::{ArrowPosition, Tooltip};
use kit::elements::{button::Button, Appearance};

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    onselect: EventHandler<String>,
    apply_to: EmojiDestination,
}

#[allow(non_snake_case)]
pub fn EmojiGroup(props: Props) -> Element {
    let mut state = use_context::<Signal<State>>();
    let emojis = state.read().ui.emojis.clone();
    let sorted_list = emojis.get_sorted_vec(Some(4));
    let emoji_selector_extension = "emoji_selector";

    let has_extension = state
        .read()
        .ui
        .extensions
        .enabled_extension(emoji_selector_extension);

    let picker_tooltip = if has_extension {
        rsx!({ () })
    } else {
        rsx!(Tooltip {
            arrow_position: ArrowPosition::Bottom,
            text: get_local_text("messages.missing-emoji-picker")
        })
    };

    rsx!(
        div {
            class: "emoji-group",
            for emoji in sorted_list {
                Button {
                    aria_label: "frequent-emoji".to_string(),
                    key: "{emoji.0}",
                    text: emoji.0.clone(),
                    appearance: Appearance::Secondary,
                    onpress: move |_| {
                        props.onselect.call(emoji.0.clone());
                    }
                }
            }
            Button {
                aria_label: "open-emoji-picker".to_string(),
                key: "{open-picker}",
                icon: Icon::Plus,
                appearance: Appearance::Secondary,
                disabled: !has_extension,
                onpress: move |_| {
                    state.write().mutate(Action::SetEmojiDestination(Some(props.apply_to.clone())));
                    state.write().mutate(Action::SetEmojiPickerVisible(true));
                },
                tooltip: picker_tooltip
            }
        }
    )
}
