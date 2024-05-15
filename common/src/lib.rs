pub mod language;
pub mod notifications;
pub mod profile_update_channel;
pub mod sounds;
pub mod state;
pub mod testing;
pub mod upload_file_channel;
pub mod utils;
pub mod warp_runner;

use anyhow::bail;
use clap::Parser;
// export icons crate
pub use icons;
use icons::outline::Shape as Icon;
use once_cell::sync::Lazy;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{broadcast, Mutex};
use warp_runner::{WarpCmdChannels, WarpEventChannels};

use fluent_templates::static_loader;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
        // Removes unicode isolating marks around arguments, you typically
        // should only set to false when testing.
        customise: |bundle| bundle.set_use_isolating(false),
    };
}

#[derive(Debug, Parser)]
#[clap(name = "")]
pub struct Args {
    /// The location to store the .uplink directory, within which a .warp, state.json, and other useful logs will be located
    #[clap(long)]
    path: Option<PathBuf>,
    #[clap(long)]
    discovery: Option<DiscoveryMode>,
    #[clap(long)]
    disable_quic: bool,
    #[clap(long)]
    discovery_point: Option<String>,
    #[cfg(debug_assertions)]
    #[clap(long, default_value_t = false)]
    with_mock: bool,
    /// tells the app that it was installed via an installer, not built locally. Uplink will look for an `extra.zip` file based on
    /// the platform-specific installer.
    #[clap(long, default_value_t = false)]
    pub production_mode: bool,
    /// configures log output
    #[clap(long, default_value_t = false)]
    pub log_to_file: bool,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum DiscoveryMode {
    /// Enable full discovery
    Full,

    /// Use warp specific discovery
    #[default]
    Shuttle,

    /// Address to a specific discovery point
    RzPoint { address: String },

    /// Disable discovery
    Disable,
}

impl std::str::FromStr for DiscoveryMode {
    type Err = warp::error::Error;
    fn from_str(mode: &str) -> Result<Self, Self::Err> {
        match mode.to_lowercase().as_str() {
            "full" => Ok(DiscoveryMode::Full),
            "shuttle" => Ok(DiscoveryMode::Shuttle),
            "disable" => Ok(DiscoveryMode::Disable),
            _ => Err(warp::error::Error::Other),
        }
    }
}

#[derive(Debug)]
pub struct StaticArgs {
    /// ~/.uplink
    /// contains the following: extra (folder), extensions (folder), themes (folder), fonts (folder), .user
    pub dot_uplink: PathBuf,
    /// ~/.uplink/.user
    /// contains the following: warp (folder), state.json, debug.log
    pub uplink_path: PathBuf,
    /// Directory for temporary files and deleted everytime app is closed or opened
    pub temp_files: PathBuf,
    /// custom themes for the user
    pub themes_path: PathBuf,
    /// custom fonts for the user
    pub fonts_path: PathBuf,
    /// state.json: a serialized version of State which gets saved every time state is modified
    pub cache_path: PathBuf,
    /// a fake tesseract_path to prevent anything from mutating the tesseract keypair after it has been created (probably not necessary)
    pub mock_cache_path: PathBuf,
    /// houses warp specific data
    pub warp_path: PathBuf,
    /// a debug log which is only written to when the settings are enabled. otherwise logs are only sent to stdout
    pub logger_path: PathBuf,
    /// contains the keypair used for IPFS
    pub tesseract_file: String,
    /// the unlock and auth pages don't have access to State but need to know if they should play a notification.
    /// part of state is serialized and saved here
    pub login_config_path: PathBuf,
    /// path to custom plugins
    pub extensions_path: PathBuf,
    /// crash logs
    pub crash_logs: PathBuf,
    /// recordings
    pub recordings: PathBuf,
    /// seconds
    pub typing_indicator_refresh: u64,
    /// seconds
    pub typing_indicator_timeout: u64,
    /// used only for testing the UI. generates fake friends, conversations, and messages
    pub use_mock: bool,
    /// Disable discovery
    pub discovery: DiscoveryMode,
    /// Enable quic transport
    pub disable_quic: bool,
    // some features aren't ready for release. This field is used to disable such features.
    pub production_mode: bool,
}

pub static STATIC_ARGS: Lazy<StaticArgs> = Lazy::new(|| {
    let args = Args::parse();
    #[allow(unused_mut)]
    #[allow(unused_assignments)]
    let mut use_mock = false;
    #[cfg(debug_assertions)]
    {
        use_mock = args.with_mock;
    }

    let uplink_container = match args.path {
        Some(path) => path,
        _ => dirs::home_dir().unwrap_or_default().join(".uplink"),
    };

    let uplink_path = uplink_container.join(".user");
    let warp_path = uplink_path.join("warp");
    StaticArgs {
        dot_uplink: uplink_container.clone(),
        uplink_path: uplink_path.clone(), // TODO: Should this be "User path" instead?
        temp_files: uplink_container.join("temp_files"),
        themes_path: uplink_container.join("themes"),
        fonts_path: uplink_container.join("fonts"),
        cache_path: uplink_path.join("state.json"),
        extensions_path: uplink_container.join("extensions"),
        crash_logs: uplink_container.join("crash-logs"),
        recordings: uplink_container.join("recordings"),
        mock_cache_path: uplink_path.join("mock-state.json"),
        warp_path: warp_path.clone(),
        logger_path: uplink_path.join("debug.log"),
        typing_indicator_refresh: 5,
        typing_indicator_timeout: 6,
        tesseract_file: "tesseract.json".into(),
        login_config_path: uplink_path.join("login_config.json"),
        use_mock,
        discovery: args.discovery.unwrap_or_default(),
        disable_quic: args.disable_quic,
        production_mode: cfg!(feature = "production_mode"),
    }
});

// allows the UI to send commands to Warp
pub static WARP_CMD_CH: Lazy<WarpCmdChannels> = Lazy::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    WarpCmdChannels {
        tx,
        rx: Arc::new(Mutex::new(rx)),
    }
});

// allows the UI to receive events to Warp
// pretty sure the rx channel needs to be in a mutex in order for it to be a static mutable variable
pub static WARP_EVENT_CH: Lazy<WarpEventChannels> = Lazy::new(|| {
    let (tx, _rx) = broadcast::channel(8198);
    WarpEventChannels { tx, _rx }
});

pub const MAX_FILES_PER_MESSAGE: usize = 32;

pub const ROOT_DIR_NAME: &str = "root";

pub const VIDEO_FILE_EXTENSIONS: &[&str] =
    &[".mp4", ".mov", ".avi", ".flv", ".wmv", ".m4v", ".3gp"];

pub const IMAGE_FILE_EXTENSIONS: &[&str] =
    &[".jpg", ".jpeg", ".png", ".gif", ".bmp", ".tiff", ".svg"];

pub const AUDIO_FILE_EXTENSIONS: &[&str] = &[".mp3", ".wav", ".flac", ".aac", ".m4a"];

pub const DOC_EXTENSIONS: &[&str] = &[".doc", ".docx", ".pdf", ".txt", ".csv", ".tsv"];

pub const PRODUCTIVITY_DOCS: &[&str] = &[
    ".xls", ".xlsx", ".xlsm", ".xlsb", // Excel extensions
    ".ppt", ".pptx", ".pps", ".ppsx", // PowerPoint extensions
    ".odt", ".ott", ".ods", ".ots", ".odp", ".otp", ".rtf",
];

#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    Video,
    Image,
    Audio,
    Doc,
    Code,
    Unkwnown,
}

pub fn is_file_available_to_preview(file_name: &str) -> bool {
    is_image(file_name)
        || is_video(file_name)
        || is_audio(file_name)
        || is_pdf_file(file_name)
        || is_lang_file(file_name)
}

pub fn is_image(file_name: &str) -> bool {
    IMAGE_FILE_EXTENSIONS
        .iter()
        .any(|x| file_name.to_lowercase().ends_with(x))
}

pub fn is_video(file_name: &str) -> bool {
    VIDEO_FILE_EXTENSIONS
        .iter()
        .any(|x| file_name.to_lowercase().ends_with(x))
}

pub fn is_audio(file_name: &str) -> bool {
    AUDIO_FILE_EXTENSIONS
        .iter()
        .any(|x| file_name.to_lowercase().ends_with(x))
}

pub fn is_doc(file_name: &str) -> bool {
    DOC_EXTENSIONS
        .iter()
        .any(|x| file_name.to_lowercase().ends_with(x))
}

pub fn is_pdf_file(file_name: &str) -> bool {
    file_name.to_lowercase().ends_with(".pdf")
}

pub fn is_productivity_doc(file_name: &str) -> bool {
    PRODUCTIVITY_DOCS
        .iter()
        .any(|x| file_name.to_lowercase().ends_with(x))
}

pub fn is_lang_file(file_name: &str) -> bool {
    PROGRAMMING_LANG_FILES
        .iter()
        .any(|x| file_name.to_lowercase().ends_with(x))
}

pub fn return_correct_icon(file_name: &str) -> Icon {
    if is_video(file_name) {
        return Icon::DocumentMedia;
    }
    if is_audio(file_name) {
        return Icon::DocumentAudio;
    }

    if is_doc(file_name) {
        return Icon::DocumentText;
    }

    if is_productivity_doc(file_name) {
        return Icon::DocumentChartBar;
    }

    if is_lang_file(file_name) {
        return Icon::DocumentCode;
    }

    if is_image(file_name) {
        return Icon::Document;
    }

    Icon::DocumentQuestion
}

pub fn get_file_type(file_name: &str) -> FileType {
    if is_video(file_name) {
        FileType::Video
    } else if is_audio(file_name) {
        FileType::Audio
    } else if is_doc(file_name) {
        FileType::Doc
    } else if is_lang_file(file_name) {
        FileType::Code
    } else if is_image(file_name) {
        FileType::Image
    } else {
        FileType::Unkwnown
    }
}

pub fn get_images_dir() -> anyhow::Result<PathBuf> {
    if !cfg!(feature = "production_mode") {
        return Ok(Path::new("ui").join("extra").join("images"));
    };

    if cfg!(target_os = "windows") {
        Ok(PathBuf::from(r"..\extra\images"))
    } else {
        Ok(get_extras_dir()?.join("images"))
    }
}

pub fn get_extras_dir() -> anyhow::Result<PathBuf> {
    if !cfg!(feature = "production_mode") {
        return Ok(Path::new("ui").join("extra"));
    };

    let assets_path = if cfg!(target_os = "windows") {
        let exe_path = std::env::current_exe()?;
        exe_path
            .parent()
            .and_then(|x| x.parent())
            .map(|x| x.join("extra"))
            .ok_or(anyhow::format_err!("failed to get Windows extra dir"))?
    } else if cfg!(target_os = "linux") {
        PathBuf::from("/opt/im.satellite/extra")
    } else if cfg!(target_os = "macos") {
        let exe_path = std::env::current_exe()?;
        exe_path
            .parent()
            .and_then(|x| x.parent())
            .map(|x| x.join("Resources"))
            .ok_or(anyhow::format_err!("failed to get MacOs resources dir"))?
    } else {
        bail!("unknown OS type. failed to copy assets");
    };

    Ok(assets_path)
}

pub fn get_extensions_dir() -> anyhow::Result<PathBuf> {
    let extensions_path = if cfg!(target_os = "windows") {
        let exe_path = std::env::current_exe()?;
        exe_path
            .parent()
            .and_then(|x| x.parent())
            .map(|x| x.join("extensions"))
            .ok_or(anyhow::format_err!("failed to get Windows extensions dir"))?
    } else if cfg!(target_os = "linux") {
        PathBuf::from("/opt/im.satellite/extensions")
    } else if cfg!(target_os = "macos") {
        let exe_path = std::env::current_exe()?;
        exe_path
            .parent()
            .and_then(|x| x.parent())
            .map(|x| x.join("Frameworks"))
            .ok_or(anyhow::format_err!("failed to get MacOs extensions dir"))?
    } else {
        bail!("unknown OS type. failed to copy assets");
    };

    Ok(extensions_path)
}

pub const PROGRAMMING_LANG_FILES: &[&str] = &[
    ".c", ".cpp", ".cc", ".h", ".hpp",  // C/C++
    ".cs",   // C#
    ".java", // Java
    ".js", ".jsx", ".ts", ".tsx", // JavaScript
    ".py", ".pyc", ".pyd", ".pyo", ".pyw", ".pyz", // Python
    ".rb",  // Ruby
    ".php", ".php4", ".php5", ".php7", ".phtml", // PHP
    ".swift", // Swift
    ".go",    // Go
    ".rs", ".toml", // Rust
    ".kt", ".kts", // Kotlin
    ".scala", ".sc", // Scala
    ".pl", ".pm", // Perl
    ".sh", ".bash", ".zsh",  // Shell Scripting
    ".ps1",  // PowerShell
    ".r",    // R
    ".dart", // Dart
    ".lua",  // Lua
    ".hs",   // Haskell
    ".m",    // Objective-C
    ".sql",  // SQL
    ".html", ".htm", ".css", ".scss", // HTML/CSS
    ".xml",  // XML
    ".asm", ".s", // Assembly
    ".m", ".mlx", // Matlab
    ".f", ".for", ".f90", // Fortran
    ".lisp", ".lsp", // Lisp
    ".pro", ".P", // Prolog
    ".ada", ".ads", ".adb", // Ada
    ".groovy", ".gvy", ".gy", ".gsh", // Groovy
    ".clj", ".cljs", ".cljc", ".edn", // Clojure
    ".elm", // Elm
    ".ex", ".exs", // Elixir
    ".erl", ".hrl", // Erlang
    ".vbs", // VBScript
    ".vb",  // Visual Basic .NET
    ".fs", ".fsx", ".fsi",    // F#
    ".ts",     // TypeScript
    ".coffee", // CoffeeScript
    ".json",   // JSON
    ".yaml", ".yml", // YAML
    ".md",  // Markdown
];
