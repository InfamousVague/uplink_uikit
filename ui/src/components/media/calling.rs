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

#[derive(PartialEq, Eq, Props, Clone)]
pub struct Props {
    in_chat: bool,
}

#[allow(non_snake_case)]
pub fn CallControl(props: Props) -> Element {
    let state = use_context::<Signal<State>>();
    match state().ui.call_info.active_call() {
        Some(call) => rsx!(ActiveCallControl {
            active_call: call,
            in_chat: props.in_chat,
            mute_text: get_local_text("remote-controls.mute"),
            unmute_text: get_local_text("remote-controls.unmute"),
            listen_text: get_local_text("remote-controls.listen"),
            silence_text: get_local_text("remote-controls.silence"),
            start_recording_text: get_local_text("remote-controls.start-recording"),
            stop_recording_text: get_local_text("remote-controls.stop-recording"),
        }),
        None => match state.read().ui.call_info.pending_calls().first() {
            Some(call) => rsx!(PendingCallDialog {
                call: call.clone(),
                in_chat: props.in_chat,
            }),
            None => rsx!({}),
        },
    }
}

#[derive(PartialEq, Eq, Props, Clone)]
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
fn ActiveCallControl(props: ActiveCallProps) -> Element {
    log::trace!("Rendering active call window");
    let state = use_context::<Signal<State>>();
    let active_call: ActiveCall = props.active_call.clone();
    let _active_call_id = active_call.call.id;
    let active_call_answer_time = active_call.answer_time;
    let scope_id = current_scope_id();
    let outgoing = active_call.call.participants_joined.is_empty();
    let update_fn = use_signal(schedule_update_any);

    let recording = use_signal(|| false);

    let scope_id_signal = use_signal(|| scope_id);
    let answer_time_signal = use_signal(|| active_call_answer_time);

    let _ = use_resource(move || async move {
        loop {
            let dur_sec = Duration::from_secs(1);
            let dur_min = Duration::from_secs(60);

            let to_sleep = match Local::now()
                .signed_duration_since(answer_time_signal())
                .to_std()
            {
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
            // HACK(Migration_0.5): Unwrap here doesn't bring a problem
            update_fn()(scope_id_signal().unwrap());
        }
    });

    let ch: Coroutine<CallDialogCmd> = use_coroutine(|mut rx| {
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
                if props.in_chat {
                    return rsx!({});
                }
            }
            Some(c) => {
                if active_call.call.conversation_id.eq(&c.id) != props.in_chat {
                    return rsx!({});
                }
            }
        };
    }

    let call = active_call.call.clone();
    let participants = state.read().get_identities_from_call(&call);
    let other_participants = state.read().remove_self(&participants);
    let participants_name = State::join_usernames(&other_participants);
    let self_id = build_user_from_identity(&state.read().get_own_identity());

    let other_participants_in_call = use_signal(|| other_participants.clone());

    use_effect(move || {
        to_owned![ch, state];
        {
            for id in other_participants_in_call() {
                if let Some(vol) = state.read().settings.user_volumes.get(&id.did_key()) {
                    ch.send(CallDialogCmd::AdjustVolume(Box::new(id.did_key()), *vol))
                }
            }
        }
    });

    rsx!(div {
        id: "remote-controls",
        aria_label: "remote-controls",
        class: format_args!("{}", if props.in_chat {"in-chat"} else {""}),
        {(*recording.read()).then(||{
            rsx!(
                div {
                    class: "recording-active",
                    aria_label: "recording-active",
                    common::icons::Icon {
                        ..common::icons::IconProps {
                            class: None,
                            size: 20,
                            fill:"currentColor".to_string(),
                            icon: Icon::RadioSelected,
                            disabled:  false,
                            disabled_fill: "#000000".to_string()
                        },
                    }
                }
            )
        })},
        div {
            class: format_args!("call-label {}", if props.in_chat {"in-chat"} else {""}),
            {outgoing.then(|| rsx!(Label {
                text: get_local_text("remote-controls.outgoing-call"),
                aria_label: "outgoing-call-label".to_string(),
            }))}
        }
        div {
            class: "call-info",
            aria_label: "call-info",
            div {
                class: format_args!("calling-users {}", if props.in_chat {"in-chat"} else {""}),
                if other_participants.is_empty() {
                    {rsx!(div {
                        class: "lonely-call",
                        aria_label: "lonely-call",
                        {get_local_text("remote-controls.empty")}
                    })}
                } else if props.in_chat {
                    {let call_participants: Vec<_> = other_participants
                        .iter()
                        .map(|x| (call.participants_speaking.contains_key(&x.did_key()), call.participants_joined.get(&x.did_key()).cloned(), build_user_from_identity(x)))
                        .collect();
                    rsx!(CallUserImageGroup {
                        participants: call_participants,
                    })}
                } else  {
                    {rsx!(UserImageGroup {
                        participants: build_participants(&other_participants),
                    })}
                }
            }
            {(!props.in_chat).then(||rsx!(
                p {
                    class: "call-name",
                    aria_label: "call-name",
                    "{participants_name}"
                }
            ))},
            p {
                class: format_args!("call-time {}", if props.in_chat {"in-chat"} else {""}),
                aria_label: "call-time",
                {format_timestamp_timeago(active_call.answer_time.into(), &state.read().settings.language_id())},
            },
            {props.in_chat.then(||rsx!(div {
                class: "self-identity",
                UserImage {
                    platform: self_id.platform,
                    status: self_id.status,
                    image: self_id.photo
                }
            }))}
        },
        div {
            class: "controls",
            aria_label: "call-controls",
            Button {
                icon: Icon::Microphone,
                aria_label: "call-mic-button".to_string(),
                appearance: if call.self_muted { Appearance::Danger } else { Appearance::Secondary },
                tooltip: rsx!(
                    Tooltip {
                        arrow_position: ArrowPosition::Bottom,
                        text: if call.self_muted { props.unmute_text.clone() } else { props.mute_text.clone() }
                    }
                ),
                onpress: move |_| {
                    if call.self_muted { ch.send(CallDialogCmd::UnmuteSelf); } else { ch.send(CallDialogCmd::MuteSelf); }
                }
            },
            Button {
                icon: if call.call_silenced { Icon::HeadphonesSlash } else { Icon::Headphones },
                aria_label: "call-speaker-button".to_string(),
                appearance: if call.call_silenced { Appearance::Danger } else { Appearance::Secondary },
                tooltip: rsx!(
                    Tooltip {
                        arrow_position: ArrowPosition::Bottom,
                        text: if call.call_silenced { props.listen_text.clone() } else { props.silence_text.clone() }
                    }
                ),
                onpress: move |_| {
                    if call.call_silenced { ch.send(CallDialogCmd::UnsilenceCall); } else { ch.send(CallDialogCmd::SilenceCall); }
                }
            },
            {(!outgoing).then(||{
                if *recording.read() {
                    rsx!(Button {
                        aria_label: "stop-recording-button".to_string(),
                        icon: Icon::StopCircle,
                        appearance: Appearance::Danger,
                        tooltip: rsx!(
                            Tooltip {
                                arrow_position: ArrowPosition::Bottom,
                                text: props.stop_recording_text.clone()
                        }
                      ),
                   onpress: move |_| {
                   ch.send(CallDialogCmd::StopRecording);
                    },
                  })
                } else {
                    rsx!(Button {
            aria_label: "start-recording-button".to_string(),
            icon: Icon::RadioSelected,
            appearance: Appearance::Secondary,
            tooltip: rsx!(
                Tooltip {
                    arrow_position: ArrowPosition::Bottom,
                    text: props.start_recording_text.clone()
                }
            ),
                        onpress: move |_| {
                        ch.send(CallDialogCmd::RecordCall);
               },
            })
         }
      })},
            Button {
                icon: Icon::PhoneXMark,
                aria_label: "call-hangup-button".to_string(),
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
    })
}

#[derive(PartialEq, Eq, Props, Clone)]
pub struct PendingCallProps {
    call: Call,
    in_chat: bool,
}

#[allow(non_snake_case)]
fn PendingCallDialog(props: PendingCallProps) -> Element {
    log::trace!("Rendering pending call window");
    let state = use_context::<Signal<State>>();
    let ch = use_coroutine(|mut rx| {
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

    let call = props.call.clone();
    if state.read().ui.current_layout == Layout::Compose {
        match state.read().get_active_chat() {
            None => {
                if props.in_chat {
                    return rsx!({});
                }
            }
            Some(c) => {
                if call.conversation_id.eq(&c.id) != props.in_chat {
                    return rsx!({});
                }
            }
        };
    }
    let alive = use_signal(|| Arc::new(AtomicBool::new(false)));
    use_effect(move || {
        to_owned![alive];
        spawn(async move { PlayUntil(ContinuousSound::RingTone, alive.read().clone()) });
    });
    let mut participants = state.read().get_identities_from_call(&call);
    participants = state.read().remove_self(&participants);
    let usernames = match state.read().get_chat_by_id(call.id) {
        Some(c) => match c.conversation_name {
            Some(name) => name,
            None => State::join_usernames(&participants),
        },
        None => State::join_usernames(&participants),
    };

    rsx!(CallDialog {
        caller: rsx!(UserImageGroup {
            participants: build_participants(&participants),
        },),
        in_chat: props.in_chat,
        usernames: usernames,
        icon: Icon::PhoneArrowDownLeft,
        description: get_local_text("remote-controls.incoming-call"),
        with_accept_btn: rsx!(Button {
            aria_label: "accept-call-button".to_string(),
            icon: Icon::Phone,
            appearance: Appearance::Success,
            onpress: move |_| {
                ch.send(PendingCallDialogCmd::Accept(call.id));
            }
        }),
        with_deny_btn: rsx!(Button {
            aria_label: "deny-call-button".to_string(),
            icon: Icon::PhoneXMark,
            appearance: Appearance::Danger,
            onpress: move |_| {
                ch.send(PendingCallDialogCmd::Reject(call.id));
            }
        }),
    })
}

#[derive(Props, Clone, PartialEq)]
pub struct CallDialogProps {
    caller: Element,
    icon: Icon,
    description: String,
    usernames: String,
    in_chat: bool,
    #[props(optional)]
    with_accept_btn: Option<Element>,
    #[props(optional)]
    with_deny_btn: Option<Element>,
}

// todo: remove this
#[allow(unused)]
#[allow(non_snake_case)]
pub fn CallDialog(props: CallDialogProps) -> Element {
    let with_accept_btn = match props.with_accept_btn.clone() {
        Some(w_a_b) => w_a_b,
        None => None,
    };
    let with_deny_btn = match props.with_deny_btn.clone() {
        Some(w_d_b) => w_d_b,
        None => None,
    };
    rsx! (
        div {
            class:format_args!("call-dialog {}", if props.in_chat {"in-chat"} else {""}),
            aria_label: format_args!("call-dialog-{}", if props.in_chat {"in-chat"} else {""}),
            div {
                class: "call-information",
                aria_label: "call-information",
                {rsx!(
                    common::icons::Icon {
                        ..common::icons::IconProps {
                            class: None,
                            size: 20,
                            fill:"currentColor".to_string(),
                            icon: props.icon,
                            disabled: false,
                            disabled_fill: "#9CA3AF".to_string()
                        },
                    },
                )}
                p {
                    aria_label: "incoming-call",
                    "{props.description}",
                },
            },
            div {
                aria_label: "calling-users",
                class: "calling-users",
                {&props.caller},
            },
            {(!props.in_chat).then(||rsx!(div {
                class: "users",
                class: "call-users",
                "{props.usernames}",
            }))}
            div {
                aria_label: "controls",
                class: "controls",
                {with_accept_btn},
                {with_deny_btn},
            }
        }
    )
}

#[derive(Props, Clone, PartialEq)]
pub struct CallUserImageProps {
    participants: Vec<(bool, Option<ParticipantState>, User)>,
}

#[allow(non_snake_case)]
pub fn CallUserImageGroup(props: CallUserImageProps) -> Element {
    let amount = use_signal(|| 3);
    let id = use_signal(Uuid::new_v4);
    use_effect(move || {
        to_owned![amount];
        spawn(async move {
            let mut eval_result = eval(include_str!("./resize_handler.js"));
            loop {
                match eval_result.recv().await {
                    Ok(value) => {
                        amount.set(value.as_f64().unwrap_or(3_f64) as i64);
                    }
                    Err(e) => {
                        log::error!("eval receiver failed: {:?}", e);
                    }
                }
            }
        });
    });
    let visible_amount = *amount.read() as usize;
    let (visible, context) = if visible_amount >= props.participants.len() {
        (props.participants.clone(), None)
    } else {
        let (visible, context) = props.participants.split_at(visible_amount.max(3) - 1);
        (visible.to_vec(), Some(context.to_vec()))
    };

    let user_state_icons = move |user_state: Option<ParticipantState>| {
        user_state.map(move |s| {
            rsx!(div {
                class: "call-status",
                {s.muted.then(||{
                    rsx!(div {
                        class: "call-status-icon",
                        IconElement {
                            icon: Icon::MicrophoneSlash,
                            fill:"currentColor",
                        }
                    })
                })},
                {s.deafened.then(||{
                    rsx!(div {
                        class: "call-status-icon",
                        IconElement {
                            icon: Icon::HeadphonesSlash,
                            fill:"currentColor",
                        }
                    })
                })},
                {s.recording.then(||{
                    rsx!(div {
                        class: "call-status-icon",
                        IconElement {
                            icon: Icon::VideoCamera,
                            fill:"currentColor",
                        }
                    })
                })}
            })
        })
    };

    rsx!(
        {
            visible.iter().map(|(speaking, user_state, user)| {
                rsx!(div {
                    class: format_args!("call-user {}", if *speaking {"speaking"} else {""}),
                    UserImage {
                        platform: user.platform,
                        image: user.photo.clone(),
                    }
                    {user_state_icons(user_state.clone())}
                })
            })
        },
        {
            context.map(|ctx| {
            let txt = format!("{}+", ctx.len());
            rsx!(
                div {
                    class: "additional-participants",
                    ContextMenu {
                        id: format!("{}", id),
                        left_click_trigger: true,
                        items: rsx!(
                            {ctx.iter().map(|(speaking, user_state, user)|{
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
                                           { user.username.to_string()}
                                        },
                                        {user_state_icons(user_state.clone())}
                                })
                            })}
                        ),
                        Button {
                            aria_label: "additional-participants-button".to_string(),
                            appearance: Appearance::Secondary,
                            text: txt,
                        }
                    },
                }
            )
        })
        },
    )
}
