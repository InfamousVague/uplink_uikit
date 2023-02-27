//! an event from Warp isn't necessarily what the UI needs to display. and the UI doesn't have access to RayGun, MultiPass, etc. As a result,
//! a translation must be performed by WarpRunner.
//!

mod message_event;
mod multipass_event;
mod raygun_event;

pub use message_event::{convert_message_event, MessageEvent};
pub use multipass_event::{convert_multipass_event, MultiPassEvent};
pub use raygun_event::{convert_raygun_event, RayGunEvent};

use crate::state::{self, chats};
use futures::{stream::FuturesOrdered, FutureExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use warp::{
    crypto::DID,
    error::Error,
    logging::tracing::log,
    multipass::identity::Identity,
    raygun::{self, Conversation, MessageOptions},
};

/// the UI needs additional information for message replies, namely the text of the message being replied to.
/// fetch that before sending the message to the UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub inner: warp::raygun::Message,
    pub in_reply_to: Option<String>,
}

/// if a raygun::Message is in reply to another message, attempt to fetch part of the message text
pub async fn convert_raygun_message(
    messaging: &super::Messaging,
    msg: &raygun::Message,
) -> Message {
    let reply: Option<raygun::Message> = match msg.replied() {
        Some(id) => messaging.get_message(msg.conversation_id(), id).await.ok(),
        None => None,
    };

    Message {
        inner: msg.clone(),
        in_reply_to: reply.and_then(|msg| msg.value().first().cloned()),
    }
}

// this function is used in response to warp events. assuming that the DID from these events is valid.
// Warp sends the Identity over. if the Identity has not been received yet, get_identity will fail for
// a valid DID.
pub async fn did_to_identity(
    did: &DID,
    account: &super::Account,
) -> Result<state::Identity, Error> {
    let identity = match account.get_identity(did.clone().into()).await {
        Ok(list) => list.first().cloned(),
        Err(e) => {
            log::warn!("multipass couldn't find identity {}: {}", did, e);
            None
        }
    };
    let identity = match identity {
        Some(id) => id,
        None => {
            let mut default: Identity = Default::default();
            default.set_did_key(did.clone());
            let did_str = &did.to_string();
            // warning: assumes DIDs are very long. this can cause a panic if that ever changes
            let start = did_str
                .get(8..=10)
                .ok_or(Error::OtherWithContext("DID too short".into()))?;
            let len = did_str.len();
            let end = did_str
                .get(len - 3..)
                .ok_or(Error::OtherWithContext("DID too short".into()))?;
            default.set_username(&format!("{start}...{end}"));
            default
        }
    };
    Ok(state::Identity::from(identity))
}

pub async fn dids_to_identity(
    dids: &[DID],
    account: &mut super::Account,
) -> Result<Vec<state::Identity>, Error> {
    let mut ret = Vec::new();
    ret.reserve(dids.len());
    for id in dids {
        let ident = did_to_identity(id, account).await?;
        ret.push(ident);
    }
    Ok(ret)
}

pub async fn conversation_to_chat(
    conv: &Conversation,
    account: &super::Account,
    messaging: &mut super::Messaging,
) -> Result<chats::Chat, Error> {
    // todo: should Chat::participants include self?
    let mut participants = Vec::new();
    for id in conv.recipients() {
        let identity = did_to_identity(&id, account).await?;
        participants.push(identity);
    }

    // todo: warp doesn't support paging yet. it also doesn't check the range bounds
    let unreads = messaging.get_message_count(conv.id()).await?;
    let messages = messaging
        .get_messages(conv.id(), MessageOptions::default().set_range(0..unreads))
        .await?;

    let messages = FuturesOrdered::from_iter(
        messages
            .iter()
            .map(|message| convert_raygun_message(messaging, message).boxed()),
    )
    .collect()
    .await;

    Ok(chats::Chat {
        id: conv.id(),
        participants,
        messages,
        unreads: unreads as u32,
        replying_to: None,
        typing_indicator: HashMap::new(),
    })
}
