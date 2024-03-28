use derive_more::Display;
use futures::{stream::FuturesOrdered, FutureExt, StreamExt};
use uuid::Uuid;
use warp::{
    crypto::DID,
    error::Error,
    raygun::{self, Location, MessageEventKind, MessageOptions},
};

use super::Message;
use crate::{
    state::{self, pending_message::FileProgression},
    warp_runner::{
        ui_adapter::{convert_raygun_message, did_to_identity},
        Messaging,
    },
};

#[derive(Display, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum MessageEvent {
    #[display(fmt = "Received")]
    Received {
        conversation_id: Uuid,
        message: Message,
    },
    #[display(fmt = "Sent")]
    Sent {
        conversation_id: Uuid,
        message: Message,
    },
    #[display(fmt = "Edited")]
    Edited {
        conversation_id: Uuid,
        message: Message,
    },
    #[display(fmt = "Deleted")]
    Deleted {
        conversation_id: Uuid,
        message_id: Uuid,
        // this makes it easy to keep the sidebar up to date with the most recent message
        most_recent_message: Option<Message>,
    },
    #[display(fmt = "MessagePinned")]
    MessagePinned { message: warp::raygun::Message },
    #[display(fmt = "MessageUnpinned")]
    MessageUnpinned { message: warp::raygun::Message },
    #[display(fmt = "MessageReactionAdded")]
    MessageReactionAdded { message: warp::raygun::Message },
    #[display(fmt = "MessageReactionRemoved")]
    MessageReactionRemoved { message: warp::raygun::Message },
    #[display(fmt = "TypingIndicator")]
    TypingIndicator {
        conversation_id: Uuid,
        participant: DID,
    },
    #[display(fmt = "RecipientAdded")]
    RecipientAdded {
        conversation: raygun::Conversation,
        identity: state::Identity,
    },
    #[display(fmt = "RecipientRemoved")]
    RecipientRemoved { conversation: raygun::Conversation },
    #[display(fmt = "ConversationNameUpdated")]
    ConversationNameUpdated { conversation: raygun::Conversation },
    #[display(fmt = "ConversationNameUpdated")]
    ConversationSettingsUpdated {
        conversation: raygun::Conversation,
        settings: raygun::ConversationSettings,
    },
    #[display(fmt = "AttachmentProgress")]
    AttachmentProgress {
        progress: FileProgression,
        location: Location,
        conversation_id: Uuid,
        msg: Uuid,
    },
}

pub async fn convert_message_event(
    event: warp::raygun::MessageEventKind,
    account: &mut super::super::Account,
    messaging: &mut super::super::Messaging,
) -> Result<MessageEvent, Error> {
    let evt = match event {
        MessageEventKind::MessageReceived {
            conversation_id,
            message_id,
        } => {
            let message = messaging.get_message(conversation_id, message_id).await?;

            // Return the event.
            MessageEvent::Received {
                conversation_id,
                message: convert_raygun_message(messaging, &message).await,
            }
        }
        MessageEventKind::MessageSent {
            conversation_id,
            message_id,
        } => {
            let message = messaging.get_message(conversation_id, message_id).await?;
            MessageEvent::Sent {
                conversation_id,
                message: convert_raygun_message(messaging, &message).await,
            }
        }
        MessageEventKind::MessageDeleted {
            conversation_id,
            message_id,
        } => MessageEvent::Deleted {
            conversation_id,
            message_id,
            most_recent_message: fetch_latest(messaging, conversation_id).await,
        },
        MessageEventKind::MessageReactionAdded {
            conversation_id,
            message_id,
            ..
        } => MessageEvent::MessageReactionAdded {
            message: messaging.get_message(conversation_id, message_id).await?,
        },
        MessageEventKind::MessageReactionRemoved {
            conversation_id,
            message_id,
            ..
        } => MessageEvent::MessageReactionRemoved {
            message: messaging.get_message(conversation_id, message_id).await?,
        },
        MessageEventKind::EventReceived {
            conversation_id,
            did_key,
            event,
        } => match event {
            raygun::MessageEvent::Typing => MessageEvent::TypingIndicator {
                conversation_id,
                participant: did_key,
            },
        },
        MessageEventKind::MessageEdited {
            conversation_id,
            message_id,
        } => {
            let message = messaging.get_message(conversation_id, message_id).await?;
            MessageEvent::Edited {
                conversation_id,
                message: convert_raygun_message(messaging, &message).await,
            }
        }
        MessageEventKind::RecipientAdded {
            conversation_id,
            recipient,
        } => MessageEvent::RecipientAdded {
            identity: did_to_identity(&recipient, account).await?,
            conversation: messaging.get_conversation(conversation_id).await?,
        },
        MessageEventKind::RecipientRemoved {
            conversation_id, ..
        } => MessageEvent::RecipientRemoved {
            conversation: messaging.get_conversation(conversation_id).await?,
        },
        MessageEventKind::ConversationNameUpdated {
            conversation_id, ..
        } => MessageEvent::ConversationNameUpdated {
            conversation: messaging.get_conversation(conversation_id).await?,
        },
        MessageEventKind::MessagePinned {
            conversation_id,
            message_id,
        } => MessageEvent::MessagePinned {
            message: messaging.get_message(conversation_id, message_id).await?,
        },
        MessageEventKind::MessageUnpinned {
            conversation_id,
            message_id,
        } => MessageEvent::MessageUnpinned {
            message: messaging.get_message(conversation_id, message_id).await?,
        },
        MessageEventKind::EventCancelled { .. } => {
            todo!("Handle EventCancelled");
        }
        MessageEventKind::ConversationSettingsUpdated {
            conversation_id,
            settings,
        } => MessageEvent::ConversationSettingsUpdated {
            conversation: messaging.get_conversation(conversation_id).await?,
            settings,
        },
    };

    Ok(evt)
}

async fn fetch_latest(messaging: &mut Messaging, conv_id: Uuid) -> Option<Message> {
    let total_messages = messaging.get_message_count(conv_id).await.ok()?;
    let messages = messaging
        .get_messages(
            conv_id,
            MessageOptions::default().set_range(total_messages.saturating_sub(1)..total_messages),
        )
        .await
        .and_then(Vec::<_>::try_from)
        .ok()?;

    let mut messages: Vec<_> = FuturesOrdered::from_iter(
        messages
            .iter()
            .map(|message| convert_raygun_message(messaging, message).boxed()),
    )
    .collect()
    .await;

    messages.pop()
}
