#![forbid(unsafe_code)]

use fanwaave_desktop_core::{app::DesktopApp, config::DesktopConfig};

fn main() {
    let cfg = match DesktopConfig::from_process() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Fanwaave desktop configuration error: {error}");
            std::process::exit(2);
        }
    };
    DesktopApp::new(cfg).run();
}
