use std::time::Duration;

use common::language::get_local_text;
use common::state::ToastNotification;
use common::warp_runner::{BlinkCmd, WarpCmd};
use dioxus::prelude::*;
use futures::{channel::oneshot, StreamExt};

use kit::elements::button::Button;
use kit::elements::select::Select;
use kit::elements::switch::Switch;
use tracing::log;
use warp::blink::AudioTestEvent;

use crate::components::settings::{SettingSection, SettingSectionSimple};
use common::state::{action::ConfigAction, Action, State};
use common::{sounds, WARP_CMD_CH};

// pub const VOL_MIN: f32 = 0.0;
// pub const VOL_MAX: f32 = 200.0;
pub const MAX_VOLUME: f32 = 127_f32 * 0.01;

enum AudioCmd {
    FetchOutputDevices,
    SetOutputDevice(String),
    FetchInputDevices,
    SetInputDevice(String),
    SetEchoCancellation(bool),
    TestSpeaker,
    TestMicrophone,
}

#[allow(non_snake_case)]
pub fn AudioSettings(cx: Scope) -> Element {
    log::trace!("Audio settings page rendered.");
    let state = use_shared_state::<State>(cx)?;
    let input_devices = use_ref(cx, Vec::new);
    let output_devices = use_ref(cx, Vec::new);

    let speaker_volume = use_ref(cx, || 0);
    let microphone_volume = use_ref(cx, || 0);

    let ch = use_coroutine(cx, |mut rx| {
        to_owned![
            state,
            input_devices,
            output_devices,
            speaker_volume,
            microphone_volume
        ];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();

            'GET_AUDIO_CONFIG: loop {
                let audio_config = {
                    let (tx, rx) = oneshot::channel();
                    warp_cmd_tx
                        .send(WarpCmd::Blink(BlinkCmd::GetAudioDeviceConfig { rsp: tx }))
                        .expect("failed to send command");
                    match rx.await.expect("warp runner failed to get audio config") {
                        Ok(r) => r,
                        Err(e) => {
                            log::debug!("failed to get audio config: {e}");
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            continue;
                        }
                    }
                };

                while let Some(cmd) = rx.next().await {
                    match cmd {
                        AudioCmd::SetEchoCancellation(flag) => {
                            let (tx, rx) = oneshot::channel();
                            if let Err(_e) =
                                warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::SetEchoCancellation {
                                    flag,
                                    rsp: tx,
                                }))
                            {
                                log::error!("failed to send blink command");
                                continue;
                            }

                            match rx.await {
                                Ok(_) => {}
                                Err(e) => {
                                    log::error!("warp_runner failed to set echo cancellation: {e}");
                                    state.write().mutate(Action::Config(
                                        ConfigAction::SetEchoCancellation(!flag),
                                    ));
                                    state
                                        .write()
                                        .mutate(common::state::Action::AddToastNotification(
                                        ToastNotification::init(
                                            get_local_text("warning-messages.error"),
                                            get_local_text("settings-audio.failed"),
                                            Some(
                                                common::icons::outline::Shape::ExclamationTriangle,
                                            ),
                                            2,
                                        ),
                                    ));
                                }
                            }
                        }
                        AudioCmd::FetchInputDevices => {
                            state.write_silent().settings.input_device =
                                audio_config.microphone_device_name();
                            *input_devices.write() =
                                audio_config.get_available_microphones().unwrap_or_default();
                        }
                        AudioCmd::FetchOutputDevices => {
                            state.write_silent().settings.output_device =
                                audio_config.speaker_device_name();
                            *output_devices.write() =
                                audio_config.get_available_speakers().unwrap_or_default();
                        }
                        AudioCmd::SetInputDevice(device_name) => {
                            let device = device_name.clone();
                            let (tx, rx) = oneshot::channel();
                            if let Err(_e) =
                                warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::SetMicrophone {
                                    device_name,
                                    rsp: tx,
                                }))
                            {
                                log::error!("failed to send blink command");
                                continue;
                            }

                            match rx.await {
                                Ok(_) => {
                                    state.write_silent().settings.input_device = Some(device);
                                    continue 'GET_AUDIO_CONFIG;
                                }
                                Err(e) => {
                                    log::error!("warp_runner failed to set input device: {e}");
                                }
                            }
                        }
                        AudioCmd::SetOutputDevice(device_name) => {
                            let device = device_name.clone();
                            let (tx, rx) = oneshot::channel();
                            if let Err(_e) =
                                warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::SetSpeaker {
                                    device_name,
                                    rsp: tx,
                                }))
                            {
                                log::error!("failed to send blink command");
                                continue;
                            }

                            match rx.await {
                                Ok(_) => {
                                    state.write_silent().settings.output_device = Some(device);
                                    continue 'GET_AUDIO_CONFIG;
                                }
                                Err(e) => {
                                    log::error!("warp_runner failed to set output device: {e}");
                                }
                            }
                        }
                        AudioCmd::TestSpeaker => {
                            let (tx, rx) = oneshot::channel();
                            if let Err(_e) =
                                warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::TestSpeaker { rsp: tx }))
                            {
                                log::error!("failed to send blink command");
                                continue;
                            }

                            let mut ch = match rx.await {
                                Ok(ch) => ch,
                                Err(_) => {
                                    log::error!("warp_runner failed to test speaker");
                                    continue;
                                }
                            };

                            while let Some(evt) = ch.recv().await {
                                match evt {
                                    AudioTestEvent::Done => {
                                        log::info!("audio test done");
                                        break;
                                    }
                                    AudioTestEvent::Output { loudness } => {
                                        *speaker_volume.write() =
                                            (loudness as f32 / MAX_VOLUME) as u8;
                                    }
                                    _ => {}
                                }
                            }
                            *speaker_volume.write() = 0;
                        }
                        AudioCmd::TestMicrophone => {
                            let (tx, rx) = oneshot::channel();
                            if let Err(_e) = warp_cmd_tx
                                .send(WarpCmd::Blink(BlinkCmd::TestMicrophone { rsp: tx }))
                            {
                                log::error!("failed to send blink command");
                                continue;
                            }

                            let mut ch = match rx.await {
                                Ok(ch) => ch,
                                Err(_) => {
                                    log::error!("warp_runner failed to test microphone");
                                    continue;
                                }
                            };

                            while let Some(evt) = ch.recv().await {
                                match evt {
                                    AudioTestEvent::Done => {
                                        log::info!("audio test done");
                                        break;
                                    }
                                    AudioTestEvent::Output { loudness } => {
                                        *microphone_volume.write() =
                                            (loudness as f32 / MAX_VOLUME) as u8;
                                    }
                                    _ => {}
                                }
                            }
                            *microphone_volume.write() = 0;
                        }
                    }
                }
                break;
            }
        }
    });

    use_future(cx, (), |_| {
        to_owned![ch];
        async move {
            loop {
                ch.send(AudioCmd::FetchInputDevices);
                ch.send(AudioCmd::FetchOutputDevices);
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        }
    });

    cx.render(rsx!(
        div {
            id: "settings-audio",
            aria_label: "settings-audio",
            SettingSection {
                aria_label: "input-device-section".into(),
                section_label: get_local_text("settings-audio.input-device"),
                section_description: get_local_text("settings-audio.input-device-description"),
                Select {
                    initial_value: state.read().settings.input_device.as_ref().cloned().unwrap_or("default".into()),
                    options: input_devices.read().clone(),
                    onselect: move |device| {
                        ch.send(AudioCmd::SetInputDevice(device))
                    }
                },
            },
            // SettingSectionSimple {
            //     Range {
            //         aria_label: "range-input-device".into(),
            //         initial_value: 100.0,
            //         min: VOL_MIN,
            //         max: VOL_MAX,
            //         icon_left: Shape::Microphone,
            //         icon_right: Shape::MicrophoneWave,
            //         disabled: true,
            //         onchange: move |_| {}
            //     }
            // }
            SettingSectionSimple {
                aria_label: "test-device-input-section".into(),
                Button {
                    text: get_local_text("settings-audio.device-test"),
                    disabled: false,
                    onpress: move |_| {
                        ch.send(AudioCmd::TestMicrophone);
                    },
                },
                VolumeIndicator {
                    volume: microphone_volume.clone(),
                }
            },
            SettingSection {
                aria_label: "output-device-section".into(),
                section_label: get_local_text("settings-audio.output-device"),
                section_description: get_local_text("settings-audio.output-device-description"),
                Select {
                    initial_value: state.read().settings.output_device.as_ref().cloned().unwrap_or("default".into()),
                    options: output_devices.read().clone(),
                    onselect: move |device| {
                        ch.send(AudioCmd::SetOutputDevice(device));
                    }
                },
            },
            // SettingSectionSimple {
            //     Range {
            //         aria_label: "range-output-device".into(),
            //         initial_value: 100.0,
            //         min: VOL_MIN,
            //         max: VOL_MAX,
            //         icon_left: Shape::Speaker,
            //         icon_right: Shape::SpeakerWave,
            //         disabled: true,
            //         onchange: move |_| {}
            //     }
            // }
            SettingSectionSimple {
                aria_label: "test-device-output-section".into(),
                Button {
                    text: get_local_text("settings-audio.device-test"),
                    disabled: false,
                    onpress: move |_| {
                        ch.send(AudioCmd::TestSpeaker);
                    },
                },
                VolumeIndicator {
                    volume: speaker_volume.clone(),
                }
            },

            // currently does nothing
            //SettingSection {
            //    section_label: get_local_text("settings-audio.sample-rate"),
            //    section_description: get_local_text("settings-audio.sample-rate-description"),
            //    Select {
            //        initial_value: "48000 Hz".into(),
            //        options: vec!["24000 Hz".into(), "48000 Hz".into(), "96000 Hz".into()],
            //        onselect: move |_| {}
            //    },
            //},

            // currently not implemented
            //SettingSection {
            //    section_label: get_local_text("settings-audio.noise-suppression"),
            //    section_description: get_local_text("settings-audio.noise-suppression-description"),
            //    no_border: true,
            //},
            //SettingSectionSimple {
            //    RadioList {
            //        initial_value: "None".into(),
            //        values: vec!["None".into(), "Low".into(), "Medium".into(), "High".into()],
            //        onchange: move |_| {}
            //    },
            //}

            SettingSection {
                aria_label: "echo-cancellation-section".into(),
                section_label: get_local_text("settings-audio.echo-cancellation"),
                section_description: get_local_text("settings-audio.echo-cancellation-description"),
                Switch {
                    active: state.read().configuration.audiovideo.echo_cancellation,
                    onflipped: move |e| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            sounds::Play(sounds::Sounds::Flip);
                        }
                        ch.send(AudioCmd::SetEchoCancellation(e));
                        state.write().mutate(Action::Config(
                            ConfigAction::SetEchoCancellation(e),
                        ));
                    }
                }
            },

            SettingSection {
                aria_label: "interface-sounds-section".into(),
                section_label: get_local_text("settings-audio.interface-sounds"),
                section_description: get_local_text("settings-audio.interface-sounds-description"),
                Switch {
                    active: state.read().configuration.audiovideo.interface_sounds,
                    onflipped: move |e| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            sounds::Play(sounds::Sounds::Flip);
                        }
                        state.write().mutate(Action::Config(ConfigAction::SetInterfaceSoundsEnabled(e)));
                    }
                }
            },
            SettingSection {
                aria_label: "media-sounds-section".into(),
                section_label: get_local_text("settings-audio.media-sounds"),
                section_description: get_local_text("settings-audio.media-sounds-description"),
                Switch {
                    active: state.read().configuration.audiovideo.media_sounds,
                    onflipped: move |e| {
                        if state.read().configuration.audiovideo.interface_sounds {
                           sounds::Play(sounds::Sounds::Flip);
                        }
                        state.write().mutate(Action::Config(ConfigAction::SetMediaSoundsEnabled(e)));
                    }
                }
            },
            SettingSection {
                aria_label: "message-sounds-section".into(),
                section_label: get_local_text("settings-audio.message-sounds"),
                section_description: get_local_text("settings-audio.message-sounds-description"),
                Switch {
                    active: state.read().configuration.audiovideo.message_sounds,
                    onflipped: move |e| {
                        if state.read().configuration.audiovideo.interface_sounds {
                            sounds::Play(sounds::Sounds::Flip);
                        }
                        state.write().mutate(Action::Config(ConfigAction::SetMessageSoundsEnabled(e)));
                    }
                }
            },
            SettingSection {
                aria_label: "call-timer-section".into(),
                section_label: get_local_text("settings-audio.call-timer"),
                section_description: get_local_text("settings-audio.call-timer-description"),
                Switch {
                    active: state.read().ui.call_timer,
                    onflipped: move |e| {
                        state.write().ui.call_timer = e;
                    }
                }
            }
        }
    ))
}

#[derive(Props, PartialEq)]
pub struct VolumeIndicatorProps {
    volume: UseRef<u8>,
}

pub fn VolumeIndicator(cx: Scope<VolumeIndicatorProps>) -> Element {
    cx.render(rsx!(div{
        class: "volume-indicator-wrap",
        div {
            class: "volume-indicator volume-indicator-overlay",
            z_index: 2,
            width: format_args!("{}%", 100 - *cx.props.volume.read())
        },
        div {
            class: "volume-indicator",
            z_index: 1,
        }
    }))
}
