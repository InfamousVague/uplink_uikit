use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp::{
    crypto::DID,
    raygun::{self, ConversationSettings, ConversationType, Location},
};

use crate::{warp_runner::ui_adapter, STATIC_ARGS};

use super::pending_message::{FileLocation, FileProgression, PendingMessage};

// let (p = window_bottom) be an index into Chat.messages
// show messages from (p - window_size) to (p + window_extra)
// scroll up by window_extra (this allows an onmouseout event to trigger)
// pub struct ChatView {
//     // the idx of the message on the bottom of the screen
//     message_idx: usize,
//     // the number of messages to render in the window
//     window_size: usize,
//     // the number of messages to add outside the window, for scrolling purposes
//     window_extra: usize,
// }

// warning: Chat implements Serialize
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chat {
    // Warp generated UUID of the chat
    // TODO: This should be wired up to warp conversation id's
    pub id: Uuid,
    // Includes the list of participants within a given chat.
    // these don't need to be stored in state either
    pub participants: HashSet<DID>,
    // this makes it easier to tell direct conversations from group conversations. There should be no group conversations with only 2 participants.
    #[serde(default = "default_conversation_type")]
    pub conversation_type: ConversationType,
    // Conversation settings.
    #[serde(default)]
    pub settings: ConversationSettings,
    // only Some for group chats
    #[serde(default)]
    pub conversation_name: Option<String>,
    // Only for group chats
    #[serde(default)]
    pub creator: Option<DID>,
    // Messages should only contain messages we want to render. Do not include the entire message history.
    // don't store the actual message in state
    // warn: Chat has a custom serialize method which skips this field when not using mock data.
    #[serde(default, skip_serializing_if = "skip_chat_messages")]
    pub messages: VecDeque<ui_adapter::Message>,
    // Unread count for this chat, should be cleared when we view the chat.
    #[serde(default)]
    unreads: HashSet<Uuid>,
    // This tracks the messages that mentions the user. For future use
    // E.g. displaying a list of mentions to the user in a pop up
    #[serde(default, skip)]
    pub mentions: VecDeque<ui_adapter::Message>,
    // If a value exists, we will render the message we're replying to above the chatbar
    #[serde(skip)]
    pub replying_to: Option<raygun::Message>,
    // list of users currently typing.
    // (user id, last update time)
    #[serde(skip)]
    pub typing_indicator: HashMap<DID, Instant>,
    #[serde(skip)]
    pub draft: Option<String>,
    // for loading messages into the UI - indicates if more messages can be fetched from warp and added to Chat.messages
    #[serde(skip)]
    pub has_more_messages: bool,
    #[serde(skip)]
    pub pending_outgoing_messages: Vec<PendingMessage>,
    #[serde(skip)]
    pub files_attached_to_send: Vec<Location>,
    // used to determine number of unread messages, for the active chat
    #[serde(skip)]
    pub is_scrolled: bool,
    #[serde(skip)]
    pub pinned_messages: Vec<raygun::Message>,
}

fn skip_chat_messages(_messages: &VecDeque<ui_adapter::Message>) -> bool {
    // don't skip messages and participants when using mock data
    !STATIC_ARGS.use_mock
}

// can't derive default because there is no default conversation_type
impl Default for Chat {
    fn default() -> Self {
        Self {
            id: Default::default(),
            participants: Default::default(),
            conversation_type: ConversationType::Direct,
            settings: ConversationSettings::Direct(Default::default()),
            conversation_name: Default::default(),
            creator: Default::default(),
            messages: Default::default(),
            unreads: Default::default(),
            mentions: Default::default(),
            replying_to: Default::default(),
            typing_indicator: Default::default(),
            draft: Default::default(),
            has_more_messages: Default::default(),
            pending_outgoing_messages: Default::default(),
            files_attached_to_send: Default::default(),
            is_scrolled: false,
            pinned_messages: Default::default(),
        }
    }
}

impl Chat {
    pub fn new(
        id: Uuid,
        participants: HashSet<DID>,
        settings: ConversationSettings,
        conversation_name: Option<String>,
        creator: Option<DID>,
        messages: VecDeque<ui_adapter::Message>,
        pinned_messages: Vec<raygun::Message>,
    ) -> Self {
        let conversation_type = match settings {
            ConversationSettings::Direct(_) => ConversationType::Direct,
            ConversationSettings::Group(_) => ConversationType::Group,
        };
        Self {
            id,
            participants,
            conversation_type,
            settings,
            conversation_name,
            creator,
            messages,
            pinned_messages,
            ..Default::default()
        }
    }
    pub fn append_pending_msg(
        &mut self,
        chat_id: Uuid,
        message_id: Uuid,
        did: DID,
        msg: Vec<String>,
    ) -> bool {
        if self
            .pending_outgoing_messages
            .iter()
            .any(|m| m.id().eq(&message_id))
        {
            return false;
        }
        self.pending_outgoing_messages
            .push(PendingMessage::new(chat_id, did, message_id, msg));
        true
    }

    pub fn update_pending_msg(
        &mut self,
        message_id: Uuid,
        location: Location,
        progress: FileProgression,
    ) {
        if let Some(m) = &mut self
            .pending_outgoing_messages
            .iter_mut()
            .find(|m| m.id().eq(&message_id))
        {
            m.attachments_progress.insert(location.into(), progress);
        }
    }

    pub fn remove_pending_msg_attachment(&mut self, message_id: Uuid, location: FileLocation) {
        if let Some(m) = &mut self
            .pending_outgoing_messages
            .iter_mut()
            .find(|m| m.id().eq(&message_id))
        {
            m.attachments_progress.remove(&location);
            if m.attachments_progress.is_empty() {
                self.remove_pending_msg(message_id);
            }
        }
    }

    pub fn remove_pending_msg(&mut self, message_id: Uuid) {
        self.pending_outgoing_messages
            .retain(|m| !m.id().eq(&message_id))
    }

    pub fn unreads(&self) -> u32 {
        self.unreads.len() as _
    }

    pub fn clear_unreads(&mut self) {
        self.unreads.clear();
    }

    pub fn remove_unread(&mut self, id: &Uuid) -> bool {
        self.unreads.remove(id)
    }

    pub fn add_unread(&mut self, id: Uuid) {
        self.unreads.insert(id);
    }
}

// warning: Chats implements Serialize
#[derive(Clone, Serialize, Debug, Default, Deserialize)]
pub struct Chats {
    // All active chats from warp.
    #[serde(skip)]
    pub all: HashMap<Uuid, Chat>,
    // Chat to display / interact with currently.
    pub active: Option<Uuid>,
    // don't persist a call across restarts
    // the Uuid is the chat associated with the current call
    #[serde(skip)]
    pub active_media: Option<Uuid>, // TODO: in the future, this should probably be a vec of media streams or something
    // Chats to show in the sidebar
    pub in_sidebar: VecDeque<Uuid>,
    // Favorite Chats
    pub favorites: Vec<Uuid>,
    // If there was a problem with loading state or state was deleted we readd all existing chats to the sidebar.
    pub readd_sidebars: bool,
}

impl Chats {
    pub fn active_chat_has_unreads(&self) -> bool {
        let id = match self.active {
            Some(c) => c,
            None => return false,
        };

        match self.all.get(&id) {
            Some(c) => c.unreads() > 0,
            None => false,
        }
    }

    pub fn active_chat_is_scrolled(&self) -> bool {
        let id = match self.active {
            Some(c) => c,
            None => return false,
        };
        self.all.get(&id).map(|c| c.is_scrolled).unwrap_or_default()
    }

    /// returns the UUID of the message being replied to by the active chat
    pub fn get_replying_to(&self) -> Option<Uuid> {
        self.active.and_then(|id| {
            self.all
                .get(&id)
                .and_then(|chat| chat.replying_to.as_ref().map(|msg| msg.id()))
        })
    }
}

fn default_conversation_type() -> ConversationType {
    ConversationType::Direct
}
