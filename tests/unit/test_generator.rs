use std::collections::HashMap;

use loggen::output::{ProgressReporter, StdoutWriter};
use loggen::{Config, Generator, OutputConfig};

fn test_config() -> Config {
    Config {
        output: OutputConfig::default(),
        count: 1,
        log_level: "INFO".to_string(),
        message: "test".to_string(),
        logs: None,
        templates: None,
        template_vars: None,
        seed: None,
        random_vars: None,
        random_intensity: 1.0,
        template_rotation: "sequential".to_string(),
        num_threads: None,
        progress: None,
        progress_interval: 10000,
        simulation: None,
    }
}

fn config_with_logs(logs: Vec<&str>) -> Config {
    Config {
        logs: Some(logs.iter().map(|s| s.to_string()).collect()),
        ..test_config()
    }
}

#[test]
fn test_generate_default_count() {
    let config = Config {
        count: 3,
        ..test_config()
    };
    let generator = Generator::new(config);
    let entries = generator.generate();
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_generate_with_count() {
    let generator = Generator::new(test_config());
    let entries = generator.generate_with_count(5);
    assert_eq!(entries.len(), 5);
}

#[test]
fn test_generate_zero_count() {
    let generator = Generator::new(test_config());
    let entries = generator.generate_with_count(0);
    assert_eq!(entries.len(), 0);
}

#[test]
fn test_generate_entry_content_legacy() {
    let config = Config {
        count: 1,
        log_level: "ERROR".to_string(),
        message: "custom message".to_string(),
        ..test_config()
    };
    let generator = Generator::new(config);
    let entries = generator.generate();
    assert_eq!(entries[0].level, "ERROR");
    assert_eq!(entries[0].message, "custom message #1");
    assert!(!entries[0].timestamp.is_empty());
}

// ── Simulation / Timing Control ──

#[test]
fn test_simulation_delay_stream() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        cancel_clone.store(true, Ordering::SeqCst);
    });

    let config = Config {
        count: 5,
        simulation: Some(loggen::SimulationConfig {
            delay: Some("0-1".to_string()),
            rotation: "none".to_string(),
        }),
        ..test_config()
    };
    let gen = Generator::new_with_cancel(config, cancel);
    let mut writer = StdoutWriter::new();
    let mut progress = ProgressReporter::new(false, 5, 0.0, 1000);
    let result = gen.generate_to_writer_with_progress(&mut writer, &mut progress);
    handle.join().unwrap();
    assert!(result.is_ok());
}

#[test]
fn test_simulation_no_delay() {
    let config = Config {
        count: 3,
        simulation: Some(loggen::SimulationConfig {
            delay: None,
            rotation: "round_robin".to_string(),
        }),
        ..test_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate_with_count(3);
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_simulation_delay_legacy_stream() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        cancel_clone.store(true, Ordering::SeqCst);
    });

    let config = Config {
        count: 3,
        log_level: "INFO".to_string(),
        message: "delay test".to_string(),
        simulation: Some(loggen::SimulationConfig {
            delay: Some("1-2".to_string()),
            rotation: "none".to_string(),
        }),
        ..Config::default()
    };
    let gen = Generator::new_with_cancel(config, cancel);
    let mut writer = StdoutWriter::new();
    let mut progress = ProgressReporter::new(false, 3, 0.0, 1000);
    let result = gen.generate_to_writer_with_progress(&mut writer, &mut progress);
    handle.join().unwrap();
    assert!(result.is_ok());
}

// ── parse_delay_range tests ──

#[test]
fn test_parse_delay_range_valid() {
    let result = loggen::generator::parse_delay_range("100-500");
    assert_eq!(result, Ok((100, 500)));
}

#[test]
fn test_parse_delay_range_zero() {
    let result = loggen::generator::parse_delay_range("0-0");
    assert_eq!(result, Ok((0, 0)));
}

#[test]
fn test_parse_delay_range_invalid_format() {
    let result = loggen::generator::parse_delay_range("abc");
    assert!(result.is_err());
}

#[test]
fn test_parse_delay_range_single_value() {
    let result = loggen::generator::parse_delay_range("100");
    assert_eq!(result, Ok((100, 100)));
}

#[test]
fn test_parse_delay_range_min_gt_max() {
    let result = loggen::generator::parse_delay_range("200-100");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("min") || err.contains("200") || err.contains("100"));
}

#[test]
fn test_parse_delay_range_non_numeric() {
    let result = loggen::generator::parse_delay_range("abc-def");
    assert!(result.is_err());
}

#[test]
fn test_generate_large_count_legacy() {
    let generator = Generator::new(test_config());
    let entries = generator.generate_with_count(1000);
    assert_eq!(entries.len(), 1000);
    assert_eq!(entries[999].message, "test #1000");
}

#[test]
fn test_template_basic_render() {
    let config = config_with_logs(vec!["hello {{ message }}"]);
    let generator = Generator::new(config);
    let entries = generator.generate_with_count(1);
    assert_eq!(entries[0].message, "hello test");
}

#[test]
fn test_template_with_level() {
    let config = Config {
        log_level: "ERROR".to_string(),
        count: 1,
        logs: Some(vec!["[{{ level }}] {{ message }}".to_string()]),
        ..test_config()
    };
    let generator = Generator::new(config);
    let entries = generator.generate_with_count(1);
    assert_eq!(entries[0].message, "[ERROR] test");
}

#[test]
fn test_template_with_index() {
    let config = config_with_logs(vec!["entry {{ index }}"]);
    let generator = Generator::new(config);
    let entries = generator.generate_with_count(3);
    assert_eq!(entries[0].message, "entry 1");
    assert_eq!(entries[1].message, "entry 2");
    assert_eq!(entries[2].message, "entry 3");
}

#[test]
fn test_template_with_template_vars() {
    let config = Config {
        count: 1,
        logs: Some(vec!["{{ app }} v{{ version }}".to_string()]),
        template_vars: Some(HashMap::from([
            ("app".to_string(), "myapp".to_string()),
            ("version".to_string(), "1.0".to_string()),
        ])),
        ..test_config()
    };
    let generator = Generator::new(config);
    let entries = generator.generate_with_count(1);
    assert_eq!(entries[0].message, "myapp v1.0");
}

#[test]
fn test_template_random_vars_resolve() {
    let config = Config {
        count: 5,
        logs: Some(vec!["{{ ipv4 }} - {{ status }}".to_string()]),
        ..test_config()
    };
    let generator = Generator::new(config);
    let entries = generator.generate_with_count(5);
    assert_eq!(entries.len(), 5);
    for entry in &entries {
        assert!(entry.message.contains(" - "));
    }
}

#[test]
fn test_template_unknown_var_panics() {
    let result = std::panic::catch_unwind(|| {
        let config = config_with_logs(vec!["{{ unknown_var }}"]);
        let _generator = Generator::new(config);
    });
    assert!(result.is_err());
}

#[test]
fn test_seeded_reproducibility() {
    let config1 = Config {
        count: 10,
        logs: Some(vec!["{{ ip }} - {{ status }}".to_string()]),
        seed: Some(42),
        ..test_config()
    };
    let config2 = Config {
        count: 10,
        logs: Some(vec!["{{ ip }} - {{ status }}".to_string()]),
        seed: Some(42),
        ..test_config()
    };
    let gen1 = Generator::new(config1);
    let gen2 = Generator::new(config2);
    let entries1 = gen1.generate_with_count(10);
    let entries2 = gen2.generate_with_count(10);
    for (e1, e2) in entries1.iter().zip(entries2.iter()) {
        assert_eq!(e1.message, e2.message);
    }
}

#[test]
fn test_multiple_templates_rotation_sequential() {
    let config = Config {
        count: 4,
        logs: Some(vec!["tplA".to_string(), "tplB".to_string()]),
        ..test_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate_with_count(4);
    assert_eq!(entries[0].message, "tplA");
    assert_eq!(entries[1].message, "tplB");
    assert_eq!(entries[2].message, "tplA");
    assert_eq!(entries[3].message, "tplB");
}

#[test]
fn test_same_generator_reproducibility() {
    let config = Config {
        count: 10,
        logs: Some(vec!["{{ ip }} - {{ status }}".to_string()]),
        seed: Some(42),
        ..test_config()
    };
    let gen = Generator::new(config);
    let entries1 = gen.generate_with_count(10);
    let entries2 = gen.generate_with_count(10);
    for (e1, e2) in entries1.iter().zip(entries2.iter()) {
        assert_eq!(e1.message, e2.message, "same generator should produce same output");
    }
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
    writer.set_template_mode(true);
    let mut progress = ProgressReporter::new(false, 20, 0.0, 10);
    gen.generate_to_writer_with_progress(&mut writer, &mut progress).unwrap();
    progress.done();
}

// ── Timestamp caching ──

#[test]
fn test_generator_timestamp_caching() {
    let config = Config {
        count: 100,
        message: "cached ts".to_string(),
        ..Config::default()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 100);
    let first_ts = &entries[0].timestamp;
    for entry in &entries {
        assert_eq!(&entry.timestamp, first_ts, "timestamps should be cached");
    }
}
