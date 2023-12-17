use color_eyre::eyre::Result;
use std::path::PathBuf;
use crate::Provider;

pub struct Komokana {
}

impl Provider for Komokana {
    fn init() -> Result<Self>
    where
        Self: Sized {
        todo!()
    }

    fn listen(self) {
        todo!()
    }

    fn resolve_config_path(config: &str) -> Result<PathBuf> {
        todo!()
    }
}
