#![warn(clippy::all, clippy::nursery, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

use clap::Parser;
use color_eyre::Result;
use once_cell::sync::OnceCell;
use std::time::Duration;
use std::path::PathBuf;

mod configuration;
mod events;
mod kanata;

use configuration::Configuration;
use kanata::Kanata;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::Komokana;

#[cfg(target_os = "macos")]
mod osx;
#[cfg(target_os = "macos")]
pub use osx::Komokana;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Komokana;

static DEFAULT_LAYER: OnceCell<String> = OnceCell::new();
static CONFIG: OnceCell<Configuration> = OnceCell::new();
static TMPFILE: OnceCell<bool> = OnceCell::new();
static KANATA: OnceCell<Kanata> = OnceCell::new();

/// A Provider (struct named Komokana) has to implement this trait
/// init():  for initialization
/// listen(self):  for the thread loop
/// resolve_config_path(&str) -> Result<PathBuf>:
/// validates the configuration argument, returns a PathBuf if valid
pub trait Provider {
    fn init() -> Result<Self>
    where
        Self: Sized;
    fn listen(self);
    fn resolve_config_path(config: &str) -> Result<PathBuf>;
    #[cfg(feature = "virtual_keys")]
    fn get_key_state(key_code: i32) -> i16;
}

#[derive(Debug, Parser)]
#[clap(author, about, version, arg_required_else_help = true)]
struct Cli {
    /// The port on which kanata's TCP server is running
    #[clap(short = 'p', long)]
    kanata_port: i32,
    /// Path to your komokana configuration file
    #[clap(short, long, default_value = "~/komokana.yaml")]
    configuration: String,
    /// Layer to default to when an active window doesn't match any rules
    #[clap(short, long)]
    default_layer: String,
    /// Write the current layer to ~/AppData/Local/Temp/kanata_layer
    #[clap(short, long, action)]
    tmpfile: bool,
}

fn init_configuration(config: &str) -> Result<()> {
    let config_path = Komokana::resolve_config_path(config)?;
    let configuration: Configuration =
        serde_yaml::from_str(&std::fs::read_to_string(config_path)?)?;
    CONFIG.set(configuration).unwrap();
    Ok(())
}

fn init_kanata(port: i32) -> Result<()> {
    let kanata = Kanata::new(port)?;
    KANATA.set(kanata).unwrap();
    log::debug!("connected to kanata");
    KANATA.get().unwrap().spawn_kanata_listener();
    Ok(())
}

fn main() -> Result<()> {
    let cli: Cli = Cli::parse();

    init_configuration(&cli.configuration)?;
    DEFAULT_LAYER.set(cli.default_layer).unwrap();
    TMPFILE.set(cli.tmpfile).unwrap();

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    color_eyre::install()?;
    env_logger::builder().format_timestamp(None).init();

    // initializes static kanata object and starts
    // kanata's listening loop
    init_kanata(cli.kanata_port)?;

    let komokana = Komokana::init()?;
    komokana.listen();

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
