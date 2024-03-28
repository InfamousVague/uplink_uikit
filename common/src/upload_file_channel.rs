use std::{path::PathBuf, sync::Arc};

use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::state::{
    data_transfer::TransferState, pending_message::FileProgression, storage::Storage,
};

pub enum UploadFileAction<T> {
    Starting(Uuid, TransferState, String),
    SizeNotAvailable(PathBuf, String),
    Pausing(Uuid),
    Cancelling(PathBuf, Uuid),
    UploadFiles(Vec<PathBuf>),
    Uploading((Option<FileProgression>, Option<String>, Uuid)),
    Finishing(PathBuf, Uuid),
    Finished(T),
    Remove(PathBuf, Uuid),
    Error(Option<PathBuf>, Option<Uuid>),
}
pub struct UploadFileChannel<T> {
    pub tx: tokio::sync::mpsc::UnboundedSender<UploadFileAction<T>>,
    pub rx: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<UploadFileAction<T>>>>,
}

pub static UPLOAD_FILE_LISTENER: Lazy<UploadFileChannel<Storage>> = Lazy::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    UploadFileChannel {
        tx,
        rx: Arc::new(Mutex::new(rx)),
    }
});
