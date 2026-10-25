#![forbid(unsafe_code)]

use fanwaave_desktop_core::{app::DesktopApp, config::DesktopConfig};

fn main() {
    // Shared lifecycle logging stays local and leaves stdout available for IPC.
    let _desktop_session = next_loggers::desktop::DesktopSession::start(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    )
    .ok();

    let cfg = match DesktopConfig::from_process() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Fanwaave desktop configuration error: {error}");
            std::process::exit(2);
        }
    };
    DesktopApp::new(cfg).run();
}
