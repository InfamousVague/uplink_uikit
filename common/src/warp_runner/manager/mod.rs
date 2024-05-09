//! this is the main warp_runner task. It initializes Warp and sits between Warp and Uplink, allowing communication via channels.

pub mod commands;
mod events;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Notify;

use warp::{
    blink::BlinkEventStream, multipass::MultiPassEventStream, raygun::RayGunEventStream,
    tesseract::Tesseract,
};

use tracing::log;

use super::{conv_stream, Account, Calling, Messaging, Storage};
use crate::WARP_CMD_CH;

pub use commands::{BlinkCmd, ConstellationCmd, MultiPassCmd, OtherCmd, RayGunCmd, TesseractCmd};

/// Contains the structs needed for run() to handle various events
pub struct Warp {
    pub tesseract: Tesseract,
    pub multipass: Account,
    pub raygun: Messaging,
    pub constellation: Storage,
    pub blink: Calling,
}

pub async fn run(mut warp: Warp, notify: Arc<Notify>) {
    // receive command from Uplink
    let warp_cmd_rx = WARP_CMD_CH.rx.clone();

    // using a mutex was the only way to get a mutable static variable. this channel should only be read here and only needs to be acquired once
    let mut warp_cmd_rx = warp_cmd_rx.lock().await;

    // gather incoming messages from all conversations and read them from conversation_msg_rx
    let (conversation_msg_tx, mut conversation_msg_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut conversation_manager = conv_stream::Manager::new(conversation_msg_tx.clone());

    // receive events from RayGun and MultiPass
    let mut raygun_stream = get_raygun_stream(&mut warp.raygun).await;
    let mut multipass_stream = get_multipass_stream(&mut warp.multipass).await;
    let mut blink_stream = get_blink_stream(&mut warp.blink).await;

    log::debug!("warp_runner::manager::run");
    loop {
        tokio::select! {
            opt = multipass_stream.next() => {
                if events::handle_multipass_event(opt, &mut warp).await.is_err() {
                    break;
                }
            },
            opt = raygun_stream.next() => {
                if events::handle_raygun_event(opt, &mut warp, &mut conversation_manager).await.is_err() {
                    break;
                }
            },
            opt = blink_stream.next() => {
                if let Some(evt) = opt {
                    if let Err(e) = events::handle_blink_event(evt, &mut warp).await {
                        log::error!("failed to handle blink event: {e}");
                        break;
                    }
                }
            },
            opt = conversation_msg_rx.recv() => {
                if events::handle_message_event(opt, &mut warp).await.is_err() {
                    break;
                }
            }
            opt = warp_cmd_rx.recv() => {
                if events::handle_warp_command(opt, &mut warp, &mut conversation_manager).await.is_err() {
                    break;
                }
            },
            // the WarpRunner has been dropped. stop the task
            _ = notify.notified() => break,
        }
    }

    log::debug!("terminating warp_runner thread");
}

async fn get_raygun_stream(rg: &mut Messaging) -> RayGunEventStream {
    loop {
        match rg.raygun_subscribe().await {
            Ok(stream) => break stream,
            Err(warp::error::Error::MultiPassExtensionUnavailable)
            | Err(warp::error::Error::RayGunExtensionUnavailable) => {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            Err(_e) => {
                //Should not reach this point but should handle an error if it does
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
}

async fn get_multipass_stream(account: &mut Account) -> MultiPassEventStream {
    loop {
        match account.multipass_subscribe().await {
            Ok(stream) => break stream,
            Err(e) => match e {
                //Note: Used as a precaution for future checks
                warp::error::Error::MultiPassExtensionUnavailable => {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                //TODO: log error
                //Note: Shouldnt give any other error but if it does to probably file as a bug
                _e => {
                    log::error!("failed to get multipass stream: {}", _e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            },
        };
    }
}

async fn get_blink_stream(blink: &mut Calling) -> BlinkEventStream {
    loop {
        match blink.get_event_stream().await {
            Ok(stream) => break stream,
            Err(_e) => {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
}
