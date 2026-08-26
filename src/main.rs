#![forbid(unsafe_code)]

use fanwaave_desktop_core::{app::DesktopApp, config::DesktopConfig};

fn main() {
    let cfg = DesktopConfig::from_env();
    DesktopApp::new(cfg).run();
}

