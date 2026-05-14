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

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn test_entry() -> LogEntry {
        LogEntry {
            timestamp: "12345".to_string(),
            level: "INFO".to_string(),
            message: "test message #1".to_string(),
        }
    }

    #[test]
    fn test_stdout_writer_write() {
        let mut writer = StdoutWriter::new();
        let entry = test_entry();
        assert!(writer.write_entry(&entry).is_ok());
    }

    #[test]
    fn test_stdout_writer_flush() {
        let mut writer = StdoutWriter::new();
        assert!(writer.flush().is_ok());
    }

    #[test]
    fn test_stdout_writer_template_mode() {
        let mut writer = StdoutWriter { template_mode: true };
        let entry = LogEntry {
            timestamp: "12345".to_string(),
            level: "INFO".to_string(),
            message: "test message".to_string(),
        };
        assert!(writer.write_entry(&entry).is_ok());
    }

    #[test]
    fn test_file_writer_template_mode() {
        let path = "test_template_mode.log";
        {
            let mut writer = FileWriter::new(path).unwrap();
            writer.template_mode = true;
            let entry = LogEntry {
                timestamp: "12345".to_string(),
                level: "INFO".to_string(),
                message: "template output only".to_string(),
            };
            writer.write_entry(&entry).unwrap();
            writer.flush().unwrap();
        }
        let mut content = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(!content.contains("[12345]"));
        assert!(content.contains("template output only"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_file_writer_write_and_read() {
        let path = "test_output.log";
        let entry = test_entry();

        {
            let mut writer = FileWriter::new(path).unwrap();
            writer.write_entry(&entry).unwrap();
            writer.flush().unwrap();
        }

        let mut content = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("[12345] [INFO] test message #1"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_file_writer_new_creates_file() {
        let path = "test_new_file.log";
        {
            let writer = FileWriter::new(path);
            assert!(writer.is_ok());
            assert_eq!(writer.unwrap().path(), path);
        }
        assert!(std::path::Path::new(path).exists());
        std::fs::remove_file(path).unwrap();
    }
}
