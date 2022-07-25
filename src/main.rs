#![warn(clippy::all, clippy::nursery, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

use clap::AppSettings;
use clap::Parser;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::anyhow;
use color_eyre::Report;
use color_eyre::Result;
use json_dotpath::DotPaths;
use miow::pipe::NamedPipe;
use parking_lot::Mutex;
use serde_json::json;
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;

use crate::configuration::Configuration;
use crate::configuration::Strategy;

mod configuration;

static KANATA_DISCONNECTED: AtomicBool = AtomicBool::new(false);
static KANATA_RECONNECT_REQUIRED: AtomicBool = AtomicBool::new(false);

const PIPE: &str = r#"\\.\pipe\"#;
const NAME: &str = "komokana";

#[derive(Debug, Parser)]
#[clap(author, about, version, setting = AppSettings::DeriveDisplayOrder, arg_required_else_help = true)]
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

fn main() -> Result<()> {
    let cli: Cli = Cli::parse();
    let configuration = resolve_windows_path(&cli.configuration)?;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    color_eyre::install()?;
    env_logger::builder().format_timestamp(None).init();

    let mut komokana = Komokana::init(
        configuration,
        cli.kanata_port,
        cli.default_layer,
        cli.tmpfile,
    )?;

    komokana.listen();

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

struct Komokana {
    komorebi: Arc<Mutex<NamedPipe>>,
    kanata: Arc<Mutex<TcpStream>>,
    kanata_port: i32,
    configuration: Configuration,
    default_layer: String,
    tmpfile: bool,
}

impl Komokana {
    pub fn init(
        configuration: PathBuf,
        kanata_port: i32,
        default_layer: String,
        tmpfile: bool,
    ) -> Result<Self> {
        let pipe = format!("{}\\{}", PIPE, NAME);

        let configuration: Configuration =
            serde_yaml::from_str(&std::fs::read_to_string(configuration)?)?;

        let named_pipe = NamedPipe::new(pipe)?;

        let mut output = Command::new("cmd.exe")
            .args(["/C", "komorebic.exe", "subscribe", NAME])
            .output()?;

        while !output.status.success() {
            log::warn!(
                "komorebic.exe failed with error code {:?}, retrying in 5 seconds...",
                output.status.code()
            );

            std::thread::sleep(Duration::from_secs(5));

            output = Command::new("cmd.exe")
                .args(["/C", "komorebic.exe", "subscribe", NAME])
                .output()?;
        }

        named_pipe.connect()?;
        log::debug!("connected to komorebi");

        let stream = TcpStream::connect(format!("localhost:{kanata_port}"))?;
        log::debug!("connected to kanata");

        Ok(Self {
            komorebi: Arc::new(Mutex::new(named_pipe)),
            kanata: Arc::new(Mutex::new(stream)),
            kanata_port,
            configuration,
            default_layer,
            tmpfile,
        })
    }

    #[allow(clippy::too_many_lines)]
    pub fn listen(&mut self) {
        let pipe = self.komorebi.clone();
        let mut stream = self.kanata.clone();
        let stream_read = self.kanata.clone();
        let kanata_port = self.kanata_port;
        let tmpfile = self.tmpfile;
        log::info!("listening");

        std::thread::spawn(move || -> Result<()> {
            let mut read_stream = stream_read.lock().try_clone()?;
            drop(stream_read);

            loop {
                let mut buf = vec![0; 1024];
                match read_stream.read(&mut buf) {
                    Ok(bytes_read) => {
                        let data = String::from_utf8(buf[0..bytes_read].to_vec())?;
                        if data == "\n" {
                            continue;
                        }

                        let notification: serde_json::Value = serde_json::from_str(&data)?;

                        if notification.dot_has("LayerChange.new") {
                            if let Some(new) = notification.dot_get::<String>("LayerChange.new")? {
                                log::info!("current layer: {new}");
                                if tmpfile {
                                    let mut tmp = std::env::temp_dir();
                                    tmp.push("kanata_layer");
                                    std::fs::write(tmp, new)?;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        // Connection reset
                        if error.raw_os_error().expect("could not get raw os error") == 10054 {
                            KANATA_DISCONNECTED.store(true, Ordering::SeqCst);
                            log::warn!("kanata tcp server is no longer running");

                            let mut result = TcpStream::connect(format!("localhost:{kanata_port}"));
                            while result.is_err() {
                                log::warn!("kanata tcp server is not running, retrying connection in 5 seconds");
                                std::thread::sleep(Duration::from_secs(5));
                                result = TcpStream::connect(format!("localhost:{kanata_port}"));
                            }

                            log::info!("reconnected to kanata on read thread");

                            read_stream = result?;

                            KANATA_DISCONNECTED.store(false, Ordering::SeqCst);
                            KANATA_RECONNECT_REQUIRED.store(true, Ordering::SeqCst);
                        }
                    }
                }
            }
        });

        let config = self.configuration.clone();
        let default_layer = self.default_layer.clone();
        std::thread::spawn(move || -> Result<()> {
            let mut buf = vec![0; 4096];

            loop {
                let mut named_pipe = pipe.lock();
                match (*named_pipe).read(&mut buf) {
                    Ok(bytes_read) => {
                        let data = String::from_utf8(buf[0..bytes_read].to_vec())?;
                        if data == "\n" {
                            continue;
                        }

                        let notification: serde_json::Value = match serde_json::from_str(&data) {
                            Ok(value) => value,
                            Err(error) => {
                                log::debug!("discarding malformed komorebi notification: {error}");
                                continue;
                            }
                        };

                        if notification.dot_has("event.content.1.exe") {
                            if let (Some(exe), Some(title), Some(kind)) = (
                                notification.dot_get::<String>("event.content.1.exe")?,
                                notification.dot_get::<String>("event.content.1.title")?,
                                notification.dot_get::<String>("event.type")?,
                            ) {
                                log::debug!("processing komorebi notifcation: {kind}");
                                if KANATA_DISCONNECTED.load(Ordering::SeqCst) {
                                    log::info!("kanata is currently disconnected, will not try to send this ChangeLayer request");
                                    continue;
                                }

                                match kind.as_str() {
                                    "Show" => handle_event(
                                        &config,
                                        &mut stream,
                                        &default_layer,
                                        Event::Show,
                                        &exe,
                                        &title,
                                        kanata_port,
                                    )?,
                                    "FocusChange" => handle_event(
                                        &config,
                                        &mut stream,
                                        &default_layer,
                                        Event::FocusChange,
                                        &exe,
                                        &title,
                                        kanata_port,
                                    )?,
                                    _ => {}
                                };
                            }
                        }
                    }
                    Err(error) => {
                        // Broken pipe
                        if error.raw_os_error().expect("could not get raw os error") == 109 {
                            log::warn!("komorebi is no longer running");
                            named_pipe.disconnect()?;

                            let mut output = Command::new("cmd.exe")
                                .args(["/C", "komorebic.exe", "subscribe", NAME])
                                .output()?;

                            while !output.status.success() {
                                log::warn!(
                                    "komorebic.exe failed with error code {:?}, retrying in 5 seconds...",
                                    output.status.code()
                                );

                                std::thread::sleep(Duration::from_secs(5));

                                output = Command::new("cmd.exe")
                                    .args(["/C", "komorebic.exe", "subscribe", NAME])
                                    .output()?;
                            }

                            log::warn!("reconnected to komorebi");
                            named_pipe.connect()?;
                        } else {
                            return Err(Report::from(error));
                        }
                    }
                }
            }
        });
    }
}

fn handle_event(
    configuration: &Configuration,
    stream: &mut Arc<Mutex<TcpStream>>,
    default_layer: &str,
    event: Event,
    exe: &str,
    title: &str,
    kanata_port: i32,
) -> Result<()> {
    let target = calculate_target(
        configuration,
        event,
        exe,
        title,
        if matches!(event, Event::FocusChange) {
            Option::from(default_layer)
        } else {
            None
        },
    );

    if let Some(target) = target {
        if KANATA_RECONNECT_REQUIRED.load(Ordering::SeqCst) {
            let mut result = TcpStream::connect(format!("localhost:{kanata_port}"));
            while result.is_err() {
                std::thread::sleep(Duration::from_secs(5));
                result = TcpStream::connect(format!("localhost:{kanata_port}"));
            }

            log::info!("reconnected to kanata on write thread");
            *stream = Arc::new(Mutex::new(result?));
            KANATA_RECONNECT_REQUIRED.store(false, Ordering::SeqCst);
        }

        let mut stream = stream.lock();
        let request = json!({
            "ChangeLayer": {
                "new": target,
            }
        });

        stream.write_all(request.to_string().as_bytes())?;
        log::debug!("request sent: {request}");
    };

    Ok(())
}

#[derive(Debug, Copy, Clone)]
pub enum Event {
    Show,
    FocusChange,
}

fn calculate_target(
    configuration: &Configuration,
    event: Event,
    exe: &str,
    title: &str,
    default: Option<&str>,
) -> Option<String> {
    let mut new_layer = default;
    for entry in configuration {
        if entry.exe == exe {
            if matches!(event, Event::FocusChange) {
                new_layer = Option::from(entry.target_layer.as_str());
            }

            if let Some(title_overrides) = &entry.title_overrides {
                for title_override in title_overrides {
                    match title_override.strategy {
                        Strategy::StartsWith => {
                            if title.starts_with(&title_override.title) {
                                new_layer = Option::from(title_override.target_layer.as_str());
                            }
                        }
                        Strategy::EndsWith => {
                            if title.ends_with(&title_override.title) {
                                new_layer = Option::from(title_override.target_layer.as_str());
                            }
                        }
                        Strategy::Contains => {
                            if title.contains(&title_override.title) {
                                new_layer = Option::from(title_override.target_layer.as_str());
                            }
                        }
                        Strategy::Equals => {
                            if title.eq(&title_override.title) {
                                new_layer = Option::from(title_override.target_layer.as_str());
                            }
                        }
                    }
                }

                // This acts like a default target layer within the application
                // which defaults back to the entry's main target layer
                if new_layer.is_none() {
                    new_layer = Option::from(entry.target_layer.as_str());
                }
            }

            if matches!(event, Event::FocusChange) {
                if let Some(virtual_key_overrides) = &entry.virtual_key_overrides {
                    for virtual_key_override in virtual_key_overrides {
                        if unsafe { GetKeyState(virtual_key_override.virtual_key_code) } < 0 {
                            new_layer = Option::from(virtual_key_override.targer_layer.as_str());
                        }
                    }
                }

                if let Some(virtual_key_ignores) = &entry.virtual_key_ignores {
                    for virtual_key in virtual_key_ignores {
                        if unsafe { GetKeyState(*virtual_key) } < 0 {
                            new_layer = None;
                        }
                    }
                }
            }
        }
    }

    new_layer.and_then(|new_layer| Option::from(new_layer.to_string()))
}

fn resolve_windows_path(raw_path: &str) -> Result<PathBuf> {
    let path = if raw_path.starts_with('~') {
        raw_path.replacen(
            '~',
            &dirs::home_dir()
                .ok_or_else(|| anyhow!("there is no home directory"))?
                .display()
                .to_string(),
            1,
        )
    } else {
        raw_path.to_string()
    };

    let full_path = PathBuf::from(path);

    let parent = full_path
        .parent()
        .ok_or_else(|| anyhow!("cannot parse directory"))?;

    let file = full_path
        .components()
        .last()
        .ok_or_else(|| anyhow!("cannot parse filename"))?;

    let mut canonicalized = std::fs::canonicalize(parent)?;
    canonicalized.push(file);

    Ok(canonicalized)
}
