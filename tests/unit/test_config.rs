use std::fs::File;
use std::io::Write;
use loggen::{read_yaml_file, Config};

#[test]
fn test_example_config() {
    let config: Config = read_yaml_file("examples/example.yaml").unwrap();
    assert_eq!(config.count, 10);
    assert_eq!(config.log_level, "INFO");
    assert_eq!(config.message, "Example log entry");
    assert_eq!(config.output.target, "stdout");
    assert!(config.output.path.is_none());
}

#[test]
fn test_example_file_output_config() {
    let config: Config = read_yaml_file("examples/file-output.yaml").unwrap();
    assert_eq!(config.count, 100);
    assert_eq!(config.log_level, "ERROR");
    assert_eq!(config.message, "File output test");
    assert_eq!(config.output.target, "file");
    assert_eq!(config.output.path.as_deref(), Some("/tmp/loggen-example.log"));
}

#[test]
fn test_example_minimal_config() {
    let config: Config = read_yaml_file("examples/minimal.yaml").unwrap();
    assert_eq!(config.count, 5);
    assert_eq!(config.log_level, "INFO");
    assert_eq!(config.message, "Log entry generated");
}

#[test]
fn test_read_yaml_file() {
    let test_file_path = "test_config.yaml";
    let yaml_content = "\
count: 5
log_level: DEBUG
message: integration test
";

    {
        let mut file = File::create(test_file_path).unwrap();
        file.write_all(yaml_content.as_bytes()).unwrap();
    }

    let result = read_yaml_file::<Config, &str>(test_file_path);
    std::fs::remove_file(test_file_path).unwrap();

    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.count, 5);
    assert_eq!(config.log_level, "DEBUG");
    assert_eq!(config.message, "integration test");
}
