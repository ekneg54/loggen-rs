use std::collections::HashMap;
use std::{fs::File, io::Read, path::Path};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_output_target")]
    pub target: String,
    pub path: Option<String>,
}

fn default_output_target() -> String {
    "stdout".to_string()
}

impl Default for OutputConfig {
    fn default() -> Self {
        OutputConfig {
            target: default_output_target(),
            path: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default = "default_count")]
    pub count: u64,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_message")]
    pub message: String,

    // Phase 2: Template System
    #[serde(default)]
    pub logs: Option<Vec<String>>,
    #[serde(default)]
    pub logs_dir: Option<String>,
    #[serde(default)]
    pub template_vars: Option<HashMap<String, String>>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub random_vars: Option<HashMap<String, Vec<String>>>,
    #[serde(default = "default_random_intensity")]
    pub random_intensity: f64,
    #[serde(default = "default_template_rotation")]
    pub template_rotation: String,
}

fn default_count() -> u64 {
    1
}

fn default_log_level() -> String {
    "INFO".to_string()
}

fn default_message() -> String {
    "Log entry generated".to_string()
}

fn default_random_intensity() -> f64 {
    1.0
}

fn default_template_rotation() -> String {
    "sequential".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            output: OutputConfig::default(),
            count: default_count(),
            log_level: default_log_level(),
            message: default_message(),
            logs: None,
            logs_dir: None,
            template_vars: None,
            seed: None,
            random_vars: None,
            random_intensity: default_random_intensity(),
            template_rotation: default_template_rotation(),
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        read_yaml_file(path)
    }
}

pub fn read_yaml_file<T, P>(path: P) -> Result<T, Box<dyn std::error::Error>>
where
    T: for<'de> Deserialize<'de>,
    P: AsRef<Path>,
{
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let parsed: T = serde_yaml::from_str(&content)?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.count, 1);
        assert_eq!(config.log_level, "INFO");
        assert_eq!(config.message, "Log entry generated");
        assert_eq!(config.output.target, "stdout");
        assert!(config.output.path.is_none());
        assert!(config.logs.is_none());
        assert!(config.logs_dir.is_none());
        assert!(config.template_vars.is_none());
        assert!(config.seed.is_none());
        assert!(config.random_vars.is_none());
        assert_eq!(config.random_intensity, 1.0);
        assert_eq!(config.template_rotation, "sequential");
    }

    #[test]
    fn test_config_from_yaml_full() {
        let yaml = r#"
output:
  target: file
  path: /tmp/test.log
count: 10
log_level: ERROR
message: test error
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.output.target, "file");
        assert_eq!(config.output.path.as_deref(), Some("/tmp/test.log"));
        assert_eq!(config.count, 10);
        assert_eq!(config.log_level, "ERROR");
        assert_eq!(config.message, "test error");
    }

    #[test]
    fn test_config_yaml_with_templates() {
        let yaml = r#"
count: 5
logs:
  - "{{ ip }} - {{ status }}"
logs_dir: /tmp/logs
template_vars:
  app: myapp
seed: 42
random_vars:
  codes: [200, 404, 500]
random_intensity: 0.8
template_rotation: round_robin
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.count, 5);
        assert_eq!(config.logs, Some(vec!["{{ ip }} - {{ status }}".to_string()]));
        assert_eq!(config.logs_dir, Some("/tmp/logs".to_string()));
        let vars = config.template_vars.unwrap();
        assert_eq!(vars.get("app").unwrap(), "myapp");
        assert_eq!(config.seed, Some(42));
        let rv = config.random_vars.unwrap();
        assert_eq!(rv.get("codes").unwrap(), &vec!["200", "404", "500"]);
        assert!((config.random_intensity - 0.8).abs() < 1e-6);
        assert_eq!(config.template_rotation, "round_robin");
    }

    #[test]
    fn test_config_partial_yaml() {
        let yaml = "count: 5\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.count, 5);
        assert_eq!(config.log_level, "INFO");
        assert_eq!(config.message, "Log entry generated");
    }

    #[test]
    fn test_read_yaml_file() {
        use std::io::Write;

        let test_file_path = "test_config_unit.yaml";
        let yaml_content = "count: 3\nlog_level: DEBUG\n";

        {
            let mut file = File::create(test_file_path).unwrap();
            file.write_all(yaml_content.as_bytes()).unwrap();
        }

        let result = read_yaml_file::<Config, &str>(test_file_path);
        std::fs::remove_file(test_file_path).unwrap();

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.count, 3);
        assert_eq!(config.log_level, "DEBUG");
    }
}
