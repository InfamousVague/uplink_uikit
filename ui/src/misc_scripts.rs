use std::rc::Rc;

use super::*;

const PRISM_SCRIPT: &str = include_str!("../extra/assets/scripts/prism.js");
pub const PRISM_STYLE: &str = include_str!("../extra/assets/styles/prism.css");
pub const PRISM_THEME: &str = include_str!("../extra/assets/styles/prism-one-dark.css");
pub const MARKDOWN_EDITOR: &str = include_str!("../extra/assets/scripts/editor.js");

pub fn PrismScripts() -> Element {
    let prism_path = use_prism_path();

    rsx! {
        script { "{PRISM_SCRIPT}" },
        script { "{prism_path}" },
        script { "{MARKDOWN_EDITOR}" },
    }
}

fn use_prism_path() -> String {
    let hook_result = &use_hook(|| {
        Rc::new(format!(
            r"Prism.plugins.autoloader.languages_path = '{}';",
            get_prism_path().to_string_lossy()
        ))
    });
    hook_result.clone().as_str().to_string()
}

fn get_prism_path() -> PathBuf {
    if STATIC_ARGS.production_mode {
        if cfg!(target_os = "windows") {
            STATIC_ARGS.dot_uplink.join("prism_langs")
        } else {
            get_extras_dir().unwrap_or_default().join("prism_langs")
        }
    } else {
        PathBuf::from("ui").join("extra").join("prism_langs")
    }
}
