use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::config::LogEntry;

pub trait LogWriter {
    fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>>;
    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct StdoutWriter {
    pub template_mode: bool,
}

impl StdoutWriter {
    pub fn new() -> Self {
        StdoutWriter { template_mode: false }
    }
}

impl Default for StdoutWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl LogWriter for StdoutWriter {
    fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
        if self.template_mode {
            println!("{}", entry.message);
        } else {
            println!("[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

pub struct FileWriter {
    file: File,
    path: String,
    pub template_mode: bool,
}

impl FileWriter {
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(FileWriter {
            file,
            path: path.to_string(),
            template_mode: false,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl LogWriter for FileWriter {
    fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
        if self.template_mode {
            writeln!(self.file, "{}", entry.message)?;
        } else {
            writeln!(self.file, "[{}] [{}] {}", entry.timestamp, entry.level, entry.message)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.file.flush()?;
        Ok(())
    }
}


