use std::path::{Path, PathBuf};

use dioxus::prelude::*;

use kit::{
    components::context_menu::{ContextItem, ContextMenu},
    elements::loader::Loader,
    layout::modal::Modal,
};
use warp::constellation::file::File;

use common::{
    get_file_type,
    icons::outline::Shape as Icon,
    is_audio, is_lang_file, is_video,
    language::get_local_text,
    state::{State, ToastNotification},
    utils::{
        img_dimensions_preview::{IMAGE_MAX_HEIGHT, IMAGE_MAX_WIDTH},
        local_file_path::get_fixed_path_to_load_local_file,
    },
    warp_runner::thumbnail_to_base64,
    FileType, STATIC_ARGS,
};

const TIME_TO_WAIT_FOR_VIDEO_TO_DOWNLOAD: u64 = 10000;
const TIME_TO_WAIT_FOR_IMAGE_TO_DOWNLOAD: u64 = 1500;

#[component(no_case_check)]
pub fn open_file_preview_modal(
    on_dismiss: EventHandler<()>,
    on_download: EventHandler<Option<PathBuf>>,
    file: File,
) -> Element {
    rsx!(Modal {
        onclose: move |_| on_dismiss.call(()),
        open: true,
        transparent: false,
        dont_pad: true,
        close_on_click_inside_modal: false,
        children: rsx!(FilePreview {
            file: file,
            on_download: move |temp_path| {
                on_download.call(temp_path);
            },
            on_dismiss: move |_| on_dismiss.call(()),
        })
    })
}

#[derive(Props, Clone, PartialEq)]
struct Props {
    file: File,
    on_download: EventHandler<Option<PathBuf>>,
    on_dismiss: EventHandler<()>,
}

#[allow(non_snake_case)]
fn FilePreview(props: Props) -> Element {
    let mut state = use_context::<Signal<State>>();
    let mut file_path_in_local_disk = use_signal(PathBuf::new);

    let thumbnail = thumbnail_to_base64(&props.file);
    let temp_dir = STATIC_ARGS.temp_files.join(props.file.name());

    let file_loading_counter = use_signal(|| 0);
    // Using id to change file name in case of duplicate files and avoid
    // open different file from that user clicked
    let temp_dir_with_file_id = STATIC_ARGS.temp_files.join(format!(
        "{}.{}",
        props.file.id(),
        temp_dir.extension().unwrap_or_default().to_string_lossy()
    ));
    let mut should_download = use_signal(|| true);

    let is_video = is_video(&props.file.name());
    let is_audio = is_audio(&props.file.name());
    let is_code = is_lang_file(&props.file.name());

    if file_path_in_local_disk.read().to_string_lossy().is_empty() {
        if !temp_dir_with_file_id.exists() && should_download() {
            props.on_download.call(Some(temp_dir.clone()));
            should_download.set(false);
        }
        if temp_dir_with_file_id.exists() {
            file_path_in_local_disk.set(temp_dir_with_file_id.clone());
        }
    }

    let _ = use_future(move || {
        to_owned![
            temp_dir,
            file_path_in_local_disk,
            temp_dir_with_file_id,
            file_loading_counter
        ];
        async move {
            let mut counter = 0;
            loop {
                if file_path_in_local_disk.read().exists() {
                    break;
                }
                if temp_dir.exists() {
                    let _ = tokio::fs::rename(
                        temp_dir.to_string_lossy().to_string(),
                        temp_dir_with_file_id.to_string_lossy().to_string(),
                    )
                    .await;
                    file_path_in_local_disk.set(temp_dir_with_file_id);
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                counter += 250;
                if counter > TIME_TO_WAIT_FOR_IMAGE_TO_DOWNLOAD && !is_video && !is_audio {
                    file_loading_counter.with_mut(|i| *i = counter);
                    break;
                }
                if counter > TIME_TO_WAIT_FOR_VIDEO_TO_DOWNLOAD && (is_video || is_audio) {
                    file_loading_counter.with_mut(|i| *i = counter);
                    break;
                }
            }
        }
    });

    let local_disk_path_fixed = get_fixed_path_to_load_local_file(file_path_in_local_disk());

    let code_content = is_code
        .then(|| std::fs::read_to_string(file_path_in_local_disk()).ok())
        .flatten()
        .unwrap_or_default();

    let file_type = get_file_type(&props.file.name());
    let should_dismiss_on_error = use_signal(|| false);

    if file_type == FileType::Unkwnown {
        state
            .write()
            .mutate(common::state::Action::AddToastNotification(
                ToastNotification::init(
                    "".into(),
                    get_local_text("files.not-possible-to-preview-file"),
                    None,
                    3,
                ),
            ));
        props.on_dismiss.call(());
    }

    let is_developer_mode = state.read().configuration.developer.developer_mode;

    rsx!(
        ContextMenu {
            id: "file-preview-context-menu".to_string(),
            devmode: is_developer_mode,
            items: rsx!(
                ContextItem {
                    icon: Icon::ArrowDownCircle,
                    aria_label: "files-download-preview".to_string(),
                    text: get_local_text("files.download"),
                    onpress: move |_| {
                        props.on_download.call(None);
                    }
                },
            ),
            if *file_loading_counter.read() > TIME_TO_WAIT_FOR_VIDEO_TO_DOWNLOAD
                && (is_video || is_audio) {
                // It will show a video player with error, because take much time
                // to download a video and is not possible to load it
                {rsx!(FileTypeTag {
                    file_type: file_type,
                    source: "".to_string(),
                    code_content: code_content,
                })}
            } else if !file_path_in_local_disk.read().exists()
                && *file_loading_counter.read() > TIME_TO_WAIT_FOR_IMAGE_TO_DOWNLOAD
                && file_type == FileType::Image {
                // It will show image with thumbnail and not with high quality
                // because image didn't download and is not possible to load it
                {rsx!(FileTypeTag {
                    file_type: file_type,
                    source: thumbnail,
                    code_content: code_content,
                })}
            } else if file_path_in_local_disk().exists() {
                {*should_dismiss_on_error.write_silent() = true;
                // Success for both any kind of file
                rsx!(FileTypeTag {
                    file_type: file_type,
                    source: local_disk_path_fixed,
                    code_content: code_content,
                })}
            } else if *file_loading_counter.read() <  TIME_TO_WAIT_FOR_VIDEO_TO_DOWNLOAD {
                if *should_dismiss_on_error.read() {
                    {props.on_dismiss.call(());}
                }
                {rsx!(Loader {
                    spinning: true
                },)}
            } else {
                {state
                .write()
                .mutate(common::state::Action::AddToastNotification(
                    ToastNotification::init(
                        "".into(),
                        get_local_text("files.not-possible-to-preview-file"),
                        None,
                        3,
                    ),
                ));
                props.on_dismiss.call(());
                rsx!(div {})}
            }
        },
    )
}

#[derive(Props, PartialEq, Clone)]
struct FileTypeTagProps {
    file_type: FileType,
    source: String,
    code_content: String,
}

#[allow(non_snake_case)]
fn FileTypeTag(props: FileTypeTagProps) -> Element {
    let file_type = props.file_type.clone();
    let source_path = props.source.clone();
    let code_content = props.code_content.clone();
    let code_class = get_language_class(&source_path);

    match file_type {
        FileType::Video => {
            rsx!(video {
                id: "file_preview_img",
                aria_label: "file-preview-image",
                max_height: IMAGE_MAX_HEIGHT,
                max_width: IMAGE_MAX_WIDTH,
                autoplay: true,
                controls: true,
                src: "{source_path}"
            })
        }
        FileType::Audio => {
            rsx!(
             div {
                 height: "80px",
                 padding_top: "50px",
                 audio {
                     id: "file_preview_img",
                     aria_label: "file-preview-image",
                     autoplay: true,
                     controls: true,
                     src: "{source_path}"
                 }
             }
            )
        }
        FileType::Image => {
            rsx!(img {
                id: "file_preview_img",
                aria_label: "file-preview-image",
                max_height: IMAGE_MAX_HEIGHT,
                max_width: IMAGE_MAX_WIDTH,
                src: "{source_path}"
            },)
        }
        FileType::Doc => {
            rsx!(iframe {
                id: "file_preview_img",
                aria_label: "file-preview-image",
                max_height: "80vh",
                max_width: "80vw",
                height: "800px",
                width: "800px",
                src: "{source_path}"
            })
        }
        FileType::Code => {
            rsx!(
                div {
                    class: "code-preview",
                    pre {
                        code {
                            class: format_args!("{code_class}"),
                            "{code_content}"
                        }
                    }
                }
                script {
                    r#"
                    (() => {{
                        Prism.highlightAll();
                    }})();
                    "#
                }
            )
        }
        _ => rsx!(div {}),
    }
}

fn get_language_class(file_path: &str) -> String {
    let extension = Path::new(file_path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();

    let extension_formatted = match extension {
        "rs" => "rust",
        "js" => "javascript",
        "ts" => "typescript",
        "py" => "python",
        "html" => "html",
        "css" => "css",
        "toml" => "toml",
        "java" => "java",
        "cpp" => "cpp",
        "c" => "c",
        _ => "plaintext",
    }
    .to_string();

    format!("language-{}", extension_formatted)
}
