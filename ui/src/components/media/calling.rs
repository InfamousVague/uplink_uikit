use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use chrono::Local;
use common::icons::Icon as IconElement;
use dioxus::prelude::*;

use dioxus_core::Element;
use futures::{channel::oneshot, StreamExt};
use kit::{
    components::{
        context_menu::ContextMenu, user_image::UserImage, user_image_group::UserImageGroup,
    },
    elements::{
        button::Button,
        label::Label,
        tooltip::{ArrowPosition, Tooltip},
        Appearance,
    },
    User,
};
use warp::{blink::ParticipantState, crypto::DID};

use crate::utils::{
    build_participants, build_user_from_identity, format_timestamp::format_timestamp_timeago,
};
use common::{
    icons::outline::Shape as Icon,
    sounds::{ContinuousSound, PlayUntil},
    state::{
        call::{ActiveCall, Call},
        ui::Layout,
    },
    warp_runner::{BlinkCmd, WarpCmd},
    STATIC_ARGS, WARP_CMD_CH,
};
use common::{
    language::get_local_text,
    state::{Action, State},
};
use uuid::Uuid;

pub enum CallDialogCmd {
    Hangup(Uuid),
    MuteSelf,
    UnmuteSelf,
    AdjustVolume(Box<DID>, f32),
    RecordCall,
    StopRecording,
    SilenceCall,
    UnsilenceCall,
}

enum PendingCallDialogCmd {
    Accept(Uuid),
    Reject(Uuid),
}

#[derive(PartialEq, Eq, Props)]
pub struct Props {
    in_chat: bool,
}

#[allow(non_snake_case)]
pub fn CallControl(cx: Scope<Props>) -> Element {
    let state = use_shared_state::<State>(cx)?;
    match state.read().ui.call_info.active_call() {
        Some(call) => cx.render(rsx!(ActiveCallControl {
            active_call: call,
            in_chat: cx.props.in_chat,
            mute_text: get_local_text("remote-controls.mute"),
            unmute_text: get_local_text("remote-controls.unmute"),
            listen_text: get_local_text("remote-controls.listen"),
            silence_text: get_local_text("remote-controls.silence"),
            start_recording_text: get_local_text("remote-controls.start-recording"),
            stop_recording_text: get_local_text("remote-controls.stop-recording"),
        })),
        None => match state.read().ui.call_info.pending_calls().first() {
            Some(call) => cx.render(rsx!(PendingCallDialog {
                call: call.clone(),
                in_chat: cx.props.in_chat,
            })),
            None => cx.render(rsx!(())),
        },
    }
}

#[derive(PartialEq, Eq, Props)]
pub struct ActiveCallProps {
    active_call: ActiveCall,
    in_chat: bool,
    mute_text: String,
    unmute_text: String,
    listen_text: String,
    silence_text: String,
    start_recording_text: String,
    stop_recording_text: String,
}

#[allow(non_snake_case)]
fn ActiveCallControl(cx: Scope<ActiveCallProps>) -> Element {
    log::trace!("Rendering active call window");
    let state = use_shared_state::<State>(cx)?;
    let active_call: &ActiveCall = &cx.props.active_call;
    let active_call_id = active_call.call.id;
    let active_call_answer_time = active_call.answer_time;
    let scope_id = cx.scope_id();
    let outgoing = active_call.call.participants_joined.is_empty();
    let update_fn = cx.schedule_update_any();

    let recording = use_ref(cx, || false);

    use_future(
        cx,
        (&scope_id, &active_call_id, &active_call_answer_time),
        |(scope_id, _, answer_time)| async move {
            loop {
                let dur_sec = Duration::from_secs(1);
                let dur_min = Duration::from_secs(60);

                let to_sleep = match Local::now().signed_duration_since(answer_time).to_std() {
                    Ok(duration) => {
                        if duration < dur_min {
                            dur_sec
                        } else {
                            dur_min
                        }
                    }
                    Err(_) => dur_sec,
                };

                tokio::time::sleep(to_sleep).await;
                update_fn(scope_id);
            }
        },
    );

    let ch: &Coroutine<CallDialogCmd> = use_coroutine(cx, |mut rx| {
        to_owned![state, recording];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    CallDialogCmd::Hangup(_id) => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::LeaveCall { rsp: tx }))
                        {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                state.write().mutate(Action::EndCall);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to answer call: {e}");
                            }
                        }
                    }
                    CallDialogCmd::MuteSelf => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::MuteSelf { rsp: tx }))
                        {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                // disaster waiting to happen if State ever gets out of sync with blink.
                                state.write().mutate(Action::ToggleMute);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to mute self: {e}");
                            }
                        }
                    }
                    CallDialogCmd::UnmuteSelf => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::UnmuteSelf { rsp: tx }))
                        {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                // disaster waiting to happen if State ever gets out of sync with blink.
                                state.write().mutate(Action::ToggleMute);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to unmute self: {e}");
                            }
                        }
                    }
                    CallDialogCmd::SilenceCall => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::SilenceCall { rsp: tx }))
                        {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                // disaster waiting to happen if State ever gets out of sync with blink.
                                state.write().mutate(Action::ToggleSilence);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to silence call: {e}");
                            }
                        }
                    }
                    CallDialogCmd::UnsilenceCall => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::UnsilenceCall { rsp: tx }))
                        {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                // disaster waiting to happen if State ever gets out of sync with blink.
                                state.write().mutate(Action::ToggleSilence);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to unsilence call: {e}");
                            }
                        }
                    }
                    CallDialogCmd::RecordCall => {
                        let (tx, rx) = oneshot::channel();
                        let time = Local::now().format("%d-%m-%Y_%H-%M-%S").to_string();
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::StartRecording {
                            output_dir: STATIC_ARGS
                                .recordings
                                .join(time)
                                .to_string_lossy()
                                .to_string(),
                            rsp: tx,
                        })) {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                recording.with_mut(|v| *v = true);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to start recording: {e}");
                            }
                        }
                    }
                    CallDialogCmd::StopRecording => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::StopRecording { rsp: tx }))
                        {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                recording.with_mut(|v| *v = false);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to stop recording: {e}");
                            }
                        }
                    }
                    CallDialogCmd::AdjustVolume(user, volume) => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::AdjustVolume {
                            user: *user.clone(),
                            volume,
                            rsp: tx,
                        })) {
                            log::error!("failed to send blink command: {e}");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                state
                                    .write_silent()
                                    .settings
                                    .user_volumes
                                    .insert(*user, volume);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to adjust voluem: {e}");
                            }
                        }
                    } // TODO: Method to end call before a connection is made
                }
            }
        }
    });
    if state.read().ui.current_layout == Layout::Compose {
        match state.read().get_active_chat() {
            None => {
                if cx.props.in_chat {
                    return cx.render(rsx!(()));
                }
            }
            Some(c) => {
                if active_call.call.conversation_id.eq(&c.id) != cx.props.in_chat {
                    return cx.render(rsx!(()));
                }
            }
        };
    }

    let call = &active_call.call;
    let participants = state.read().get_identities_from_call(call);
    let other_participants = state.read().remove_self(&participants);
    let participants_name = State::join_usernames(&other_participants);
    let self_id = build_user_from_identity(&state.read().get_own_identity());

    use_effect(cx, &other_participants, |in_call| {
        to_owned![ch, state];
        async move {
            for id in in_call {
                if let Some(vol) = state.read().settings.user_volumes.get(&id.did_key()) {
                    ch.send(CallDialogCmd::AdjustVolume(Box::new(id.did_key()), *vol))
                }
            }
        }
    });

    cx.render(rsx!(div {
        id: "remote-controls",
        aria_label: "remote-controls",
        class: format_args!("{}", if cx.props.in_chat {"in-chat"} else {""}),
        (*recording.read()).then(||{
            rsx!(
                div {
                    class: "recording-active",
                    aria_label: "recording-active",
                    common::icons::Icon {
                        ..common::icons::IconProps {
                            class: None,
                            size: 20,
                            fill:"currentColor",
                            icon: Icon::RadioSelected,
                            disabled:  false,
                            disabled_fill: "#000000"
                        },
                    }
                }
            )
        }),
        div {
            class: format_args!("call-label {}", if cx.props.in_chat {"in-chat"} else {""}),
            outgoing.then(|| rsx!(Label {
                text: get_local_text("remote-controls.outgoing-call"),
                aria_label: "outgoing-call-label".into(),
            }))
        }
        div {
            class: "call-info",
            aria_label: "call-info",
            div {
                class: format_args!("calling-users {}", if cx.props.in_chat {"in-chat"} else {""}),
                if other_participants.is_empty() {
                    rsx!(div {
                        class: "lonely-call",
                        aria_label: "lonely-call",
                        get_local_text("remote-controls.empty")
                    })
                } else if cx.props.in_chat {
                    let call_participants: Vec<_> = other_participants
                        .iter()
                        .map(|x| (call.participants_speaking.contains_key(&x.did_key()), call.participants_joined.get(&x.did_key()).cloned(), build_user_from_identity(x)))
                        .collect();
                    rsx!(CallUserImageGroup {
                        participants: call_participants,
                    })
                } else  {
                    rsx!(UserImageGroup {
                        participants: build_participants(&other_participants),
                    })
                }
            }
            (!cx.props.in_chat).then(||rsx!(
                p {
                    class: "call-name",
                    aria_label: "call-name",
                    "{participants_name}"
                }
            )),
            state.read().ui.call_timer.then(||rsx!(
                p {
                    class: format_args!("call-time {}", if cx.props.in_chat {"in-chat"} else {""}),
                    aria_label: "call-time",
                    format_timestamp_timeago(active_call.answer_time.into(), &state.read().settings.language_id()),
                }
            )),
            cx.props.in_chat.then(||rsx!(div {
                class: "self-identity",
                UserImage {
                    platform: self_id.platform,
                    status: self_id.status,
                    image: self_id.photo
                }
            }))
        },
        div {
            class: "controls",
            aria_label: "call-controls",
            Button {
                icon: Icon::Microphone,
                aria_label: "call-mic-button".into(),
                appearance: if call.self_muted { Appearance::Danger } else { Appearance::Secondary },
                tooltip: cx.render(rsx!(
                    Tooltip {
                        arrow_position: ArrowPosition::Bottom,
                        text: if call.self_muted { cx.props.unmute_text.clone() } else { cx.props.mute_text.clone() }
                    }
                )),
                onpress: move |_| {
                    if call.self_muted { ch.send(CallDialogCmd::UnmuteSelf); } else { ch.send(CallDialogCmd::MuteSelf); }
                }
            },
            Button {
                icon: if call.call_silenced { Icon::HeadphonesSlash } else { Icon::Headphones },
                aria_label: "call-speaker-button".into(),
                appearance: if call.call_silenced { Appearance::Danger } else { Appearance::Secondary },
                tooltip: cx.render(rsx!(
                    Tooltip {
                        arrow_position: ArrowPosition::Bottom,
                        text: if call.call_silenced { cx.props.listen_text.clone() } else { cx.props.silence_text.clone() }
                    }
                )),
                onpress: move |_| {
                    if call.call_silenced { ch.send(CallDialogCmd::UnsilenceCall); } else { ch.send(CallDialogCmd::SilenceCall); }
                }
            },
            (!outgoing).then(||{
                if *recording.read() {
                    rsx!(Button {
                        aria_label: "stop-recording-button".into(),
                        icon: Icon::StopCircle,
                        appearance: Appearance::Danger,
                        tooltip: cx.render(rsx!(
                            Tooltip {
                                arrow_position: ArrowPosition::Bottom,
                                text: cx.props.stop_recording_text.clone()
                        }
                      )),
                   onpress: move |_| {
                   ch.send(CallDialogCmd::StopRecording);
                    },
                  })
                } else {
                    rsx!(Button {
            aria_label: "start-recording-button".into(),
            icon: Icon::RadioSelected,
            appearance: Appearance::Secondary,
            tooltip: cx.render(rsx!(
                Tooltip {
                    arrow_position: ArrowPosition::Bottom,
                    text: cx.props.start_recording_text.clone()
                }
            )),
                        onpress: move |_| {
                        ch.send(CallDialogCmd::RecordCall);
               },
            })
         }
      }),
            Button {
                icon: Icon::PhoneXMark,
                aria_label: "call-hangup-button".into(),
                appearance: Appearance::Danger,
                onpress: move |_| {
                    ch.send(CallDialogCmd::Hangup(call.id));
                },
            },
            //Currently not implemented
            /*Button {
                icon: Icon::Cog6Tooth,
                appearance: Appearance::Secondary,
                onpress: move |_| {
                    //TODO
                },
            }*/

        }
    }))
}

#[derive(PartialEq, Eq, Props)]
pub struct PendingCallProps {
    call: Call,
    in_chat: bool,
}

#[allow(non_snake_case)]
fn PendingCallDialog(cx: Scope<PendingCallProps>) -> Element {
    log::trace!("Rendering pending call window");
    let state = use_shared_state::<State>(cx)?;
    let ch = use_coroutine(cx, |mut rx| {
        to_owned![state];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    PendingCallDialogCmd::Accept(id) => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(_e) = warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::AnswerCall {
                            call_id: id,
                            rsp: tx,
                        })) {
                            log::error!("failed to send blink command");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                state.write().mutate(Action::AnswerCall(id));
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to answer call: {e}");
                            }
                        }
                    }
                    PendingCallDialogCmd::Reject(id) => {
                        let (tx, rx) = oneshot::channel();
                        if let Err(_e) = warp_cmd_tx.send(WarpCmd::Blink(BlinkCmd::RejectCall {
                            call_id: id,
                            rsp: tx,
                        })) {
                            log::error!("failed to send blink command");
                            continue;
                        }

                        match rx.await {
                            Ok(_) => {
                                state.write().ui.call_info.reject_call(id);
                            }
                            Err(e) => {
                                log::error!("warp_runner failed to answer call: {e}");
                            }
                        }
                    }
                }
            }
        }
    });

    let call = &cx.props.call;
    if state.read().ui.current_layout == Layout::Compose {
        match state.read().get_active_chat() {
            None => {
                if cx.props.in_chat {
                    return cx.render(rsx!(()));
                }
            }
            Some(c) => {
                if call.conversation_id.eq(&c.id) != cx.props.in_chat {
                    return cx.render(rsx!(()));
                }
            }
        };
    }
    let alive = use_ref(cx, || Arc::new(AtomicBool::new(false)));
    use_effect(cx, (), |_| {
        to_owned![alive];
        async move { PlayUntil(ContinuousSound::RingTone, alive.read().clone()) }
    });
    let mut participants = state.read().get_identities_from_call(call);
    participants = state.read().remove_self(&participants);
    let usernames = match state.read().get_chat_by_id(call.id) {
        Some(c) => match c.conversation_name {
            Some(name) => name,
            None => State::join_usernames(&participants),
        },
        None => State::join_usernames(&participants),
    };

    cx.render(rsx!(CallDialog {
        caller: cx.render(rsx!(UserImageGroup {
            participants: build_participants(&participants),
        },)),
        in_chat: cx.props.in_chat,
        usernames: usernames,
        icon: Icon::PhoneArrowDownLeft,
        description: get_local_text("remote-controls.incoming-call"),
        with_accept_btn: cx.render(rsx!(Button {
            aria_label: "accept-call-button".into(),
            icon: Icon::Phone,
            appearance: Appearance::Success,
            onpress: move |_| {
                ch.send(PendingCallDialogCmd::Accept(call.id));
            }
        })),
        with_deny_btn: cx.render(rsx!(Button {
            aria_label: "deny-call-button".into(),
            icon: Icon::PhoneXMark,
            appearance: Appearance::Danger,
            onpress: move |_| {
                ch.send(PendingCallDialogCmd::Reject(call.id));
            }
        })),
    }))
}

#[derive(Props)]
pub struct CallDialogProps<'a> {
    caller: Element<'a>,
    icon: Icon,
    description: String,
    usernames: String,
    in_chat: bool,
    #[props(optional)]
    with_accept_btn: Option<Element<'a>>,
    #[props(optional)]
    with_deny_btn: Option<Element<'a>>,
}

// todo: remove this
#[allow(unused)]
#[allow(non_snake_case)]
pub fn CallDialog<'a>(cx: Scope<'a, CallDialogProps<'a>>) -> Element<'a> {
    let with_accept_btn = match cx.props.with_accept_btn.clone() {
        Some(w_a_b) => w_a_b,
        None => None,
    };
    let with_deny_btn = match cx.props.with_deny_btn.clone() {
        Some(w_d_b) => w_d_b,
        None => None,
    };
    cx.render(rsx! (
        div {
            class:format_args!("call-dialog {}", if cx.props.in_chat {"in-chat"} else {""}),
            aria_label: format_args!("call-dialog-{}", if cx.props.in_chat {"in-chat"} else {""}),
            div {
                class: "call-information",
                aria_label: "call-information",
                rsx!(
                    common::icons::Icon {
                        ..common::icons::IconProps {
                            class: None,
                            size: 20,
                            fill:"currentColor",
                            icon: cx.props.icon,
                            disabled: false,
                            disabled_fill: "#9CA3AF"
                        },
                    },
                )
                p {
                    aria_label: "incoming-call",
                    "{cx.props.description}",
                },
            },
            div {
                aria_label: "calling-users",
                class: "calling-users",
                &cx.props.caller,
            },
            (!cx.props.in_chat).then(||rsx!(div {
                class: "users",
                class: "call-users",
                "{cx.props.usernames}",
            }))
            div {
                aria_label: "controls",
                class: "controls",
                with_accept_btn,
                with_deny_btn,
            }
        }
    ))
}

#[derive(Props, PartialEq)]
pub struct CallUserImageProps {
    participants: Vec<(bool, Option<ParticipantState>, User)>,
}

#[allow(non_snake_case)]
pub fn CallUserImageGroup(cx: Scope<CallUserImageProps>) -> Element {
    let eval = use_eval(cx);
    let amount = use_state(cx, || 3);
    let id = use_state(cx, Uuid::new_v4);
    use_effect(cx, (), move |_| {
        to_owned![eval, amount];
        async move {
            let eval = match eval(include_str!("./resize_handler.js")) {
                Ok(r) => r,
                Err(e) => {
                    log::error!("use eval failed: {:?}", e);
                    return;
                }
            };
            loop {
                match eval.recv().await {
                    Ok(value) => {
                        amount.set(value.as_f64().unwrap_or(3_f64) as i64);
                    }
                    Err(e) => {
                        log::error!("eval receiver failed: {:?}", e);
                    }
                }
            }
        }
    });
    let visible_amount = *amount.get() as usize;
    let (visible, context) = if visible_amount >= cx.props.participants.len() {
        (cx.props.participants.clone(), None)
    } else {
        let (visible, context) = cx.props.participants.split_at(visible_amount.max(3) - 1);
        (visible.to_vec(), Some(context.to_vec()))
    };

    let user_state_icons = move |user_state: Option<ParticipantState>| {
        user_state.map(move |s| {
            rsx!(div {
                class: "call-status",
                s.muted.then(||{
                    rsx!(div {
                        class: "call-status-icon",
                        IconElement {
                            icon: Icon::MicrophoneSlash,
                            fill:"currentColor",
                        }
                    })
                }),
                s.deafened.then(||{
                    rsx!(div {
                        class: "call-status-icon",
                        IconElement {
                            icon: Icon::HeadphonesSlash,
                            fill:"currentColor",
                        }
                    })
                }),
                s.recording.then(||{
                    rsx!(div {
                        class: "call-status-icon",
                        IconElement {
                            icon: Icon::VideoCamera,
                            fill:"currentColor",
                        }
                    })
                })
            })
        })
    };

    cx.render(rsx!(
        visible.iter().map(|(speaking, user_state, user)| {
            rsx!(div {
                class: format_args!("call-user {}", if *speaking {"speaking"} else {""}),
                UserImage {
                    platform: user.platform,
                    image: user.photo.clone(),
                }
                user_state_icons(user_state.clone())
            })
        }),
        context.map(|ctx| {
            let txt = format!("{}+", ctx.len());
            rsx!(
                div {
                    class: "additional-participants",
                    ContextMenu {
                        id: format!("{}", id),
                        left_click_trigger: true,
                        items: cx.render(rsx!(
                            ctx.iter().map(|(speaking, user_state, user)|{
                                rsx!(div {
                                        class: "additional-participant",
                                        div {
                                            class: format_args!("{}", if *speaking {"speaking"} else {""}),
                                            UserImage {
                                                platform: user.platform,
                                                image: user.photo.clone(),
                                            }
                                        },
                                        p {
                                            class: "additional-participant-name",
                                            user.username.to_string()
                                        },
                                        user_state_icons(user_state.clone())
                                })
                            })
                        )),
                        Button {
                            aria_label: "additional-participants-button".to_string(),
                            appearance: Appearance::Secondary,
                            text: txt,
                        }
                    },
                }
            )
        }),
    ))
}
