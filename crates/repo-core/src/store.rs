use crate::types::*;
use fs2::FileExt;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct Store {
    directory: PathBuf,
    mutex: Mutex<()>,
}
impl Store {
    pub fn new(directory: impl Into<PathBuf>) -> Result<Self, String> {
        let directory = directory.into();
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        Ok(Self {
            directory,
            mutex: Mutex::new(()),
        })
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    fn load_unlocked(&self) -> Result<AppData, String> {
        let path = self.directory.join("state.json");
        match fs::read(path) {
            Ok(bytes) => {
                let data: AppData = serde_json::from_slice(&bytes).map_err(|e| {
                    format!(
                        "Cannot read saved review data: {e}. The original file has been preserved."
                    )
                })?;
                if data.schema_version != 1 {
                    return Err("This review store was created by a newer version of Peer".into());
                }
                Ok(data)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppData::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn read(&self) -> Result<AppData, String> {
        let _guard = self.mutex.lock().map_err(|e| e.to_string())?;
        self.load_unlocked()
    }
    pub fn update<T>(
        &self,
        action: impl FnOnce(&mut AppData) -> Result<T, String>,
    ) -> Result<T, String> {
        let _guard = self.mutex.lock().map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.directory.join("state.lock"))
            .map_err(|e| e.to_string())?;
        lock.lock_exclusive().map_err(|e| e.to_string())?;
        let mut data = self.load_unlocked()?;
        let result = action(&mut data)?;
        let bytes = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
        let mut file =
            tempfile::NamedTempFile::new_in(&self.directory).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(self.directory.join("state.json"))
            .map_err(|e| e.to_string())?;
        Ok(result)
    }
}
