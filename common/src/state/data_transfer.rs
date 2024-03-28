use std::sync::Arc;

use humansize::{format_size, DECIMAL};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::language::{get_local_text, get_local_text_with_args};

use super::pending_message::FileProgression;

static SCALE_DECIMAL: [&str; 9] = ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

// Struct to ease updating/reading from it
#[derive(Debug, Clone, Default)]
pub struct TransferState {
    inner: Arc<Mutex<TransferStates>>,
}

impl TransferState {
    pub fn new() -> TransferState {
        TransferState {
            inner: Arc::new(Mutex::new(TransferStates::default())),
        }
    }

    pub async fn matches(&self, state: TransferStates) -> bool {
        *self.inner.lock().await == state
    }

    pub async fn update(&self, cancel: bool) {
        let mut v = self.inner.lock().await;
        *v = v.swap(cancel);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferProgress {
    Starting,
    Progress(u8),
    Finishing,
    Paused(u8),
    Cancelling(u8),
    Error(u8),
}

impl TransferProgress {
    pub fn get_progress(&self) -> u8 {
        match self {
            TransferProgress::Progress(p)
            | TransferProgress::Paused(p)
            | TransferProgress::Cancelling(p)
            | TransferProgress::Error(p) => *p,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TransferStates {
    #[default]
    Normal,
    Cancel,
    Pause,
}

impl TransferStates {
    pub fn swap(&self, cancel: bool) -> TransferStates {
        match self {
            TransferStates::Normal => {
                if cancel {
                    TransferStates::Cancel
                } else {
                    TransferStates::Pause
                }
            }
            TransferStates::Cancel => TransferStates::Cancel,
            TransferStates::Pause => {
                if cancel {
                    TransferStates::Cancel
                } else {
                    TransferStates::Normal
                }
            }
        }
    }
}

unsafe impl Send for TransferStates {}

#[derive(Debug, Clone)]
pub enum TrackerType {
    FileUpload,
    FileDownload,
}

#[derive(Debug, Clone)]
pub struct FileProgress {
    // Use an uuid for duplicate file names
    pub id: Uuid,
    pub file: String,
    pub progress: TransferProgress,
    pub size: usize,
    pub total_size: usize,
    pub description: String,
    // Flag used to pause or cancel this transfer
    pub state: TransferState,
}

impl PartialEq for FileProgress {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.file == other.file
            && self.progress == other.progress
            && self.description == other.description
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransferTracker {
    pub file_progress_upload: Vec<FileProgress>,
    pub file_progress_download: Vec<FileProgress>,
}

impl TransferTracker {
    pub fn start_file_transfer(
        &mut self,
        id: Uuid,
        file: String,
        state: TransferState,
        tracker: TrackerType,
    ) {
        match tracker {
            TrackerType::FileUpload => self.file_progress_upload.push(FileProgress {
                id,
                file,
                progress: TransferProgress::Starting,
                size: 0,
                total_size: 0,
                description: get_local_text("files.transfer-start"),
                state,
            }),
            TrackerType::FileDownload => self.file_progress_download.push(FileProgress {
                id,
                file,
                progress: TransferProgress::Starting,
                size: 0,
                total_size: 0,
                description: get_local_text("files.transfer-start"),
                state,
            }),
        }
    }

    pub fn update_file_upload(
        &mut self,
        file_id: Uuid,
        progression: FileProgression,
        tracker: TrackerType,
    ) {
        let download = matches!(tracker, TrackerType::FileDownload);
        if let Some(f) = self
            .get_tracker_from(tracker)
            .iter_mut()
            .find(|p| file_id.eq(&p.id))
        {
            match progression {
                FileProgression::CurrentProgress {
                    name: _,
                    current,
                    total,
                } => {
                    f.size = current;
                    if let Some(total) = total {
                        f.total_size = total;
                    }
                    let progress = total
                        .map(|total| current as f64 / total as f64 * 100.)
                        .unwrap_or_default() as u8;
                    let (current_desc, total_desc) = Self::get_size_display(f.size, f.total_size);
                    f.description = get_local_text_with_args(
                        if download {
                            "files.transfer-progress-upload"
                        } else {
                            "files.transfer-progress-download"
                        },
                        vec![
                            ("progress", progress.to_string()),
                            ("size", current_desc),
                            ("total", total_desc),
                        ],
                    );
                    f.progress = TransferProgress::Progress(progress);
                }
                FileProgression::ProgressComplete { name: _, total } => {
                    if let Some(total) = total {
                        f.total_size = total;
                    }
                    f.description = get_local_text_with_args(
                        "files.transfer-finishing",
                        vec![("size", format_size(f.total_size, DECIMAL))],
                    );
                    f.progress = TransferProgress::Finishing;
                }
                FileProgression::ProgressFailed {
                    name: _,
                    last_size,
                    error,
                } => {
                    f.description = get_local_text_with_args(
                        "files.transfer-error",
                        vec![("error", error.to_string())],
                    );
                    if let Some(last_size) = last_size {
                        f.total_size = last_size;
                    }
                    f.progress = TransferProgress::Error(f.progress.get_progress());
                }
            };
        }
    }

    pub fn update_file_description(
        &mut self,
        file_id: Uuid,
        description: String,
        tracker: TrackerType,
    ) {
        if let Some(f) = self
            .get_tracker_from(tracker)
            .iter_mut()
            .find(|p| file_id.eq(&p.id))
        {
            f.description = description;
        }
    }

    pub fn pause_file_upload(&mut self, file_id: Uuid, tracker: TrackerType) {
        if let Some(f) = self
            .get_tracker_from(tracker)
            .iter_mut()
            .find(|p| file_id.eq(&p.id))
        {
            let (current_desc, total_desc) = Self::get_size_display(f.size, f.total_size);
            f.description = get_local_text_with_args(
                "files.transfer-paused",
                vec![
                    ("progress", f.progress.get_progress().to_string()),
                    ("size", current_desc),
                    ("total", total_desc),
                ],
            );
            f.progress = TransferProgress::Paused(f.progress.get_progress());
        }
    }

    pub fn cancel_file_upload(&mut self, file_id: Uuid, tracker: TrackerType) {
        if let Some(f) = self
            .get_tracker_from(tracker)
            .iter_mut()
            .find(|p| file_id.eq(&p.id))
        {
            f.description = get_local_text("files.transfer-cancelling");
            f.progress = TransferProgress::Cancelling(f.progress.get_progress());
        }
    }

    pub fn error_file_upload(&mut self, file_id: Uuid, tracker: TrackerType) {
        if let Some(f) = self
            .get_tracker_from(tracker)
            .iter_mut()
            .find(|p| file_id.eq(&p.id))
        {
            f.progress = TransferProgress::Error(f.progress.get_progress());
            f.description = get_local_text("files.error-to-upload");
        }
    }

    pub fn remove_file_upload(&mut self, file_id: Uuid, tracker: TrackerType) {
        self.get_tracker_from(tracker)
            .retain(|p| !file_id.eq(&p.id))
    }

    fn get_tracker_from(&mut self, tracker: TrackerType) -> &mut Vec<FileProgress> {
        match tracker {
            TrackerType::FileUpload => &mut self.file_progress_upload,
            TrackerType::FileDownload => &mut self.file_progress_download,
        }
    }

    pub fn get_tracker(&self, tracker: TrackerType) -> &Vec<FileProgress> {
        match tracker {
            TrackerType::FileUpload => &self.file_progress_upload,
            TrackerType::FileDownload => &self.file_progress_download,
        }
    }

    pub fn total_progress(&self) -> i8 {
        let upload = self
            .file_progress_upload
            .iter()
            .filter_map(|f| match f.progress {
                TransferProgress::Progress(p) | TransferProgress::Paused(p) => Some(p as u32),
                _ => None,
            });
        let download = self
            .file_progress_download
            .iter()
            .filter_map(|f| match f.progress {
                TransferProgress::Progress(p) | TransferProgress::Paused(p) => Some(p as u32),
                _ => None,
            });
        let count = (upload.clone().count() + download.clone().count()) as f64 * 100.;
        let sum = (upload.sum::<u32>() + download.sum::<u32>()) as f64;
        if count > 0. {
            ((sum / count) * 100.) as i8
        } else {
            -1
        }
    }

    pub fn get_size_display(size: usize, total: usize) -> (String, String) {
        let divider = 1000.0;
        let mut total = total as f64;
        let mut scale_idx = 0;
        // First format the total size
        while total.abs() >= divider {
            total /= divider;
            scale_idx += 1;
        }
        let scale = SCALE_DECIMAL[scale_idx];
        let places = if total.fract() == 0.0 { 0 } else { 2 };
        let total_size = format!("{:.*} {}", places, total, scale);

        // Format the current size now using the scale of the total size
        let mut size = size as f64;

        while scale_idx > 0 {
            size /= divider;
            scale_idx -= 1;
        }
        let places = if size.fract() == 0.0 { 0 } else { 2 };
        (format!("{:.*}", places, size), total_size)
    }
}
