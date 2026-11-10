//! `weathertrack-web`: weather for any place, and one place for all your ESP32 devices.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use weathertrack_web::app::{ServerOptions, serve};
use weathertrack_web::net::UreqNet;
use weathertrack_web::state::Shared;
use weathertrack_web::weather::OpenMeteo;

#[derive(Debug, Parser)]
#[command(
    version,
    about = "WeatherTrack web: weather for any place, and one place for all your ESP32 devices"
)]
struct Cli {
    /// Interface to bind (there is no login: keep it on the loopback address).
    #[arg(long, default_value = "127.0.0.1", env = "WEATHERTRACK_HOST")]
    host: String,
    /// Port to listen on.
    #[arg(long, short, default_value_t = 3030, env = "WEATHERTRACK_PORT")]
    port: u16,
    /// Do not open a browser.
    #[arg(long)]
    no_open: bool,
    /// A device to add, e.g. 192.168.3.210 (repeatable; devices are remembered).
    #[arg(long = "device", short = 'd')]
    devices: Vec<String>,
    /// Where the settings are kept (default: the user's config directory).
    #[arg(long)]
    config: Option<PathBuf>,
}

fn config_path(cli: &Cli) -> Option<PathBuf> {
    cli.config
        .clone()
        .or_else(|| dirs::config_dir().map(|d| d.join("weathertrack").join("web.json")))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let net = Arc::new(UreqNet::new());
    let state = Shared::new(
        net.clone(),
        Arc::new(OpenMeteo::new(net)),
        config_path(&cli),
    );
    for d in &cli.devices {
        if !state.add_device(d) {
            eprintln!("weathertrack-web: not adding {d:?} (not an address, or already known)");
        }
    }
    serve(
        ServerOptions {
            host: cli.host,
            port: cli.port,
            open_browser: !cli.no_open,
        },
        state,
    )
    .await
}
