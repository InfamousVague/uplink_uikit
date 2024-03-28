use std::{collections::HashMap, path::PathBuf};

use uuid::Uuid;
use warp::{constellation::Progression, crypto::DID, raygun::Location};

use crate::warp_runner::ui_adapter::Message;
// We can improve message equality detection if warp e.g. can send us their assigned uuid.
// Else it is just a guesswork
#[derive(Clone, Debug)]
pub struct PendingMessage {
    pub attachments_progress: HashMap<FileLocation, FileProgression>,
    pub message: Message,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FileLocation {
    /// Use [`Constellation`] to send a file from constellation
    Constellation { path: String },

    /// Use file from disk
    Disk { path: PathBuf },
}

impl From<Location> for FileLocation {
    fn from(location: Location) -> Self {
        match location {
            Location::Constellation { path } => FileLocation::Constellation { path },
            Location::Disk { path } => FileLocation::Disk { path },
        }
    }
}

impl From<FileLocation> for Location {
    fn from(location: FileLocation) -> Self {
        match location {
            FileLocation::Constellation { path } => Location::Constellation { path },
            FileLocation::Disk { path } => Location::Disk { path },
        }
    }
}

impl PendingMessage {
    pub fn new(chat_id: Uuid, did: DID, message_id: Uuid, text: Vec<String>) -> Self {
        // Create a dummy message
        let mut inner = warp::raygun::Message::default();
        inner.set_id(message_id);
        inner.set_sender(did);
        inner.set_conversation_id(chat_id);
        inner.set_lines(text);
        let message = Message::new(inner, None, Uuid::new_v4().to_string());
        PendingMessage {
            attachments_progress: HashMap::new(),
            message,
        }
    }

    pub fn id(&self) -> Uuid {
        self.message.inner.id()
    }
}

impl PartialEq for PendingMessage {
    fn eq(&self, other: &Self) -> bool {
        self.id().eq(&other.id())
    }
}

impl Eq for PendingMessage {}

#[derive(Debug, Clone)]
pub enum FileProgression {
    CurrentProgress {
        /// name of the file
        name: String,

        /// size of the progression
        current: usize,

        /// total size of the file, if any is supplied
        total: Option<usize>,
    },
    ProgressComplete {
        /// name of the file
        name: String,

        /// total size of the file, if any is supplied
        total: Option<usize>,
    },
    ProgressFailed {
        /// name of the file that failed
        name: String,

        /// last known size, if any, of where it failed
        last_size: Option<usize>,

        /// error of why it failed, if any
        error: std::sync::Arc<warp::error::Error>,
    },
}

impl PartialEq for FileProgression {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                FileProgression::CurrentProgress {
                    name: name1,
                    current: current1,
                    total: total1,
                },
                FileProgression::CurrentProgress {
                    name: name2,
                    current: current2,
                    total: total2,
                },
            ) => name1 == name2 && current1 == current2 && total1 == total2,
            (
                FileProgression::ProgressComplete {
                    name: name1,
                    total: total1,
                },
                FileProgression::ProgressComplete {
                    name: name2,
                    total: total2,
                },
            ) => name1 == name2 && total1 == total2,
            (
                FileProgression::ProgressFailed {
                    name: name1,
                    last_size: last_size1,
                    ..
                },
                FileProgression::ProgressFailed {
                    name: name2,
                    last_size: last_size2,
                    ..
                },
            ) => name1 == name2 && last_size1 == last_size2,
            _ => false,
        }
    }
}

impl From<Progression> for FileProgression {
    fn from(progress: Progression) -> Self {
        match progress {
            Progression::CurrentProgress {
                name,
                current,
                total,
            } => FileProgression::CurrentProgress {
                name,
                current,
                total,
            },
            Progression::ProgressComplete { name, total } => {
                FileProgression::ProgressComplete { name, total }
            }
            Progression::ProgressFailed {
                name,
                last_size,
                error,
            } => FileProgression::ProgressFailed {
                name,
                last_size,
                error: std::sync::Arc::new(error),
            },
        }
    }
}

pub fn progress_file(progress: &FileProgression) -> String {
    match progress {
        FileProgression::CurrentProgress {
            name,
            current: _,
            total: _,
        } => name.clone(),
        FileProgression::ProgressComplete { name, total: _ } => name.clone(),
        FileProgression::ProgressFailed {
            name,
            last_size: _,
            error: _,
        } => name.clone(),
    }
}
