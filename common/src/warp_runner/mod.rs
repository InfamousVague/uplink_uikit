//! Defines important types and structs, and spawns the main task for warp_runner - manager::run.
use derive_more::Display;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex, Notify,
};
use warp::{
    blink::{
        Blink::{self},
        BlinkEventKind,
    },
    constellation::{file::FileType, Constellation},
    error::Error,
    logging::tracing::log,
    multipass::{self, IdentityImportOption, ImportLocation, MultiPass},
    raygun::RayGun,
    tesseract::Tesseract,
};
use warp_ipfs::{
    config::{Config, Discovery, UpdateEvents},
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
                            Ok(ident) => match save_tesseract(&warp.tesseract) {
                                Ok(_) => {
                                    let _ = rsp.send(Ok(ident));
                                    break Some(warp);
                                }
                                Err(e) => {
                                    let _ = rsp.send(Err(e));
                                    continue;
                                }
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
                                Ok(ident) => match save_tesseract(&warp.tesseract) {
                                    Ok(_) => {
                                        // todo: remove this once warp is updated to export the identity automatically
                                        if matches!(STATIC_ARGS.discovery, DiscoveryMode::Shuttle) {
                                            if let Err(e) = warp.multipass.export_identity(ImportLocation::Remote).await {
                                                log::error!("failed to export identity: {e}");
                                            }
                                        }

                                        let _ = rsp.send(Ok(ident));
                                        break Some(warp);
                                    }
                                    Err(e) => {
                                        let _ = rsp.send(Err(e));
                                        continue;
                                    }
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

    let configure_tesseract = |tesseract: Tesseract| {
        // prevent other things from corrupting the real tesseract file.
        tesseract.set_file(STATIC_ARGS.warp_path.join("fake_tesseract.json"));
        tesseract.set_autosave();
        tesseract
    };

    // this code path addresses cross-platform issues involving account recreation.
    // the tesseract file was being overwritten incorrectly.
    // to fix this, manually delete the file and re-create it.
    if overwrite_old_account {
        // delete old account data
        if let Err(e) = std::fs::remove_dir_all(&STATIC_ARGS.uplink_path) {
            log::warn!("failed to delete uplink directory: {}", e);
        }

        // create directories
        if let Err(e) = std::fs::create_dir_all(&STATIC_ARGS.warp_path) {
            log::warn!("failed to create warp directory: {}", e);
        }

        // create the tesseract key file so it can be saved later
        if let Err(e) = std::fs::File::create(&STATIC_ARGS.tesseract_path) {
            log::error!("failed to create tesseract file: {}", e);
            return Err(warp::error::Error::CannotSaveTesseract);
        }

        return Ok(configure_tesseract(Tesseract::default()));
    }

    // open existing file or create new one
    let tesseract = match std::fs::File::open(&STATIC_ARGS.tesseract_path) {
        Ok(mut file) => match Tesseract::from_reader(&mut file) {
            Ok(tesseract) => configure_tesseract(tesseract),
            Err(e) => {
                log::error!("failed to deserialize tesseract: {}", e);
                log::warn!("creating new tesseract");
                configure_tesseract(Tesseract::default())
            }
        },
        Err(e) => {
            log::error!("failed to open file: {}", e);
            log::warn!("creating new tesseract");

            // create the file so it can be saved later
            if let Err(e) = std::fs::File::create(&STATIC_ARGS.tesseract_path) {
                log::error!("failed to create tesseract file: {}", e);
                return Err(warp::error::Error::CannotSaveTesseract);
            }
            configure_tesseract(Tesseract::default())
        }
    };

    Ok(tesseract)
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
                #[cfg(not(feature = "production_mode"))]
                {
                    Discovery::Shuttle {
                        addresses: HashMap::from_iter([(
                            "12D3KooWJSes8386p2T1sMeZ2DzsNJThKkZWbj4US6uPMpEgBTHu"
                                .parse()
                                .expect("Valid id"),
                            HashSet::from_iter(["/ip4/104.236.194.35/tcp/34053"
                                .parse()
                                .expect("valid addr")]),
                        )]),
                    }
                }
                #[cfg(feature = "production_mode")]
                {
                    //TODO: Add a production map
                    Discovery::None
                }
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

    // Discovery is disabled by default for now but may offload manual discovery through a separate service
    // in the near future
    config.store_setting.discovery = Discovery::from(&STATIC_ARGS.discovery);

    config.ipfs_setting.disable_quic = !STATIC_ARGS.enable_quic;
    config.ipfs_setting.portmapping = true;
    config.ipfs_setting.agent_version = Some(format!("uplink/{}", env!("CARGO_PKG_VERSION")));
    config.store_setting.emit_online_event = true;
    config.store_setting.share_platform = true;
    config.store_setting.update_events = UpdateEvents::Enabled;
    config.store_setting.default_profile_picture = Some(Arc::new(|identity| {
        let mut content = plot_icon::generate_png(identity.did_key().to_string().as_bytes(), 512)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        content.extend([11, 00, 23]);

        Ok((
            content,
            FileType::Mime("image/png".parse().expect("Correct mime")),
        ))
    }));
    config.thumbnail_size = (500, 500);

    let (multipass, raygun, constellation) = WarpIpfsBuilder::default()
        .set_tesseract(tesseract.clone())
        .set_config(config)
        .finalize()
        .await?;

    let blink = warp_blink_wrtc::BlinkImpl::new(multipass.clone()).await?;

    Ok(manager::Warp {
        tesseract,
        multipass,
        raygun,
        constellation,
        blink,
    })
}

pub fn save_tesseract(tesseract: &warp::tesseract::Tesseract) -> Result<(), Error> {
    log::info!("saving tesseract");
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .append(false)
        .create(false)
        .open(&STATIC_ARGS.tesseract_path)
    {
        Ok(f) => f,
        Err(e) => {
            log::error!("failed to open tesseract keystore for saving: {}", e);
            return Err(Error::CorruptedDataStore);
        }
    };
    if let Err(e) = tesseract.to_writer(&mut file) {
        log::error!("tesseract.to_writer() failed: {}", e);
        return Err(e);
    }

    if let Err(e) = file.sync_all() {
        log::error!("failed to sync tesseract: {}", e);
        return Err(Error::CorruptedDataStore);
    }

    Ok(())
}
