use common::icons::outline::Shape as Icon;
use common::state::data_transfer::{TrackerType, TransferProgress, TransferTracker};
use common::state::State;
use common::{language::get_local_text, state::data_transfer::FileProgress};
use dioxus::prelude::*;
use futures::StreamExt;
use kit::elements::{button::Button, Appearance};

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    state: Signal<State>,
    modal: Option<bool>,
}

pub fn FileTransferModal(props: Props) -> Element {
    let file_tracker = use_context::<Signal<TransferTracker>>();
    // TODO(Migration_0.5): See this unwrap later
    props.state.write_silent().scope_ids.file_transfer = Some(current_scope_id().unwrap().0);
    let tracker = file_tracker.read();
    let (file_progress_upload, file_progress_download) = (
        tracker.get_tracker(TrackerType::FileUpload),
        tracker.get_tracker(TrackerType::FileDownload),
    );
    if file_progress_upload.is_empty() && file_progress_download.is_empty() {
        return rsx!({});
    }
    let modal = props.modal.unwrap_or_default();
    rsx!(div {
        class: format_args!("file-transfer-wrap {}", if modal {"file-transfer-modal"} else {""}),
       {(!file_progress_upload.is_empty()).then(||
            rsx!(FileTransferElement {
                transfers: file_progress_upload.clone(),
                label: get_local_text("uplink.upload-queue"),
            })
        )},
        {(!file_progress_download.is_empty()).then(||
            rsx!(FileTransferElement {
                transfers: file_progress_download.clone(),
                label: get_local_text("uplink.download-queue"),
            })
        )},
    })
}

#[derive(Props, Clone, PartialEq)]
pub struct TransferProps {
    transfers: Vec<FileProgress>,
    label: String,
}

pub fn FileTransferElement(props: TransferProps) -> Element {
    rsx!(div {
        class: "file-transfer-container",
        div {
            class: "file-transfer-label-container",
            label {
                {props.label.clone()},
            },
        },
        {props.transfers.iter().map(|f| {
            let progress = f.progress.get_progress();
            let state = f.state.clone();
            let ch = use_coroutine( |mut rx: UnboundedReceiver<bool>| {
                to_owned![state];
                async move {
                    while let Some(cancel) = rx.next().await {
                        state.update(cancel).await;
                    }
                }
            });
            rsx!(
                div {
                    class: "file-transfer-file",
                    aria_label: "file-transfer-file",
                    div {
                        class: "progress-container",
                        aria_label: "progress-container",
                        p {
                            class: "filename-and-file-queue-text",
                            aria_label: "filename-and-file-queue-text",
                            margin_right: "auto",
                            {f.file.to_string()},
                        },
                        ProgressIndicator {
                            progress: progress
                        },
                        {f.description.map(|desc|rsx!(div {
                            class: "file-progress-description",
                            aria_label: "file-progress-description",
                            {desc.to_string()}
                        }))},
                        p {
                            class: "transfer-progress-percentage",
                            aria_label: "transfer-progress-percentage",
                           {f.description.clone()}
                        },
                    },
                    div {
                        class: "file-transfer-buttons",
                        Button {
                            aria_label: "pause-upload".to_string(),
                            disabled: matches!(f.progress, TransferProgress::Progress(100)),
                            appearance: Appearance::Primary,
                            small: true,
                            icon: if matches!(f.progress, TransferProgress::Paused(_)) { Icon::Play } else { Icon::Pause },
                            onpress: move |_| {
                                ch.send(false);
                            },
                        },
                        Button {
                            aria_label: "cancel-upload".to_string(),
                            disabled: matches!(f.progress, TransferProgress::Cancelling(_) | TransferProgress::Progress(100)),
                            appearance: Appearance::Primary,
                            icon: Icon::XMark,
                            small: true,
                            onpress: move |_| {
                                ch.send(true);
                            },
                        }
                    }
                }
            )
        })}
    })
}

#[derive(Props, Clone, PartialEq)]
pub struct ProgressIndicatorProps {
    progress: u8,
}

pub fn ProgressIndicator(props: ProgressIndicatorProps) -> Element {
    rsx!(div{
        class: "progress-indicator-wrap",
        div {
            class: "progress-indicator",
            div {
                class: "progress-indicator progress-indicator-overlay",
                width: format_args!("{}%", props.progress)
            },
        }
    })
}
