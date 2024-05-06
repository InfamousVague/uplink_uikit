use crate::layouts::chats::{
    data::{ChatData, ScrollTo},
    scripts::{self, SETUP_CONTEXT_PARENT},
};

use dioxus::{
    events::eval,
    prelude::*,
    signals::{Readable, Signal},
};
use dioxus_hooks::Coroutine;

pub fn use_init_msg_scroll(mut chat_data: Signal<ChatData>, ch: Coroutine<()>) {
    let chat_key = chat_data.peek().active_chat.key();
    use_effect(use_reactive(&chat_key, move |_chat_key| {
        println!("Uuid changed: {:?}", _chat_key);
        spawn(async move {
            // replicate behavior from before refactor
            let _ = eval(SETUP_CONTEXT_PARENT);

            let chat_id = chat_data.peek().active_chat.id();
            let chat_behavior = chat_data.peek().get_chat_behavior(chat_id);
            log::debug!("use_effect for init_msg_scroll {}", chat_id,);
            let unreads = chat_data.peek().active_chat.unreads();
            chat_data.write_silent().active_chat.messages.loaded.clear();

            let scroll_script = match chat_behavior.view_init.scroll_to {
                // if there are unreads, scroll up so first unread is at top of screen
                // todo: if there are too many unread messages, need to fetch more from warp.
                ScrollTo::MostRecent => {
                    if unreads > 0 {
                        chat_data.write_silent().active_chat.clear_unreads();
                    }
                    let msg_idx = chat_data
                        .peek()
                        .active_chat
                        .messages
                        .all
                        .len()
                        .saturating_sub(unreads + 1);
                    let msg_id = chat_data
                        .peek()
                        .active_chat
                        .messages
                        .all
                        .get(msg_idx)
                        .map(|x| x.inner.id());
                    match msg_id {
                        Some(id) => scripts::SCROLL_TO_END.replace("$MESSAGE_ID", &format!("{id}")),
                        None => {
                            log::debug!("failed to init message scroll - empty chat");
                            chat_data.write().active_chat.is_initialized = true;
                            return;
                        }
                    }
                }
                ScrollTo::ScrollUp { view_top } => {
                    scripts::SCROLL_TO_TOP.replace("$MESSAGE_ID", &format!("{view_top}"))
                }
                ScrollTo::ScrollDown { view_bottom } => {
                    scripts::SCROLL_TO_BOTTOM.replace("$MESSAGE_ID", &format!("{view_bottom}"))
                }
            };
            loop {
                let eval_result_scroll_script = eval(&scroll_script);

                if let Err(e) = eval_result_scroll_script.join().await {
                    log::error!("failed to join eval: {:?}, script: {:?}", e, scroll_script);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                } else {
                    break;
                }
            }

            println!("Sending command to CoRoutine");
            ch.send(());
        });
    }));
}
