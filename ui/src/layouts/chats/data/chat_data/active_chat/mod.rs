use std::collections::VecDeque;

use common::{
    state::{self, Identity, State},
    warp_runner::ui_adapter,
};
use kit::components::indicator::Platform;
use uuid::Uuid;
use warp::{
    crypto::DID,
    raygun::{self, ConversationSettings, ConversationType},
};

mod messages;
mod metadata;
mod partial_message;
pub use messages::*;
pub use metadata::*;
pub use partial_message::*;

pub const DEFAULT_MESSAGES_TO_TAKE: usize = 80;

#[derive(Debug, Default, Clone)]
pub struct ActiveChat {
    metadata: Metadata,
    pub messages: Messages,
    pub is_initialized: bool,
    pub key: Uuid,
}

impl ActiveChat {
    pub fn new(
        s: &State,
        chat: &state::chats::Chat,
        messages: VecDeque<ui_adapter::Message>,
    ) -> Self {
        Self {
            metadata: Metadata::new(s, chat),
            messages: Messages::new(s.did_key(), messages),
            is_initialized: false,
            key: Uuid::new_v4(),
        }
    }

    pub fn messages(&self) -> VecDeque<ui_adapter::Message> {
        self.messages.all.clone()
    }

    pub fn key(&self) -> Uuid {
        self.key
    }

    pub fn new_key(&mut self) {
        self.key = Uuid::new_v4();
    }

    pub fn has_message_id(&self, id: Uuid) -> bool {
        self.messages.times.contains_key(&id)
    }

    pub fn metadata_changed(&self, metadata: &Metadata) -> bool {
        &self.metadata != metadata
    }

    pub fn set_metadata(&mut self, metadata: Metadata) {
        self.metadata = metadata;
    }

    // may need these later
    // pub fn set_scrolled(&mut self) {
    //     self.scrolled = true;
    // }
    //
    // pub fn clear_scrolled(&mut self) {
    //     self.scrolled = false;
    // }
    //
    // pub fn get_scrolled(&self) -> bool {
    //     self.scrolled
    // }
}

// simplify access to metadata fields
impl ActiveChat {
    pub fn id(&self) -> Uuid {
        self.metadata.chat_id
    }
    pub fn my_id(&self) -> Identity {
        self.metadata.my_id.clone()
    }
    pub fn other_participants(&self) -> Vec<Identity> {
        self.metadata.other_participants.clone()
    }
    pub fn active_participant(&self) -> Identity {
        self.metadata.active_participant.clone()
    }
    pub fn subtext(&self) -> String {
        self.metadata.subtext.clone()
    }
    pub fn is_favorite(&self) -> bool {
        self.metadata.is_favorite
    }
    pub fn first_image(&self) -> String {
        self.metadata.first_image.clone()
    }
    pub fn other_participants_names(&self) -> String {
        self.metadata.other_participants_names.clone()
    }
    pub fn platform(&self) -> Platform {
        self.metadata.platform
    }
    pub fn conversation_name(&self) -> Option<String> {
        self.metadata.conversation_name.clone()
    }
    pub fn conversation_type(&self) -> ConversationType {
        self.metadata
            .conversation_type
            .unwrap_or(ConversationType::Direct)
    }
    pub fn conversation_settings(&self) -> ConversationSettings {
        self.metadata.conversation_settings
    }
    pub fn creator(&self) -> Option<DID> {
        self.metadata.creator.clone()
    }

    pub fn replying_to(&self) -> Option<raygun::Message> {
        self.metadata.replying_to.clone()
    }

    pub fn pinned_messages(&self) -> Vec<raygun::Message> {
        self.metadata.pinned_messages.clone()
    }

    pub fn unreads(&self) -> usize {
        self.metadata.unreads
    }

    pub fn clear_unreads(&mut self) {
        self.metadata.unreads = 0;
    }
}
