use std::collections::HashMap;

use futures::StreamExt;
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;
use warp::raygun;

use super::Messaging;

pub struct Manager {
    // (conversation_id, thread)
    handles: HashMap<Uuid, JoinHandle<()>>,
    ch: mpsc::UnboundedSender<raygun::MessageEventKind>,
}

impl Manager {
    pub fn new(ch: mpsc::UnboundedSender<raygun::MessageEventKind>) -> Self {
        Self {
            handles: HashMap::new(),
            ch,
        }
    }

    pub async fn add_stream(
        &mut self,
        conv_id: Uuid,
        messaging: &mut Messaging,
    ) -> Result<(), warp::error::Error> {
        let ch = self.ch.clone();
        let mut stream = messaging.get_conversation_stream(conv_id).await?;
        let t = tokio::task::spawn(async move {
            while let Some(evt) = stream.next().await {
                let _ = ch.send(evt);
            }
        });

        // ensure that if a handle is overwritten, the old one is aborted
        if let Some(handle) = self.handles.insert(conv_id, t) {
            handle.abort();
        }
        Ok(())
    }

    pub fn remove_stream(&mut self, conv_id: Uuid) {
        if let Some(handle) = self.handles.remove(&conv_id) {
            handle.abort();
        }
    }
}

impl std::ops::Drop for Manager {
    fn drop(&mut self) {
        for (_id, handle) in self.handles.drain() {
            handle.abort();
        }
    }
}
