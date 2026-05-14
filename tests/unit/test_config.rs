use std::fs::File;
use std::io::Write;
use loggen::{read_yaml_file, Config};

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
