use std::io::Read;

use loggen::output::{FileWriter, StdoutWriter};
use loggen::{LogEntry, LogWriter};

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
        let mut writer = FileWriter::new(path, true, None).unwrap();
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
        let mut writer = FileWriter::new(path, true, None).unwrap();
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
        let writer = FileWriter::new(path, true, None);
        assert!(writer.is_ok());
        assert_eq!(writer.unwrap().path(), path);
    }
    assert!(std::path::Path::new(path).exists());
    std::fs::remove_file(path).unwrap();
}
