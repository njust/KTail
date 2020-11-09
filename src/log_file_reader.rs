use std::path::PathBuf;
use crate::util::{read};
use std::io::{Seek, SeekFrom};
use std::error::Error;
use async_trait::async_trait;
use crate::model::{LogReader, LogState};

pub struct LogFileReader {
    path: PathBuf,
    file: std::fs::File,
    offset: u64,
}

impl LogFileReader {
    pub fn new(path: PathBuf) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(&path)?;
        Ok(Self {
            path,
            file,
            offset: 0
        })
    }
}

#[async_trait]
impl LogReader for LogFileReader {
    async fn read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        if self.offset > 0 {
            self.file.seek(SeekFrom::Start(self.offset))?;
        }
        let read = read(&mut self.file)?;
        self.offset += read.len() as u64;
        Ok(read)
    }

    async fn init(&mut self) {
    }

    fn check_changes(&mut self) -> LogState {
        if !self.path.exists() {
            return LogState::Skip;
        }

        if let Ok(metadata) = std::fs::metadata(&self.path) {
            let len = metadata.len();
            if len <= 0 {
                return LogState::Skip;
            }
            if len < self.offset {
                self.offset = 0;
                return LogState::Reload;
            }
        }

        return LogState::Ok;
    }

    fn stop(&mut self) {
    }
}