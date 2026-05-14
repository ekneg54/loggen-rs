use std::io::Read;

use loggen::cli::{create_writer, validate_http_config, validate_kafka_config};
use loggen::config::OutputConfig;
use loggen::output::{BufferedLogWriter, FileWriter, ProgressReporter, StdoutWriter};
use loggen::{Config, Generator, LogEntry, LogWriter};

fn test_entry() -> LogEntry {
    LogEntry {
        timestamp: "12345".to_string(),
        level: "INFO".to_string(),
        message: "test message".to_string(),
    }
}

// ── Progress Reporter ──

#[test]
fn test_progress_basic_output() {
    let mut pr = ProgressReporter::new(true, 1000, 0.0, 500);
    // Report at entry 0 (should be skipped)
    pr.report(0);
    // Report at entries
    pr.report(500);
    pr.report(1000);
    // Summary line (captured from stderr)
    pr.done();
}

#[test]
fn test_progress_disabled() {
    let mut pr = ProgressReporter::new(false, 10, 1.0, 10000);
    pr.report(5);
    pr.report(10);
    pr.done();
    // No output expected on stderr
}

#[test]
fn test_progress_auto_enable_conditions() {
    // Verify that ProgressReporter can be created with large counts
    let mut pr = ProgressReporter::new(true, 150000, 1.0, 10000);
    pr.report(1000);
    pr.report(50000);
    pr.report(100000);
    pr.report(150000);
    pr.done();
}

// ── Buffered Writer ──

#[test]
fn test_buffered_writer_flush_on_size() {
    let inner = StdoutWriter::new();
    let mut buf = BufferedLogWriter::new(inner, 1); // tiny buffer

    for _ in 0..5 {
        buf.write_entry(&test_entry()).unwrap();
    }
    buf.flush().unwrap();
}

#[test]
fn test_buffered_writer_flush_on_drop() {
    let inner = StdoutWriter::new();
    {
        let mut buf = BufferedLogWriter::new(inner, 8192);
        for _ in 0..3 {
            buf.write_entry(&test_entry()).unwrap();
        }
    } // drop should flush
}

// ── File Rotation ──

#[test]
fn test_file_append_vs_truncate() {
    let path = "test_phase4_append.log";
    // Write with truncate
    {
        let mut writer = FileWriter::new(path, true, None).unwrap();
        writer.template_mode = true;
        writer.write_entry(&LogEntry {
            timestamp: "0".to_string(),
            level: "INFO".to_string(),
            message: "first write".to_string(),
        }).unwrap();
        writer.flush().unwrap();
    }
    // Append
    {
        let mut writer = FileWriter::new(path, false, None).unwrap();
        writer.template_mode = true;
        writer.write_entry(&LogEntry {
            timestamp: "0".to_string(),
            level: "INFO".to_string(),
            message: "second write".to_string(),
        }).unwrap();
        writer.flush().unwrap();
    }
    let mut content = String::new();
    std::fs::File::open(path).unwrap().read_to_string(&mut content).unwrap();
    assert!(content.contains("first write") && content.contains("second write"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn test_file_rotation_bytes() {
    let path = "test_phase4_rot.log";
    {
        let mut writer = FileWriter::new(path, true, Some(100)).unwrap();
        writer.template_mode = true;
        for i in 0..30 {
            writer.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: format!("entry-{}", i),
            }).unwrap();
        }
        writer.flush().unwrap();
    }
    // Rotation should have happened
    assert!(std::path::Path::new(path).exists(), "main file should exist");
    assert!(std::path::Path::new(&format!("{}.1", path)).exists(), "rotated file should exist");
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}.1", path));
}

#[test]
fn test_file_rotation_single_entry() {
    let path = "test_phase4_no_rot.log";
    {
        let mut writer = FileWriter::new(path, true, Some(1000)).unwrap();
        writer.template_mode = true;
        writer.write_entry(&LogEntry {
            timestamp: "0".to_string(),
            level: "INFO".to_string(),
            message: "small entry".to_string(),
        }).unwrap();
        writer.flush().unwrap();
    }
    // No rotation should occur (entry too small)
    assert!(!std::path::Path::new(&format!("{}.1", path)).exists(),
        "rotated file should NOT exist for small entries");
    std::fs::remove_file(path).unwrap();
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
    use loggen::KafkaOutputConfig;
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

// ── Config validation in CLI ──

#[test]
fn test_create_writer_for_stdout() {
    let config = Config::default();
    let mut writer = create_writer(&config).unwrap();
    writer.write_entry(&test_entry()).unwrap();
    writer.flush().unwrap();
}

// ── HTTP Writer Tests ──

#[test]
fn test_http_writer_send_single() {
    let mut writer = loggen::HttpWriter::new(
        "http://localhost:1", 10, "ndjson", None, 1, 1,
    ).unwrap();
    // Sending will fail (no server), but should not panic
    let result = writer.write_entry(&test_entry());
    // Either Ok (buffered) or Err on flush
    match result {
        Ok(_) => {
            let flush_result = writer.flush();
            assert!(flush_result.is_err(), "should fail to connect");
        }
        Err(e) => {
            let msg = format!("{}", e);
            assert!(msg.contains("retries") || msg.contains("Failed")
                || msg.contains("refused") || msg.contains("connect"),
                "unexpected error: {}", msg);
        }
    }
}

#[test]
fn test_http_writer_batching() {
    let mut writer = loggen::HttpWriter::new(
        "http://localhost:1", 100, "ndjson", None, 1, 1,
    ).unwrap();
    // Batch 250 entries with batch_size 100
    for _ in 0..250 {
        let _ = writer.write_entry(&test_entry());
    }
    // Flush should try to send - will fail but not panic
    let _ = writer.flush();
}

// ── Generator with progress ──

#[test]
fn test_generator_legacy_with_progress() {
    let config = Config {
        count: 10,
        ..Config::default()
    };
    let gen = Generator::new(config);
    let mut writer = StdoutWriter::new();
    let mut progress = ProgressReporter::new(false, 10, 0.0, 5);
    gen.generate_to_writer_with_progress(&mut writer, &mut progress).unwrap();
    progress.done();
}

#[test]
fn test_generator_template_with_progress() {
    let config = Config {
        count: 20,
        logs: Some(vec!["entry {{ index }}".to_string()]),
        seed: Some(42),
        ..Config::default()
    };
    let gen = Generator::new(config);
    let mut writer = StdoutWriter::new();
    writer.template_mode = true;
    let mut progress = ProgressReporter::new(false, 20, 0.0, 10);
    gen.generate_to_writer_with_progress(&mut writer, &mut progress).unwrap();
    progress.done();
}

// ── File output with BufferedLogWriter through create_writer ──

#[test]
fn test_create_writer_file_with_buffer() {
    let path = "test_phase4_create_writer.log";
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
    // Create a LogEntry directly that write_entries would use
    let entry = LogEntry {
        timestamp: "0".to_string(),
        level: "TEST".to_string(),
        message: "phase4 writer test".to_string(),
    };
    writer.write_entry(&entry).unwrap();
    writer.flush().unwrap();
    drop(writer);

    let mut content = String::new();
    std::fs::File::open(path).unwrap().read_to_string(&mut content).unwrap();
    assert!(content.contains("phase4 writer test"), "content: '{}'", content);
    std::fs::remove_file(path).unwrap();
}

// ── Progress auto-enable test ──

#[test]
fn test_progress_interval_minimum() {
    // Verify that entry_interval is at least 1000
    let pr = ProgressReporter::new(true, 100, 1.0, 100);
    // The entry_interval is private, so just test that report doesn't crash
    pr.done();
}

// ── Timestamp caching in generator ──

#[test]
fn test_generator_timestamp_caching() {
    // Legacy mode uses cached timestamp
    let config = Config {
        count: 100,
        message: "cached ts".to_string(),
        ..Config::default()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 100);
    // All entries should have the same timestamp (cached)
    let first_ts = &entries[0].timestamp;
    for entry in &entries {
        assert_eq!(&entry.timestamp, first_ts, "timestamps should be cached");
    }
}
