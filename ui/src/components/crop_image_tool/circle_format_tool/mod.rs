use base64::{engine::general_purpose, Engine};
use common::{
    icons::outline::Shape, language::get_local_text, utils::lifecycle::use_component_lifecycle,
    STATIC_ARGS,
};
use dioxus::prelude::*;
use kit::{
    elements::{button::Button, label::Label, range::Range, Appearance},
    layout::modal::Modal,
};
use once_cell::sync::Lazy;
use std::{fs, path::PathBuf};
use tokio::io::AsyncWriteExt;

use crate::components::crop_image_tool::b64_encode;

const ADJUST_CROP_CIRCLE_SIZE_SCRIPT: &str = include_str!("./adjust_crop_circle_size.js");
const GET_IMAGE_DIMENSIONS_SCRIPT: &str = include_str!("../get_image_dimensions.js");
const SAVE_CROPPED_IMAGE_SCRIPT: &str = include_str!("./save_cropped_image.js");
const MOVE_IMAGE_SCRIPT: &str = include_str!("../move_image.js");
static CROPPED_IMAGE_PATH: Lazy<PathBuf> =
    Lazy::new(|| STATIC_ARGS.temp_files.join("cropped_image.png"));

#[derive(Debug, Clone)]
struct ImageDimensions {
    height: i64,
    width: i64,
}

#[derive(Props, Clone, PartialEq)]
pub struct Props {
    pub large_thumbnail: (Vec<u8>, String),
    pub on_cancel: EventHandler<()>,
    pub on_crop: EventHandler<PathBuf>,
}

#[allow(non_snake_case)]
pub fn CropCircleImageModal(props: Props) -> Element {
    let mut large_thumbnail = use_signal(|| props.large_thumbnail.clone());

    let mut image_scale: Signal<f32> = use_signal(|| 1.0);
    let mut crop_image = use_signal(|| true);
    let mut cropped_image_pathbuf = use_signal(PathBuf::new);
    let mut clicked_button_to_crop = use_signal(|| false);

    let mut image_dimensions = use_signal(|| ImageDimensions {
        height: 0,
        width: 0,
    });

    if clicked_button_to_crop() {
        props.on_crop.call(cropped_image_pathbuf.read().clone());
        clicked_button_to_crop.set(false);
        crop_image.set(false);
    }

    use_resource(move || {
        to_owned![image_dimensions];
        async move {
            while image_dimensions.read().width == 0 && image_dimensions.read().height == 0 {
                let mut eval_result = eval(GET_IMAGE_DIMENSIONS_SCRIPT);
                if let Ok(val) = eval_result.join().await {
                    *image_dimensions.write_silent() = ImageDimensions {
                        height: val["height"].as_i64().unwrap_or_default(),
                        width: val["width"].as_i64().unwrap_or_default(),
                    };
                }
            }
            let _ = eval(ADJUST_CROP_CIRCLE_SIZE_SCRIPT);
            let _ = eval(MOVE_IMAGE_SCRIPT);
        }
    });

    use_component_lifecycle(
        || {},
        move || {
            let _ = fs::remove_file(CROPPED_IMAGE_PATH.clone());
        },
    );

    return rsx!(div {
            Modal {
                open: crop_image(),
                onclose: move |_| {
                    // Not close if user clicks outside modal
                },
                transparent: false,
                show_close_button: false,
                dont_pad: false,
                div {
                    max_height: "85vh",
                    max_width: "80vw",
                    padding: "16px",
                    onclick: move |_| {},
                    div {
                        id: "crop-image-topbar",
                        aria_label: "crop-image-topbar",
                        background: "var(--secondary)",
                        height: "70px",
                        border_radius: "12px",
                        div {
                            id: "crop-image-topbar-left",
                            padding: "16px",
                            display: "inline-flex",
                            align_items: "center",
                            div {
                                class: "crop-image-topbar-left-title",
                                Label {
                                    text: get_local_text("settings.please-select-area-you-want-to-crop"),
                                    aria_label: "crop-image-topbar-label".to_string(),
                                }
                            },
                            Button {
                                aria_label: "crop-image-cancel-button".to_string(),
                                appearance: Appearance::DangerAlternative,
                                icon: Shape::XMark,
                                onpress: move |_| {
                                    props.on_cancel.call(());
                                    crop_image.set(false);
                                }
                            },
                            div {
                                margin_right: "16px",
                            }
                            Button {
                                aria_label: "crop-image-confirm-button".to_string(),
                                appearance: Appearance::Success,
                                icon: Shape::Check,
                                onpress: move |_| {
                                    spawn({
                                        to_owned![image_scale, cropped_image_pathbuf, clicked_button_to_crop];
                                        async move {
                                            let save_image_cropped_js = SAVE_CROPPED_IMAGE_SCRIPT
                                            .replace("$IMAGE_SCALE", (1.0 / *image_scale.read()).to_string().as_str());
                                            let mut eval_result = eval(&save_image_cropped_js);
                                                if let Ok(val) = eval_result.join().await {
                                                    let thumbnail = val.as_str().unwrap_or_default();
                                                    let base64_string = thumbnail.trim_matches('\"');
                                                    let decoded_bytes = match base64::decode(base64_string) {
                                                        Ok(bytes) => bytes,
                                                        Err(e) => {
                                                            log::error!("Error decoding base64 string for cropped image: {}", e);
                                                            return;
                                                        },
                                                    };
                                                    let mut file = match tokio::fs::File::create(CROPPED_IMAGE_PATH.clone()).await {
                                                        Ok(file) => file,
                                                        Err(e) => {
                                                            log::error!("Error creating cropped image file: {}", e);
                                                            return;
                                                        },
                                                    };

                                                    if let Err(e) = file.write_all(&decoded_bytes).await {
                                                        log::error!("Error writing cropped image file. {}", e);
                                                        return;
                                                    }
                                                    if let Err(e) = file.sync_all().await {
                                                        log::error!("Error syncing cropped image file. {}", e);
                                                        return;
                                                    }
                                                    match tokio::fs::metadata(&CROPPED_IMAGE_PATH.clone()).await {
                                                        Ok(metadata) => {
                                                            if metadata.len() == 0 {
                                                                log::error!("Cropped image file is empty.");
                                                                return;
                                                            }
                                                        }
                                                        Err(e) => {
                                                            log::error!("Error getting metadata for cropped image file: {}", e);
                                                            return;
                                                        }
                                                    }
                                                    cropped_image_pathbuf.with_mut(|f| *f = CROPPED_IMAGE_PATH.clone());
                                                    clicked_button_to_crop.set(true);
                                                }
                                    }
                                    });
                                }
                            }
                        },
                    }
                    div {
                        class: "container",
                        margin_bottom: "16px",
                        text_align: "center",
                        padding: "16px",
                        div {
                            id: "image-crop-box-container",
                            aria_label: "image-crop-box-container",
                            display: "inline-flex",
                            div {
                                id: "img-parent-div",
                                overflow: "hidden",
                                width: "auto",
                                height: "auto",
                                border: "3px solid var(--secondary)",
                                img {
                                    id: "image-preview-modal-file-embed",
                                    alt: "draggable image",
                                    aria_label: "image-preview-modal-file-embed",
                                    src: format_args!("{}", b64_encode(large_thumbnail.read().clone())),
                                    transform: format_args!("scale({})", image_scale.read()),
                                    overflow: "hidden",
                                    transition: "transform 0.2s ease",
                                    max_height: "50vh",
                                    max_width: "50vw",
                                    display: "inline-block",
                                    vertical_align: "middle",
                                    cursor: "move",
                                    position: "relative",
                                    z_index: "-1",
                                    onclick: move |e| e.stop_propagation(),
                                },
                            }
                            div {
                                id: "crop-box",
                                class: "crop-box",
                            },
                            div {
                                id: "shadow-img-mask",
                                class: "shadow-img-mask",
                            }
                        }
                    },
                    div {
                        class: "range-background",
                        Range {
                            aria_label: "range-crop-image".to_string(),
                            initial_value: 1.0,
                            min: 1.0,
                            max: 5.0,
                            step: 0.1,
                            with_buttons: true,
                            onchange: move |size_f32| {
                                *image_scale.write() = size_f32;
                            }
                        }
                    }
                }
            }
        },
    );
}
