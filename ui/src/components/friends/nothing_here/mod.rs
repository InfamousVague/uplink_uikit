use common::{language::get_local_text, state::State};
use dioxus::prelude::*;
use kit::elements::label::Label;

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    // The filename of the file
    friends_tab: String,
}

#[allow(non_snake_case)]
pub fn NothingHere(props: Props) -> Element {
    let state = use_context::<Signal<State>>();
    let pending_friends =
        state.read().incoming_fr_identities().len() + state.read().outgoing_fr_identities().len();
    let blocked_friends = state.read().blocked_fr_identities().len();
    let show_warning = match props.friends_tab.as_str() {
        "Pending" => pending_friends == 0,
        "Blocked" => blocked_friends == 0,
        _ => false,
    };

    rsx!(if show_warning {
        {
            rsx!(div {
                class: "friends-list",
                aria_label: "no-requests",
                Label {
                    text: get_local_text("friends.nothing-to-see-here"),
                }
            })
        }
    } else {
        {
            rsx!({})
        }
    })
}
