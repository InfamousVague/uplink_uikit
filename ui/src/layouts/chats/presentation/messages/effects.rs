use crate::layouts::chats::{
    data::{ChatData, ScrollTo},
    scripts::{self, SETUP_CONTEXT_PARENT},
};

use dioxus::{
    events::eval,
    prelude::*,
    signals::{Readable, Signal},
};
use dioxus_hooks::{use_signal, Coroutine};

pub fn init_msg_scroll(mut chat_data: Signal<ChatData>, ch: Coroutine<()>) {
    let scroll_script_signal = use_signal(|| String::new());

    let _ = use_resource(move || {
        async move {
            // if chat_data.read().active_chat.is_initialized {
            //     println!("2 - Started init_msg_scroll: {:?}", scroll_script_signal());
            //     let eval_result_scroll_script = eval(&scroll_script_signal());
            //     loop {
            //         tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            //         match eval_result_scroll_script.join().await {
            //             Ok(_) => {
            //                 println!("SCROLLL SCRIPPTTT OOOOKKKKK");
            //                 break;
            //             }
            //             Err(_) => {
            //                 log::error!("failed to join eval");
            //             }
            //         }
            //     }
            //     // if let Err(e) = eval_result_scroll_script.join().await {
            //     //     log::error!("failed to join eval: {:?}", e);
            //     //     return;
            //     // }
            //     println!("Sending command to CoRoutine");
            //     ch.send(());
            //     return;
            // }

            // replicate behavior from before refactor
            let _ = eval(SETUP_CONTEXT_PARENT);

            let chat_id = chat_data.read().active_chat.id();
            let chat_behavior = chat_data.read().get_chat_behavior(chat_id);
            log::debug!("use_effect for init_msg_scroll {}", chat_id,);
            let unreads = chat_data.read().active_chat.unreads();
            chat_data.write_silent().active_chat.messages.loaded.clear();

            let scroll_script = match chat_behavior.view_init.scroll_to {
                // if there are unreads, scroll up so first unread is at top of screen
                // todo: if there are too many unread messages, need to fetch more from warp.
                ScrollTo::MostRecent => {
                    if unreads > 0 {
                        chat_data.write_silent().active_chat.clear_unreads();
                    }
                    let msg_idx = chat_data
                        .read()
                        .active_chat
                        .messages
                        .all
                        .len()
                        .saturating_sub(unreads + 1);
                    let msg_id = chat_data
                        .read()
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
            // *scroll_script_signal.write_silent() = scroll_script;

            let eval_result_scroll_script = eval(&scroll_script);
            println!("Before Eval");
            if let Err(e) = eval_result_scroll_script.join().await {
                log::error!("failed to join eval: {:?}", e);
                return;
            }
            println!("Sending command to CoRoutine");
            ch.send(());
        }
    });

    // use_effect(move || {
    //     to_owned![ch];
    //     spawn(async move {
    //         println!("1 - Started init_msg_scroll");

    //         // replicate behavior from before refactor
    //         let _ = eval(SETUP_CONTEXT_PARENT);

    //         let chat_id = chat_data.read().active_chat.id();
    //         let chat_behavior = chat_data.read().get_chat_behavior(chat_id);
    //         log::debug!("use_effect for init_msg_scroll {}", chat_id,);
    //         let unreads = chat_data.read().active_chat.unreads();
    //         chat_data.write_silent().active_chat.messages.loaded.clear();

    //         let scroll_script = match chat_behavior.view_init.scroll_to {
    //             // if there are unreads, scroll up so first unread is at top of screen
    //             // todo: if there are too many unread messages, need to fetch more from warp.
    //             ScrollTo::MostRecent => {
    //                 if unreads > 0 {
    //                     chat_data.write_silent().active_chat.clear_unreads();
    //                 }
    //                 let msg_idx = chat_data
    //                     .read()
    //                     .active_chat
    //                     .messages
    //                     .all
    //                     .len()
    //                     .saturating_sub(unreads + 1);
    //                 let msg_id = chat_data
    //                     .read()
    //                     .active_chat
    //                     .messages
    //                     .all
    //                     .get(msg_idx)
    //                     .map(|x| x.inner.id());
    //                 match msg_id {
    //                     Some(id) => scripts::SCROLL_TO_END.replace("$MESSAGE_ID", &format!("{id}")),
    //                     None => {
    //                         log::debug!("failed to init message scroll - empty chat");
    //                         chat_data.write().active_chat.is_initialized = true;
    //                         return;
    //                     }
    //                 }
    //             }
    //             ScrollTo::ScrollUp { view_top } => {
    //                 scripts::SCROLL_TO_TOP.replace("$MESSAGE_ID", &format!("{view_top}"))
    //             }
    //             ScrollTo::ScrollDown { view_bottom } => {
    //                 scripts::SCROLL_TO_BOTTOM.replace("$MESSAGE_ID", &format!("{view_bottom}"))
    //             }
    //         };

    //         let eval_result_scroll_script = eval(&scroll_script);
    //         println!("Before Eval");
    //         if let Err(e) = eval_result_scroll_script.join().await {
    //             log::error!("failed to join eval: {:?}", e);
    //             return;
    //         }
    //         println!("Sending command to CoRoutine");
    //         ch.send(());
    //     });
    // });
}
