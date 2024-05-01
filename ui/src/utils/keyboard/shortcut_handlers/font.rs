use common::state::{Action, State};
use dioxus::prelude::*;
use dioxus::signals::Signal;

use crate::utils::get_font_sizes::{FONT_SIZE_BIGGEST, FONT_SIZE_SMALLEST};

pub fn increase_size(mut state: Signal<State>) {
    let value = state.peek().settings.peek().font_scale();
    if value < FONT_SIZE_BIGGEST {
        state.write().mutate(Action::SetFontScale(value + 0.25));
    }
}

pub fn decrease_size(mut state: Signal<State>) {
    let value = state.peek().settings.peek().font_scale();
    if value > FONT_SIZE_SMALLEST {
        state.write().mutate(Action::SetFontScale(value - 0.25));
    }
}
