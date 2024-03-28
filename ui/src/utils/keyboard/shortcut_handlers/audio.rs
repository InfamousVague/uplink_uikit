use common::state::{Action, State};
use common::{
    warp_runner::{BlinkCmd, WarpCmd},
    WARP_CMD_CH,
};
use dioxus::prelude::*;
use futures::channel::oneshot;
use futures::StreamExt;

use crate::components::media::calling::CallDialogCmd;

pub enum ToggleType {
    Deafen,
    Mute,
}

pub fn toggle(state: UseSharedState<State>, cx: Scope, toggle_type: ToggleType) {
    let call_state = match state.read().ui.call_info.active_call() {
        Some(c) => c.call,
        None => {
            log::error!("call not in progress");
            return;
        }
    };

    let ch: &Coroutine<CallDialogCmd> = use_coroutine(cx, |mut rx| {
        to_owned![state];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
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
                    _ => {}
                }
            }
        }
    });

    match toggle_type {
        ToggleType::Deafen => match call_state.call_silenced {
            true => ch.send(CallDialogCmd::UnsilenceCall),
            false => ch.send(CallDialogCmd::SilenceCall),
        },
        ToggleType::Mute => match call_state.self_muted {
            true => ch.send(CallDialogCmd::UnmuteSelf),
            false => ch.send(CallDialogCmd::MuteSelf),
        },
    }
}
