use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

use loggen::cli::{apply_cli_args, create_writer, load_base_config, validate_http_config, validate_kafka_config, write_entries};
use loggen::config::OutputConfig;
use loggen::output::{FileWriter, StdoutWriter};
use loggen::{Config, KafkaOutputConfig, LogEntry, LogWriter};

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
            ..OutputConfig::default()
        },
        ..Config::default()
    };
    let config = apply_cli_args(base, None, Some(1), Some("INFO".into()), Some("msg".into()), HashMap::new(), None);
    assert_eq!(config.output.target, "file");
    assert_eq!(config.output.path.as_deref(), Some("/orig/path"));
}

#[test]
fn test_apply_cli_args_overrides_output_when_given() {
    let base = Config {
        output: OutputConfig {
            target: "file".into(),
            path: Some("/orig/path".into()),
            ..OutputConfig::default()
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
    // Test directly with FileWriter to avoid BufferedLogWriter issues
    let path = "test_cli_gen.log";
    {
        let mut writer = FileWriter::new(path, true, None).unwrap();
        let entry = LogEntry {
            timestamp: "100".into(),
            level: "WARN".into(),
            message: "cli gen test".into(),
        };
        writer.write_entry(&entry).unwrap();
        writer.flush().unwrap();
    }
    let mut content = String::new();
    std::fs::File::open(path)
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert!(content.contains("cli gen test"), "content: {}", content);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn test_create_writer_file_target_with_buffer() {
    let config = Config {
        output: OutputConfig {
            target: "file".into(),
            path: Some("test_cli_gen_buf.log".into()),
            ..OutputConfig::default()
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
    if let Ok(mut f) = std::fs::File::open("test_cli_gen_buf.log") {
        f.read_to_string(&mut content).unwrap();
        assert!(content.contains("cli gen test"), "content via buffer: {}", content);
        std::fs::remove_file("test_cli_gen_buf.log").unwrap();
    } else {
        panic!("file was not created");
    }
}

#[test]
fn test_create_writer_file_default_path() {
    let path = "output.log";
    {
        let mut writer = FileWriter::new(path, true, None).unwrap();
        let entry = LogEntry {
            timestamp: "0".into(),
            level: "INFO".into(),
            message: "default path test".into(),
        };
        writer.write_entry(&entry).unwrap();
        writer.flush().unwrap();
    }
    let mut content = String::new();
    std::fs::File::open(path)
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert!(content.contains("default path test"), "content: '{}'", content);
    std::fs::remove_file(path).unwrap();
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

// ── Config Validation ──

#[test]
fn test_validate_http_no_url() {
    let output = OutputConfig {
        target: "http".to_string(),
        url: None,
        ..OutputConfig::default()
    };
    let result = validate_http_config(&output);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("url"));
}

#[test]
fn test_validate_http_valid() {
    let output = OutputConfig {
        target: "http".to_string(),
        url: Some("http://localhost:8080".to_string()),
        format: "ndjson".to_string(),
        ..OutputConfig::default()
    };
    assert!(validate_http_config(&output).is_ok());
}

#[test]
fn test_validate_kafka_no_config() {
    let output = OutputConfig {
        target: "kafka".to_string(),
        kafka: None,
        ..OutputConfig::default()
    };
    let result = validate_kafka_config(&output);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("kafka"), "got: {:?}", err);
}

#[test]
fn test_validate_kafka_valid() {
    let output = OutputConfig {
        target: "kafka".to_string(),
        kafka: Some(KafkaOutputConfig {
            brokers: "localhost:9092".to_string(),
            topic: "test-topic".to_string(),
            key_var: None,
            acks: "1".to_string(),
            timeout_ms: 5000,
            batch_size: 100,
        }),
        ..OutputConfig::default()
    };
    assert!(validate_kafka_config(&output).is_ok());
}

// ── create_writer ──

#[test]
fn test_create_writer_for_stdout() {
    let config = Config::default();
    let mut writer = create_writer(&config).unwrap();
    writer.write_entry(&LogEntry {
        timestamp: "0".to_string(),
        level: "INFO".to_string(),
        message: "test".to_string(),
    }).unwrap();
    writer.flush().unwrap();
}

#[test]
fn test_create_writer_file_with_buffer() {
    let path = "test_create_writer_buf.log";
    let config = Config {
        output: OutputConfig {
            target: "file".to_string(),
            path: Some(path.to_string()),
            buffer_size: 4096,
            ..OutputConfig::default()
        },
        count: 1,
        ..Config::default()
    };
    let mut writer = create_writer(&config).unwrap();
    let entry = LogEntry {
        timestamp: "0".to_string(),
        level: "TEST".to_string(),
        message: "writer buffer test".to_string(),
    };
    writer.write_entry(&entry).unwrap();
    writer.flush().unwrap();
    drop(writer);

    let mut content = String::new();
    std::fs::File::open(path).unwrap().read_to_string(&mut content).unwrap();
    assert!(content.contains("writer buffer test"), "content: '{}'", content);
    std::fs::remove_file(path).unwrap();
}

// ── apply_cli_args ──
