use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

use loggen::cli::{apply_cli_args, create_writer, load_base_config, write_entries};
use loggen::config::OutputConfig;
use loggen::output::StdoutWriter;
use loggen::{Config, LogEntry, LogWriter};

#[test]
fn test_load_base_config_none_returns_default() {
    let config = load_base_config(None);
    assert_eq!(config.count, 1);
    assert_eq!(config.log_level, "INFO");
}

#[test]
fn test_load_base_config_bad_path_returns_default() {
    let path = PathBuf::from("/nonexistent/path.yaml");
    let config = load_base_config(Some(&path));
    assert_eq!(config.count, 1);
}

#[test]
fn test_apply_cli_args_overrides_all() {
    let config = apply_cli_args(
        Config::default(),
        Some("out.log".into()),
        Some(99),
        Some("ERROR".into()),
        Some("test msg".into()),
        HashMap::new(),
        None,
        Vec::new(),
        false,
    );
    assert_eq!(config.count, 99);
    assert_eq!(config.log_level, "ERROR");
    assert_eq!(config.message, "test msg");
    assert_eq!(config.output.target, "file");
    assert_eq!(config.output.path.as_deref(), Some("out.log"));
}

#[test]
fn test_apply_cli_args_preserves_output_when_not_given() {
    let base = Config {
        output: OutputConfig {
            target: "file".into(),
            path: Some("/orig/path".into()),
        },
        ..Config::default()
    };
    let config = apply_cli_args(base, None, Some(1), Some("INFO".into()), Some("msg".into()), HashMap::new(), None, Vec::new(), false);
    assert_eq!(config.output.target, "file");
    assert_eq!(config.output.path.as_deref(), Some("/orig/path"));
}

#[test]
fn test_apply_cli_args_overrides_output_when_given() {
    let base = Config {
        output: OutputConfig {
            target: "file".into(),
            path: Some("/orig/path".into()),
        },
        ..Config::default()
    };
    let config = apply_cli_args(
        base,
        Some("/new/path".into()),
        Some(1),
        Some("INFO".into()),
        Some("msg".into()),
        HashMap::new(),
        None,
        Vec::new(),
        false,
    );
    assert_eq!(config.output.target, "file");
    assert_eq!(config.output.path.as_deref(), Some("/new/path"));
}

#[test]
fn test_create_writer_stdout_target() {
    let config = Config::default();
    let _ = create_writer(&config).unwrap();
}

#[test]
fn test_create_writer_file_target() {
    let config = Config {
        output: OutputConfig {
            target: "file".into(),
            path: Some("test_cli_gen.log".into()),
        },
        ..Config::default()
    };
    let mut writer = create_writer(&config).unwrap();
    let entry = LogEntry {
        timestamp: "100".into(),
        level: "WARN".into(),
        message: "cli gen test".into(),
    };
    writer.write_entry(&entry).unwrap();
    writer.flush().unwrap();
    drop(writer);

    let mut content = String::new();
    std::fs::File::open("test_cli_gen.log")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert!(content.contains("cli gen test"));
    std::fs::remove_file("test_cli_gen.log").unwrap();
}

#[test]
fn test_create_writer_file_default_path() {
    let config = Config {
        output: OutputConfig {
            target: "file".into(),
            path: None,
        },
        ..Config::default()
    };
    let mut writer = create_writer(&config).unwrap();
    let entry = LogEntry {
        timestamp: "0".into(),
        level: "INFO".into(),
        message: "default path test".into(),
    };
    writer.write_entry(&entry).unwrap();
    writer.flush().unwrap();
    drop(writer);

    let mut content = String::new();
    std::fs::File::open("output.log")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert!(content.contains("default path test"));
    std::fs::remove_file("output.log").unwrap();
}

#[test]
fn test_write_entries_multiple_entries() {
    let mut writer: Box<dyn LogWriter> = Box::new(StdoutWriter::new());
    let entries = vec![
        LogEntry {
            timestamp: "1".into(),
            level: "INFO".into(),
            message: "first".into(),
        },
        LogEntry {
            timestamp: "2".into(),
            level: "ERROR".into(),
            message: "second".into(),
        },
        LogEntry {
            timestamp: "3".into(),
            level: "DEBUG".into(),
            message: "third".into(),
        },
    ];
    write_entries(&mut writer, &entries);
}

#[test]
fn test_write_entries_empty() {
    let mut writer: Box<dyn LogWriter> = Box::new(StdoutWriter::new());
    write_entries(&mut writer, &[]);
}
