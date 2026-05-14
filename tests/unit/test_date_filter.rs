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
        attacks: None,
        attack_only: false,
        num_threads: None,
        progress: None,
        progress_interval: 10000,
    }
}

#[test]
fn test_template_date_filter_no_timezone() {
    let config = Config {
        count: 1,
        logs: Some(vec!["{{ timestamp | date(format=\"%Y-%m-%d\") }}".to_string()]),
        ..test_config()
    };
    let generator = Generator::new(config);
    let entries = generator.generate_with_count(1);
    // Should render date without panic (default %Y-%m-%d)
    assert_eq!(entries[0].message.len(), 10); // YYYY-MM-DD is 10 chars
    let parts: Vec<&str> = entries[0].message.split('-').collect();
    assert_eq!(parts.len(), 3);
}

#[test]
fn test_template_date_filter_with_tz() {
    let config = Config {
        count: 1,
        logs: Some(vec!["{{ timestamp | date(format=\"%Y-%m-%dT%H:%M:%S%z\") }}".to_string()]),
        ..test_config()
    };
    let generator = Generator::new(config);
    let entries = generator.generate_with_count(1);
    // Should render with UTC offset
    assert!(entries[0].message.ends_with("+0000"), "expected +0000 suffix, got: {}", entries[0].message);
}
