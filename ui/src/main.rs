#![cfg_attr(feature = "production_mode", windows_subsystem = "windows")]
// Having it here fixes a file dialog crash for some reason. (Seems to only be on some MacOS)
#[allow(unused_imports)]
use rfd::FileDialog;

fn main() {
    uplink::main_lib();
}
