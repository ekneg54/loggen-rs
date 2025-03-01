use std::fs::File;
use std::io::Write;
use loggen_rs::{read_yaml_file, Config};


#[cfg(test)]
mod tests {
  #[test]
  fn test_addition() {
    assert_eq!(2 + 2, 4);
  }
}
#[test]
fn test_read_yaml_file() {
  
  // Create a temporary YAML file
  let test_file_path = "test_config.yaml";
  let yaml_content = "\
name: test_app
version: 1.0
settings:
  debug: true
  max_connections: 100
  tags:
  - test
  - yaml
";
  
  // Write test content to file
  {
    let mut file = File::create(test_file_path).unwrap();
    file.write_all(yaml_content.as_bytes()).unwrap();
  }
  
  // Test the function
  let result = read_yaml_file::<Config, &str>(test_file_path);
  
  // Clean up
  std::fs::remove_file(test_file_path).unwrap();
  
  // Basic assertion
  assert!(result.is_ok(), "Failed to read valid YAML file");
  
  // Further assertions would depend on the actual return type of read_yaml_file
  // For example:
  // let data = result.unwrap();
  // assert_eq!(data.name, "test_app");
  // assert_eq!(data.version, "1.0");
}
