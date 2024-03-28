use common::state::{Action, State};

use dioxus::prelude::*;
use dioxus_desktop::use_window;
use dioxus_router::prelude::use_navigator;

use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;
use kit::{
    elements::{
        button::Button,
        tooltip::{ArrowPosition, Tooltip},
        Appearance,
    },
    layout::topbar::Topbar,
};

use crate::UplinkRoute;

#[derive(Eq, PartialEq, Props, Clone)]
pub struct Props {
    #[props(optional)]
    larger: Option<bool>,
    settings_text: String,
    enable_camera_text: String,
    fullscreen_text: String,
    popout_player_text: String,
    screenshare_text: String,
    end_text: String,
}

#[allow(non_snake_case)]
pub fn _MediaPlayer(props: Props) -> Element {
    let mut state = use_context::<Signal<State>>();
    let window = use_window();
    let silenced = state
        .read()
        .ui
        .call_info
        .active_call()
        .map(|x| x.call.call_silenced)
        .unwrap_or(false);

    let _silenced_str = silenced.to_string();

    let router = use_navigator();

    rsx!(div {
        id: "media-player",
        div {
            id: "handle",
            IconElement {
                icon: Icon::ChevronUpDown,
                size: 20,
            },
        },
        Topbar {
            controls:
                rsx! (
                    Button {
                        icon: Icon::ArrowsPointingOut,
                        appearance: Appearance::Secondary,
                        tooltip: rsx!(
                            Tooltip {
                                arrow_position: ArrowPosition::Top,
                                text: props.fullscreen_text.clone(),
                            }
                        ),
                    },
                )
        },
        div {
            id: "media-renderer",
            // video not yet supported.
            /*div {
                class: "video-wrap",
                Button {
                    icon: Icon::Square2Stack,
                    appearance: Appearance::Transparent,
                    tooltip: rsx!(
                        Tooltip {
                            arrow_position: ArrowPosition::Right,
                            text: props.popout_player_text.clone(),
                        }
                    )),
                    onpress: move |_| {
                         if state.read().ui.popout_media_player {
                             state.write().mutate(Action::ClearCallPopout(window.clone()));
                             return;
                         }

                        // close the PopoutPlayer on drop, if not already closed
                        // pass WindowDropHandler as a prop so that it doesn't get dropped when PopoutPlayer returns an Element
                        let drop_handler = WindowDropHandler::new(WindowManagerCmd::ClosePopout);
                        let popout = VirtualDom::new_with_props(PopoutPlayer, PopoutPlayerProps{
                            _drop_handler: drop_handler
                        });
                        let window = window.new_window(popout, Default::default());
                        if let Some(wv) = Weak::upgrade(&window) {
                            let id = wv.window().id();
                            state.write().mutate(Action::SetCallPopout(id));
                        }
                    }
                },
                // don't render MediadPlayer if the video is popped out
                state.read().ui.popout_media_player.then(|| rsx!(
                    span {
                        class: "popped-out",
                        video {}
                    }
                )),
                (!state.read().ui.popout_media_player).then(|| rsx!(
                    video {
                        src: "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/Sintel.mp4",
                        autoplay: "true",
                        "loop": "false",
                        muted: "{silenced_str}"
                    }
                ))
            }*/
        },
        div {
            class: "media-controls",
            Button {
                icon: Icon::VideoCamera,
                appearance: Appearance::Secondary,
                tooltip: rsx!(
                    Tooltip {
                        arrow_position: ArrowPosition::Bottom,
                        text: props.enable_camera_text.clone(),
                    }
                ),
            },
            Button {
                icon: Icon::Window,
                appearance: Appearance::Secondary,
                tooltip: rsx!(
                    Tooltip {
                        arrow_position: ArrowPosition::Bottom,
                        text: props.screenshare_text.clone(),
                    }
                ),
                // TODO: https://github.com/quadrupleslap/scrap
            },
            Button {
                icon: Icon::PhoneXMark,
                appearance: Appearance::Danger,
                text: props.end_text.clone(),
                onpress: move |_| {
                    state.write().mutate(Action::ClearCallPopout(window.clone()));
                   // state.write().mutate(Action::DisableMedia);
                }
            },
            Button {
                icon: Icon::Cog6Tooth,
                appearance: Appearance::Secondary,
                tooltip: rsx!(
                    Tooltip {
                        arrow_position: ArrowPosition::Bottom,
                        text: props.settings_text.clone(),
                    }
                ),
                // TODO: Navigate to media settings
                onpress: move |_| {
                    router.replace(UplinkRoute::SettingsLayout {});
                }
            },
        }
    })
}
