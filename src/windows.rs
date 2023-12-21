use color_eyre::eyre::anyhow;
use color_eyre::Report;
use color_eyre::Result;
use json_dotpath::DotPaths;
use miow::pipe::NamedPipe;
use parking_lot::Mutex;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "virtual_keys")]
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;

use crate::events::{handle_event, Event};
use crate::Provider;

const PIPE: &str = r#"\\.\pipe\"#;
const NAME: &str = "komokana";

pub struct Komokana {
    komorebi: Arc<Mutex<NamedPipe>>,
}

impl Komokana {
    fn connect_to_komorebi() -> io::Result<Output> {
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
        Ok(output)
    }
}

impl Provider for Komokana {
    fn init() -> Result<Self> {
        let pipe = format!("{}\\{}", PIPE, NAME);
        let named_pipe = NamedPipe::new(pipe)?;

        Self::connect_to_komorebi()?;

        named_pipe.connect()?;
        log::debug!("connected to komorebi");

        Ok(Komokana {
            komorebi: Arc::new(Mutex::new(named_pipe)),
        })
    }

    fn listen(self) {
        let pipe = self.komorebi.clone();
        std::thread::spawn(move || -> Result<()> {
            let mut buf = vec![0; 8192];

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
                                if crate::kanata::KANATA_DISCONNECTED.load(Ordering::SeqCst) {
                                    log::info!("kanata is currently disconnected, will not try to send this ChangeLayer request");
                                    continue;
                                }

                                match kind.as_str() {
                                    "Show" => handle_event(Event::Show, &exe, Some(&title))?,
                                    "FocusChange" => {
                                        handle_event(Event::FocusChange, &exe, Some(&title))?
                                    }
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
                            Self::connect_to_komorebi()?;
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

    #[cfg(feature = "virtual_keys")]
    fn get_key_state(key_code: i32) -> i16 {
        unsafe { GetKeyState(key_code) }
    }

    fn resolve_config_path(raw_path: &str) -> Result<PathBuf> {
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
}
