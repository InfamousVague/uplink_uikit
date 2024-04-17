use std::sync::Arc;

use futures::channel::oneshot;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use tracing::log;
use warp::crypto::DID;

use crate::{
    state::Identity,
    warp_runner::{MultiPassCmd, WarpCmd},
    WARP_CMD_CH,
};

pub struct ProfileUpdateChannel {
    pub tx: tokio::sync::mpsc::UnboundedSender<ProfileDataUpdate>,
    pub rx: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<ProfileDataUpdate>>>,
}

pub static PROFILE_CHANNEL_LISTENER: Lazy<ProfileUpdateChannel> = Lazy::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    ProfileUpdateChannel {
        tx,
        rx: Arc::new(Mutex::new(rx)),
    }
});

pub struct ProfileDataUpdate {
    pub did: DID,
    pub picture: Option<String>,
    pub banner: Option<String>,
}

pub fn fetch_identity_data(identities: &[Identity]) {
    let identities: Vec<_> = identities.iter().map(|id| id.did_key()).collect();
    tokio::spawn(async move {
        let warp_cmd_tx = WARP_CMD_CH.tx.clone();

        for identity in identities {
            let (tx, rx) = oneshot::channel();
            let _ = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::GetProfilePicture {
                did: identity.clone(),
                rsp: tx,
            }));

            let profile_picture = match rx.await {
                Ok(res) => res.ok(),
                Err(e) => {
                    log::error!("error fetching profile pic {e}");
                    Option::None
                }
            };
            let (tx, rx) = oneshot::channel();
            let _ = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::GetProfileBanner {
                did: identity.clone(),
                rsp: tx,
            }));
            let profile_banner = match rx.await {
                Ok(res) => res.ok(),
                Err(e) => {
                    log::error!("error fetching profile banner {e}");
                    Option::None
                }
            };
            if profile_picture.is_some() || profile_banner.is_some() {
                let _ = PROFILE_CHANNEL_LISTENER.tx.send(ProfileDataUpdate {
                    did: identity,
                    picture: profile_picture,
                    banner: profile_banner,
                });
            }
        }
    });
}
