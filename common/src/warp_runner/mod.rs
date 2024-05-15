//! Defines important types and structs, and spawns the main task for warp_runner - manager::run.
use derive_more::Display;
use std::sync::Arc;

use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex, Notify,
};
use tracing::log;
use warp::{
    blink::{
        Blink::{self},
        BlinkEventKind,
    },
    constellation::{file::FileType, Constellation},
    error::Error,
    multipass::{self, IdentityImportOption, MultiPass},
    raygun::RayGun,
    tesseract::Tesseract,
};
use warp_ipfs::{
    config::{Bootstrap, Config, Discovery, UpdateEvents},
    WarpIpfsBuilder,
};

use crate::{DiscoveryMode, STATIC_ARGS, WARP_CMD_CH};

use self::ui_adapter::{MultiPassEvent, RayGunEvent};

mod conv_stream;
mod data;
mod manager;
pub mod ui_adapter;

pub use data::*;
pub use manager::commands::thumbnail_to_base64;
pub use manager::{BlinkCmd, ConstellationCmd, MultiPassCmd, OtherCmd, RayGunCmd, TesseractCmd};

pub type WarpCmdTx = UnboundedSender<WarpCmd>;
pub type WarpCmdRx = Arc<Mutex<UnboundedReceiver<WarpCmd>>>;
pub type WarpEventTx = broadcast::Sender<WarpEvent>;

pub struct WarpCmdChannels {
    pub tx: WarpCmdTx,
    pub rx: WarpCmdRx,
}

pub struct WarpEventChannels {
    pub tx: WarpEventTx,
    pub _rx: tokio::sync::broadcast::Receiver<WarpEvent>,
}

type Account = Box<dyn MultiPass>;
type Storage = Box<dyn Constellation>;
type Messaging = Box<dyn RayGun>;
type Calling = Box<dyn Blink>;

#[allow(clippy::large_enum_variant)]
#[derive(Display, Clone)]
pub enum WarpEvent {
    #[display(fmt = "RayGunEvent {{ {_0} }} ")]
    RayGun(RayGunEvent),
    #[display(fmt = "MessageEvent {{ {_0} }} ")]
    Message(ui_adapter::MessageEvent),
    #[display(fmt = "MultiPassEvent {{ {_0} }} ")]
    MultiPass(MultiPassEvent),
    #[display(fmt = "BlinkEvent {{ {_0} }} ")]
    Blink(BlinkEventKind),
}

impl std::fmt::Debug for WarpEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Display)]
pub enum WarpCmd {
    #[display(fmt = "Tesseract {{ {_0} }} ")]
    Tesseract(TesseractCmd),
    #[display(fmt = "MultiPass {{ {_0} }} ")]
    MultiPass(MultiPassCmd),
    #[display(fmt = "RayGun {{ {_0} }} ")]
    RayGun(RayGunCmd),
    #[display(fmt = "Constellation {{ {_0} }} ")]
    Constellation(ConstellationCmd),
    #[display(fmt = "Blink {{ {_0} }} ")]
    Blink(BlinkCmd),
    // these commands may not actually be warp commands, but just require a long running
    // async task, executed separately from the UI
    #[display(fmt = "Other {{ {_0} }} ")]
    Other(OtherCmd),
}

/// Spawns a task which manages multiple streams, channels, and tasks related to warp
pub struct WarpRunner {
    // perhaps collecting a JoinHandle and calling abort() would be better than using Notify.
    notify: Arc<Notify>,
    ran_once: bool,
}

impl std::ops::Drop for WarpRunner {
    fn drop(&mut self) {
        self.notify.notify_waiters();
    }
}

impl Default for WarpRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl WarpRunner {
    pub fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
            ran_once: false,
        }
    }

    // spawns a task which will terminate when WarpRunner is dropped
    pub fn run(&mut self) {
        assert!(!self.ran_once, "WarpRunner called run() multiple times");
        self.ran_once = true;

        let notify = self.notify.clone();
        tokio::spawn(async move {
            handle_login(notify.clone()).await;
        });
    }
}

// handle_login calls manager::run, which continues to process warp commands
async fn handle_login(notify: Arc<Notify>) {
    let warp_cmd_rx = WARP_CMD_CH.rx.clone();
    // be sure to drop this channel before calling manager::run()
    let mut warp_cmd_rx = warp_cmd_rx.lock().await;

    let tesseract = init_tesseract(false)
        .await
        .expect("failed to initialize tesseract");

    let mut warp = match warp_initialization(tesseract).await {
        Ok(w) => w,
        Err(e) => {
            log::error!("warp init failed: {}", e);
            return;
        }
    };

    let account_exists = warp.tesseract.exist("keypair");

    // until the user logs in, raygun and multipass are no use.
    let warp: Option<manager::Warp> = loop {
        tokio::select! {
            opt = warp_cmd_rx.recv() => {
                if let Some(cmd) = &opt {
                    log::debug!("received warp cmd: {}", cmd);
                }

                match opt {
                    Some(WarpCmd::MultiPass(MultiPassCmd::RecoverIdentity{
                        passphrase,
                        seed_words,
                        rsp
                    })) => {
                        let tesseract = init_tesseract(true)
                                .await
                                .expect("failed to initialize tesseract");
                        warp = match warp_initialization(tesseract).await {
                            Ok(w) => w,
                            Err(e) => {
                                log::error!("warp init failed: {}", e);
                                let _ = rsp.send(Err(e));
                                return;
                            }
                        };
                        if let Err(e) = warp.tesseract.unlock(passphrase.as_bytes()) {
                            log::info!("unlock failed: {:?}", e);
                            let _ = rsp.send(Err(e));
                            continue;
                        };
                        match warp.multipass.import_identity(IdentityImportOption::Locate {
                            location: multipass::ImportLocation::Remote,
                            passphrase: seed_words
                        }).await {
                            Ok(ident) => {
                                let _ = rsp.send(Ok(ident));
                                break Some(warp);
                            },
                            Err(e) => {
                                warp.tesseract.lock();
                                let _ = rsp.send(Err(e));
                                continue;
                            }
                        }
                    },
                    Some(WarpCmd::MultiPass(MultiPassCmd::CreateIdentity {
                        username,
                        tesseract_passphrase,
                        seed_words,
                        rsp,
                    })) => {
                        if account_exists {
                            log::debug!("attempting to overwrite old account");
                            let tesseract = init_tesseract(true)
                                .await
                                .expect("failed to initialize tesseract");
                            warp = match warp_initialization(tesseract).await {
                                Ok(w) => w,
                                Err(e) => {
                                    log::error!("warp init failed: {}", e);
                                    let _ = rsp.send(Err(e));
                                    return;
                                }
                            };
                        }

                        if let Err(e) = warp.tesseract.unlock(tesseract_passphrase.as_bytes()) {
                            log::info!("unlock failed: {:?}", e);
                            let _ = rsp.send(Err(e));
                            continue;
                        };
                        match warp.multipass.create_identity(Some(&username), Some(&seed_words)).await {
                            Ok(_id) =>  match wait_for_multipass(&mut warp, notify.clone()).await {
                                Ok(ident) => {
                                    let _ = rsp.send(Ok(ident));
                                    break Some(warp);
                                },
                                Err(e) => {
                                    warp.tesseract.lock();
                                    let _ = rsp.send(Err(e));
                                    continue;
                                }
                            }
                            Err(e) => {
                                log::error!("create_identity failed. should never happen: {}", e);
                                let _ = rsp.send(Err(e));
                            }
                        }
                    }
                    Some(WarpCmd::MultiPass(MultiPassCmd::TryLogIn { passphrase, rsp })) => {
                        if let Err(e) = warp.tesseract.unlock(passphrase.as_bytes()) {
                            log::info!("unlock failed: {:?}", e);
                            let _ = rsp.send(Err(e));
                            continue;
                        };
                        match wait_for_multipass(&mut warp, notify.clone()).await {
                            Ok(ident) => {
                                let _ = rsp.send(Ok(ident));
                                break Some(warp);
                            },
                            Err(e) => {
                                warp.tesseract.lock();
                                let _ = rsp.send(Err(e));
                                continue;
                            }
                        }
                    }
                    Some(WarpCmd::Tesseract(TesseractCmd::AccountExists { rsp }))  => {
                        let _ = rsp.send(account_exists);
                    }
                    _ => {}
                }
            },
            // the WarpRunner has been dropped. stop the task
            _ = notify.notified() => break None,
        }
    };

    // release the lock
    drop(warp_cmd_rx);

    if let Some(warp) = warp {
        manager::run(warp, notify).await;
    } else {
        log::info!("warp_runner terminated during initialization");
    }
}

async fn wait_for_multipass(
    warp: &mut manager::Warp,
    notify: Arc<Notify>,
) -> Result<multipass::identity::Identity, Error> {
    let multipass_init_done = async {
        loop {
            match warp.multipass.get_own_identity().await {
                Ok(ident) => return Ok(ident),
                Err(e) => match e {
                    Error::MultiPassExtensionUnavailable => {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }
                    _ => {
                        log::error!("multipass.get_own_identity failed: {}", e);
                        return Err(e);
                    }
                },
            }
        }
    };

    tokio::select! {
        r = multipass_init_done => r,
        _ = notify.notified() => {
            log::error!("notified interrupted multipass initialization");
            Err(Error::Other)
        },
    }
}

// don't set file or autosave until tesseract is unlocked
// assumes that all anyone needs from tesseract is "keypair"
// otherwise, Tesseract::to_file probably needs to call file.sync_all()
async fn init_tesseract(overwrite_old_account: bool) -> Result<Tesseract, Error> {
    log::trace!("initializing tesseract");

    if overwrite_old_account {
        // delete old account data
        if let Err(e) = tokio::fs::remove_dir_all(&STATIC_ARGS.uplink_path).await {
            log::warn!("failed to delete uplink directory: {}", e);
        }

        // create directories
        if let Err(e) = tokio::fs::create_dir_all(&STATIC_ARGS.warp_path).await {
            log::warn!("failed to create warp directory: {}", e);
        }
    }

    // open existing file or create new one
    Tesseract::open_or_create(&STATIC_ARGS.warp_path, &STATIC_ARGS.tesseract_file)
}

impl From<&DiscoveryMode> for Discovery {
    fn from(mode: &DiscoveryMode) -> Self {
        match mode {
            DiscoveryMode::Full => Discovery::Namespace {
                namespace: None,
                discovery_type: warp_ipfs::config::DiscoveryType::DHT,
            },
            DiscoveryMode::RzPoint { address } => Discovery::Namespace {
                namespace: None,
                discovery_type: warp_ipfs::config::DiscoveryType::RzPoint {
                    addresses: vec![address.parse().expect("Valid multiaddr address")],
                },
            },
            DiscoveryMode::Shuttle => {
                let env_addrs = std::env::var("SHUTTLE_ADDR_POINT")
                    .map(|val| {
                        val.split(',')
                            .filter_map(|addr_str| addr_str.parse::<_>().ok())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let addresses = match env_addrs.is_empty() {
                    true => Vec::from_iter(["/ip4/159.65.41.31/tcp/8848/p2p/12D3KooWRF2bz3KDRPvBs1FASRDRk7BfdYc1RUcfwKsz7UBEu7mL"
                                .parse()
                                .expect("valid addr")]),
                    false => env_addrs
                };

                log::debug!("shuttle addresses: {:?}", addresses);

                Discovery::Shuttle { addresses }
            }
            DiscoveryMode::Disable => Discovery::None,
        }
    }
}

impl From<DiscoveryMode> for Discovery {
    fn from(mode: DiscoveryMode) -> Self {
        Self::from(&mode)
    }
}

// tesseract needs to be initialized before warp is initialized. need to call this function again once tesseract is unlocked by the password
async fn warp_initialization(tesseract: Tesseract) -> Result<manager::Warp, warp::error::Error> {
    log::debug!("warp initialization");

    let path = &STATIC_ARGS.warp_path;
    let mut config = Config::production(path);

    *config.save_phrase_mut() = true; // TODO: This should be bound to a setting within Uplink so that the user can choose not to reveal the phrase for increased security.``
    *config.bootstrap_mut() = Bootstrap::None;

    let ipfs_setting = config.ipfs_setting_mut();
    ipfs_setting.disable_quic = STATIC_ARGS.disable_quic;
    ipfs_setting.portmapping = true;
    ipfs_setting.agent_version = Some(format!("uplink/{}", env!("CARGO_PKG_VERSION")));

    let store_setting = config.store_setting_mut();
    // Discovery is disabled by default for now but may offload manual discovery through a separate service
    // in the near future
    store_setting.discovery = Discovery::from(&STATIC_ARGS.discovery);
    store_setting.emit_online_event = true;
    store_setting.share_platform = true;
    store_setting.update_events = UpdateEvents::Enabled;
    store_setting.default_profile_picture = Some(Arc::new(|identity| {
        let mut content = plot_icon::generate_png(identity.did_key().to_string().as_bytes(), 512)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        content.extend([11, 00, 23]);

        Ok((
            content,
            FileType::Mime("image/png".parse().expect("Correct mime")),
        ))
    }));
    *config.thumbnail_size_mut() = (500, 500);

    let (multipass, raygun, constellation) = WarpIpfsBuilder::default()
        .set_tesseract(tesseract.clone())
        .set_config(config)
        .finalize()
        .await;

    let blink = warp_blink_wrtc::BlinkImpl::new(multipass.clone()).await?;

    Ok(manager::Warp {
        tesseract,
        multipass,
        raygun,
        constellation,
        blink,
    })
}
