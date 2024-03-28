use common::{
    state::{pending_message::FileLocation, State},
    warp_runner::ui_adapter,
};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

mod active_chat;
mod chat_behavior;

pub use active_chat::*;
pub use chat_behavior::*;
use warp::raygun;

#[derive(Clone, Default)]
pub struct ChatData {
    pub active_chat: ActiveChat,
    pub chat_behaviors: HashMap<Uuid, ChatBehavior>,
}

#[derive(Clone, Default)]
pub struct MessagesToSend {
    pub messages_to_send: Vec<(Option<String>, Vec<FileLocation>)>,
}

#[derive(Clone, Default)]
pub struct MessagesToEdit {
    pub edit: Option<Uuid>,
}

impl PartialEq for ChatData {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl ChatData {
    pub fn add_message_to_view(&mut self, conv_id: Uuid, message_id: Uuid) -> bool {
        if conv_id != self.active_chat.id() {
            log::warn!("add_message_to_view wrong chat id");
            return false;
        }

        let ret = self.active_chat.messages.add_message_to_view(message_id);
        let len = self.active_chat.messages.all.len();
        // for the first message, want to scroll down, not up.
        if len > 1
            && self
                .active_chat
                .messages
                .all
                .front()
                .map(|x| x.inner.id() == message_id)
                .unwrap_or_default()
        {
            self.scroll_up(conv_id);
        } else {
            self.scroll_down(conv_id);
        }
        ret
    }

    pub fn delete_message(&mut self, conversation_id: Uuid, message_id: Uuid) {
        if conversation_id != self.active_chat.id() {
            log::warn!("delete_message wrong chat id");
            return;
        }

        let behavior = self.chat_behaviors.get(&conversation_id).cloned();

        if (self.active_chat.messages.displayed.len() == 1
            && self.active_chat.messages.displayed.contains(&message_id))
            || behavior
                .map(|x| {
                    matches!(
                        x.view_init.scroll_to,
                        ScrollTo::ScrollDown { view_bottom: x }
                            | ScrollTo::ScrollUp { view_top: x }  if x == message_id
                    )
                })
                .unwrap_or_default()
        {
            let idx = self
                .active_chat
                .messages
                .all
                .iter()
                .enumerate()
                .find(|(_idx, val)| val.inner.id() == message_id)
                .map(|x| x.0);
            if idx.map(|x| x == 0).unwrap_or(true) {
                if let Some(behavior) = self.chat_behaviors.get_mut(&conversation_id) {
                    behavior.view_init = ViewInit::default();
                }
            } else if let Some(idx) = idx {
                if let Some(prev_msg) = self.active_chat.messages.all.get(idx.saturating_sub(1)) {
                    if let Some(behavior) = self.chat_behaviors.get_mut(&conversation_id) {
                        behavior.view_init = ViewInit {
                            scroll_to: ScrollTo::ScrollDown {
                                view_bottom: prev_msg.inner.id(),
                            },
                            msg_time: Some(prev_msg.inner.date()),
                            ..Default::default()
                        };
                    } else {
                        log::warn!("delete_message failed to get behavior");
                    }
                } else {
                    // should never happen because idx must be greater than zero
                }
            } else {
                unreachable!();
            }
        }

        self.active_chat
            .messages
            .displayed
            .retain(|x| x != &message_id);
        self.active_chat
            .messages
            .all
            .retain(|x| x.inner.id() != message_id);
    }

    pub fn get_top_of_view(&self, conv_id: Uuid) -> Option<PartialMessage> {
        if self.active_chat.id() != conv_id {
            log::warn!("get_top_of_view wrong chat id");
            return None;
        }

        self.active_chat.messages.get_earliest_displayed()
    }

    pub fn get_bottom_of_view(&self, conv_id: Uuid) -> Option<PartialMessage> {
        if self.active_chat.id() != conv_id {
            log::warn!("get_bottom_of_view wrong chat id");
            return None;
        }

        let r = self.active_chat.messages.get_latest_displayed();
        if r.is_none() {
            log::trace!("couldn't get latest displayed. trying bottom of page instead");
            self.get_bottom_of_page(conv_id)
        } else {
            r
        }
    }

    pub fn get_bottom_of_page(&self, conv_id: Uuid) -> Option<PartialMessage> {
        if self.active_chat.id() != conv_id {
            log::warn!("get_bottom_of_page wrong chat id");
            return None;
        }

        self.active_chat.messages.get_bottom_of_page()
    }

    // call this first to fetch the messages
    pub fn get_chat_behavior(&self, id: Uuid) -> ChatBehavior {
        self.chat_behaviors.get(&id).cloned().unwrap_or_default()
    }

    pub fn insert_messages(&mut self, conv_id: Uuid, messages: Vec<ui_adapter::Message>) {
        if self.active_chat.id() != conv_id {
            log::warn!("insert_messages wrong chat id");
            return;
        }

        self.active_chat
            .messages
            .insert_messages(self.active_chat.my_id().did_key(), messages);
    }

    pub fn is_loaded(&self, conv_id: Uuid) -> bool {
        if self.active_chat.id() != conv_id {
            return false;
        }

        self.active_chat.messages.loaded.len() == self.active_chat.messages.all.len()
    }

    // returns true if the view (or javascript) needs to be updated
    pub fn new_message(&mut self, conv_id: Uuid, msg: ui_adapter::Message) -> bool {
        if conv_id != self.active_chat.id() {
            log::warn!("new_message wrong chat id");
            return false;
        }

        let behavior = self.chat_behaviors.get_mut(&conv_id);
        let should_append_msg = behavior
            .as_ref()
            .map(|behavior| matches!(behavior.view_init.scroll_to, ScrollTo::MostRecent))
            .unwrap_or(true);

        if behavior.is_none() {
            log::warn!("unexpected state in ChatData::new_message - chat behavior is none");
        }

        if should_append_msg {
            self.active_chat
                .messages
                .insert_messages(self.active_chat.my_id().did_key(), vec![msg]);
            true
        } else {
            if let Some(behavior) = behavior {
                behavior.message_received = true;
            }
            false
        }
    }

    pub fn reset_messages(&mut self, conv_id: Uuid) {
        if self.active_chat.id() == conv_id {
            self.active_chat.messages.reset();
            self.set_chat_behavior(conv_id, ChatBehavior::default());
        }
    }

    pub fn remove_message_from_view(&mut self, conv_id: Uuid, message_id: Uuid) -> bool {
        if conv_id != self.active_chat.id() {
            log::warn!("remove_message_from_view wrong chat id");
            return false;
        }

        self.active_chat
            .messages
            .remove_message_from_view(message_id)
    }

    // after the messages have been fetched, init the active chat
    pub fn set_active_chat(
        &mut self,
        s: &State,
        chat_id: &Uuid,
        behavior: ChatBehavior,
        mut messages: Vec<ui_adapter::Message>,
    ) {
        if let Some(chat) = s.get_chat_by_id(*chat_id) {
            self.chat_behaviors.insert(chat.id, behavior);
            self.active_chat = ActiveChat::new(s, &chat, VecDeque::from_iter(messages.drain(..)));
        } else {
            self.active_chat = ActiveChat::default();
            log::error!("failed to set active chat to id: {chat_id}");
        }
    }

    pub fn update_message(&mut self, message: raygun::Message) {
        if self.active_chat.id() != message.conversation_id() {
            log::warn!("update_message wrong chat id");
            return;
        }

        if let Some(msg) = self
            .active_chat
            .messages
            .all
            .iter_mut()
            .find(|m| m.inner.id() == message.id())
        {
            msg.inner = message;
            msg.key = Uuid::new_v4().to_string();
        }
    }

    pub fn set_chat_behavior(&mut self, id: Uuid, behavior: ChatBehavior) {
        self.chat_behaviors.insert(id, behavior);
    }

    pub fn set_scroll_value(&mut self, chat_id: Uuid, val: i64) {
        if let Some(behavior) = self.chat_behaviors.get_mut(&chat_id) {
            behavior.scroll_value.replace(val);
        }
    }
}

impl ChatData {
    fn scroll_up(&mut self, conv_id: Uuid) {
        if let Some(behavior) = self.chat_behaviors.get_mut(&conv_id) {
            if let Some(scroll_top) = self.active_chat.messages.get_earliest_displayed() {
                behavior.view_init.scroll_to = ScrollTo::ScrollUp {
                    view_top: scroll_top.message_id,
                };
                behavior.view_init.msg_time.replace(scroll_top.date);
            } else {
                behavior.view_init.scroll_to = ScrollTo::MostRecent;
                behavior.view_init.msg_time.take();
            }
        } else {
            log::warn!("failed to get chat behavior in ChatData::scroll_up");
        }
    }

    fn scroll_down(&mut self, conv_id: Uuid) {
        if let Some(behavior) = self.chat_behaviors.get_mut(&conv_id) {
            if let Some(scroll_bottom) = self.active_chat.messages.get_latest_displayed() {
                let end_msg = self
                    .active_chat
                    .messages
                    .all
                    .back()
                    .map(|x| x.inner.id())
                    .unwrap_or_default();
                if scroll_bottom.message_id == end_msg
                    && behavior.on_scroll_end == ScrollBehavior::DoNothing
                {
                    behavior.view_init.scroll_to = ScrollTo::MostRecent;
                    behavior.view_init.msg_time.take();
                } else {
                    behavior.view_init.scroll_to = ScrollTo::ScrollDown {
                        view_bottom: scroll_bottom.message_id,
                    };
                    behavior.view_init.msg_time.replace(scroll_bottom.date);
                }
            } else {
                // no messages are displayed. set to MostRecent
                behavior.view_init.scroll_to = ScrollTo::MostRecent;
                behavior.view_init.msg_time.take();
            }
        }
    }
}
