// todo: move the MessageGroup from State to this file.
// todo: consider building a VecDeque of MessageGroup inside of ChatData, as messages are added/removed.

use std::collections::VecDeque;

use common::{
    state::{
        pending_message::{FileLocation, FileProgression, PendingMessage},
        Identity,
    },
    warp_runner::ui_adapter,
};
use warp::crypto::DID;

// Define a struct to represent a group of messages from the same sender.
#[derive(Clone)]
pub struct MessageGroup {
    pub sender: DID,
    pub remote: bool,
    pub messages: Vec<MessageGroupMsg>,
}

impl MessageGroup {
    pub fn new(sender: DID, my_did: &DID) -> Self {
        Self {
            remote: sender != *my_did,
            sender,
            messages: vec![],
        }
    }
}

// The naming is somewhat verbose and redundant but it's better than what it was before.
// Define a struct to represent a message that has been placed into a group.
#[derive(Clone)]
pub struct MessageGroupMsg {
    pub message: ui_adapter::Message,
    pub is_pending: bool,
    pub is_first: bool,
    pub is_last: bool,
    pub file_progress: Option<Vec<(FileLocation, FileProgression)>>,
}

impl MessageGroupMsg {
    pub fn clear_last(&mut self) {
        self.is_last = false;
    }
}

/// Create a Vec of MessageGroup from a Vec of ui_adapter::Message.
///
/// If sender is different from the last group message, it creates a new group.
///
/// if last message in a group is a reply, it creates a new group.
pub fn create_message_groups(
    my_id: Identity,
    other_ids: Vec<Identity>,
    mut input: VecDeque<ui_adapter::Message>,
) -> Vec<MessageGroup> {
    let mut messages: Vec<MessageGroup> = vec![];
    let mut other_ids = other_ids.clone();
    other_ids.push(my_id.clone());

    for msg in input.drain(..) {
        if let Some(group) = messages.iter_mut().last() {
            if let Some(last_group_message) = group.messages.last() {
                if group.sender == msg.inner.sender()
                    && last_group_message.message.in_reply_to.is_none()
                    && msg.in_reply_to.is_none()
                {
                    let g = MessageGroupMsg {
                        message: msg.clone(),
                        is_pending: false,
                        is_first: false,
                        is_last: true,
                        file_progress: None,
                    };
                    // I really hope last() is O(1) time
                    if let Some(g) = group.messages.iter_mut().last() {
                        g.clear_last();
                    }

                    group.messages.push(g);
                    continue;
                }
            }
        }

        // new group
        let mut grp = MessageGroup::new(msg.inner.sender(), &my_id.did_key());
        let g = MessageGroupMsg {
            message: msg.clone(),
            is_pending: false,
            is_first: true,
            is_last: true,
            file_progress: None,
        };
        grp.messages.push(g);
        messages.push(grp);
    }
    messages
}

pub fn pending_group_messages(
    pending: &[PendingMessage],
    other_ids: Vec<Identity>,
    my_id: Identity,
) -> Option<MessageGroup> {
    if pending.is_empty() {
        return None;
    };
    let mut other_ids = other_ids.clone();
    other_ids.push(my_id.clone());

    let mut messages: Vec<MessageGroupMsg> = vec![];
    let size = pending.len();
    for (i, msg) in pending.iter().enumerate() {
        let message = msg.message.clone();
        if i == size - 1 {
            let g = MessageGroupMsg {
                message,
                is_pending: true,
                is_first: false,
                is_last: true,
                file_progress: Some(msg.attachments_progress.clone().into_iter().collect()),
            };
            messages.push(g);
            continue;
        }
        let g = MessageGroupMsg {
            message,
            is_pending: true,
            is_first: true,
            is_last: true,
            file_progress: Some(msg.attachments_progress.clone().into_iter().collect()),
        };
        messages.push(g);
    }
    Some(MessageGroup {
        sender: my_id.did_key(),
        remote: false,
        messages,
    })
}
