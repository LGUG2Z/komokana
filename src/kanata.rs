use color_eyre::Result;
use json_dotpath::DotPaths;
use parking_lot::Mutex;
use std::io::Read;
use std::net::TcpStream;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use crate::TMPFILE;

pub static KANATA_DISCONNECTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct Kanata {
    stream: Arc<Mutex<TcpStream>>,
    port: i32,
}

impl Kanata {
    pub fn new(port: i32) -> Result<Self> {
        Ok(Self {
            stream: Arc::new(Mutex::new(Self::connect_to_kanata(port)?)),
            port,
        })
    }

    pub fn get_stream(&self) -> Arc<Mutex<TcpStream>> {
        self.stream.clone()
    }

    fn connect_to_kanata(port: i32) -> Result<TcpStream> {
        Ok(TcpStream::connect(format!("localhost:{port}"))?)
    }

    fn re_establish_connection(&self) -> Result<TcpStream> {
        KANATA_DISCONNECTED.store(true, Ordering::SeqCst);
        log::warn!("kanata tcp server is no longer running");

        let mut result = Self::connect_to_kanata(self.port);
        while result.is_err() {
            log::warn!("kanata tcp server is not running, retrying connection in 5 seconds");
            std::thread::sleep(Duration::from_secs(5));
            result = Self::connect_to_kanata(self.port);
        }

        log::info!("reconnected to kanata on read thread");
        KANATA_DISCONNECTED.store(false, Ordering::SeqCst);
        result
    }

    pub fn spawn_kanata_listener(&'static self) {
        let stream_read = self.get_stream();
        let tmpfile = TMPFILE.get().unwrap().to_owned();
        log::info!("listening");

        std::thread::spawn(move || -> Result<()> {
            let mut reader = stream_read.lock();
            let mut read_stream = reader.try_clone()?;

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
                            *reader = self.re_establish_connection()?;
                            read_stream = reader.try_clone()?;
                        }
                    }
                }
            }
        });
    }
}
