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

    // Phase 4: Performance
    #[serde(default = "default_buffer_size")]
    pub buffer_size: u64,
    #[serde(default)]
    pub progress: Option<bool>,
    #[serde(default = "default_progress_interval")]
    pub progress_interval: u64,

    // Phase 4: HTTP output
    pub url: Option<String>,
    #[serde(default = "default_batch_size")]
    pub batch_size: u64,
    #[serde(default = "default_output_format")]
    pub format: String,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,

    // Phase 4: Kafka output
    pub kafka: Option<KafkaOutputConfig>,

    // Phase 4: File enhancements
    #[serde(default = "default_append")]
    pub append: bool,
    #[serde(default)]
    pub rotate_bytes: Option<u64>,
}

fn default_output_target() -> String {
    "stdout".to_string()
}

fn default_buffer_size() -> u64 {
    8192
}

fn default_progress_interval() -> u64 {
    10000
}

fn default_batch_size() -> u64 {
    100
}

fn default_output_format() -> String {
    "ndjson".to_string()
}

fn default_retry_attempts() -> u32 {
    3
}

fn default_retry_delay_ms() -> u64 {
    1000
}

fn default_append() -> bool {
    true
}

impl Default for OutputConfig {
    fn default() -> Self {
        OutputConfig {
            target: default_output_target(),
            path: None,
            buffer_size: default_buffer_size(),
            progress: None,
            progress_interval: default_progress_interval(),
            url: None,
            batch_size: default_batch_size(),
            format: default_output_format(),
            headers: None,
            retry_attempts: default_retry_attempts(),
            retry_delay_ms: default_retry_delay_ms(),
            kafka: None,
            append: default_append(),
            rotate_bytes: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct KafkaOutputConfig {
    #[serde(default = "default_kafka_brokers")]
    pub brokers: String,
    pub topic: String,
    #[serde(default)]
    pub key_var: Option<String>,
    #[serde(default = "default_kafka_acks")]
    pub acks: String,
    #[serde(default = "default_kafka_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_kafka_batch")]
    pub batch_size: u64,
}

fn default_kafka_brokers() -> String {
    "localhost:9092".to_string()
}

fn default_kafka_acks() -> String {
    "1".to_string()
}

fn default_kafka_timeout() -> u64 {
    5000
}

fn default_kafka_batch() -> u64 {
    100
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimulationConfig {
    #[serde(default)]
    pub delay: Option<String>,
    #[serde(default = "default_sim_rotation")]
    pub rotation: String,
}

fn default_sim_rotation() -> String {
    "none".to_string()
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
    pub templates: Option<String>,
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

    // Phase 4: Performance & Advanced
    #[serde(default)]
    pub num_threads: Option<usize>,
    #[serde(default)]
    pub progress: Option<bool>,
    #[serde(default = "default_progress_interval_config")]
    pub progress_interval: u64,

    // Phase 7: Simulation & Timing Control
    #[serde(default)]
    pub simulation: Option<SimulationConfig>,
}

fn default_progress_interval_config() -> u64 {
    10000
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
            templates: None,
            template_vars: None,
            seed: None,
            random_vars: None,
            random_intensity: default_random_intensity(),
            template_rotation: default_template_rotation(),
            num_threads: None,
            progress: None,
            progress_interval: default_progress_interval_config(),
            simulation: None,
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        read_yaml_file(path)
    }

    pub fn has_templates(&self) -> bool {
        self.logs.as_ref().is_some_and(|v| !v.is_empty()) || self.templates.is_some()
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
        assert!(config.templates.is_none());
        assert!(config.template_vars.is_none());
        assert!(config.seed.is_none());
        assert!(config.random_vars.is_none());
        assert_eq!(config.random_intensity, 1.0);
        assert_eq!(config.template_rotation, "sequential");
        assert!(config.num_threads.is_none());
        assert!(config.progress.is_none());
        assert_eq!(config.progress_interval, 10000);
        assert_eq!(config.output.buffer_size, 8192);
        assert!(config.output.url.is_none());
        assert_eq!(config.output.batch_size, 100);
        assert_eq!(config.output.format, "ndjson");
        assert!(config.output.kafka.is_none());
        assert!(config.output.append);
        assert!(config.output.rotate_bytes.is_none());
        assert!(config.simulation.is_none());
    }

    #[test]
    fn test_simulation_yaml_deser() {
        let yaml = r#"
count: 100
simulation:
  delay: "200-1000"
  rotation: round_robin
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let sim = config.simulation.expect("simulation should be present");
        assert_eq!(sim.delay.as_deref(), Some("200-1000"));
        assert_eq!(sim.rotation, "round_robin");
    }

    #[test]
    fn test_simulation_rotation_default() {
        let yaml = r#"
count: 5
simulation:
  delay: "100-500"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let sim = config.simulation.unwrap();
        assert_eq!(sim.delay.as_deref(), Some("100-500"));
        assert_eq!(sim.rotation, "none");
    }

    #[test]
    fn test_simulation_delay_parsing() {
        let yaml = r#"
count: 5
simulation:
  delay: "50-200"
  rotation: random
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let sim = config.simulation.unwrap();
        assert_eq!(sim.delay.as_deref(), Some("50-200"));
        assert_eq!(sim.rotation, "random");
    }

    #[test]
    fn test_config_yaml_with_http_output() {
        let yaml = r#"
output:
  target: http
  url: https://logs.example.com/ingest
  batch_size: 500
  format: json
  headers:
    Authorization: "Bearer token123"
  retry_attempts: 5
  retry_delay_ms: 2000
count: 100
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.output.target, "http");
        assert_eq!(config.output.url.as_deref(), Some("https://logs.example.com/ingest"));
        assert_eq!(config.output.batch_size, 500);
        assert_eq!(config.output.format, "json");
        let headers = config.output.headers.unwrap();
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer token123");
        assert_eq!(config.output.retry_attempts, 5);
        assert_eq!(config.output.retry_delay_ms, 2000);
    }

    #[test]
    fn test_config_yaml_with_kafka_output() {
        let yaml = r#"
output:
  target: kafka
  kafka:
    brokers: "kafka-1:9092,kafka-2:9092"
    topic: "app-logs"
    key_var: "ipv4"
    acks: "all"
    timeout_ms: 10000
    batch_size: 50
count: 100
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.output.target, "kafka");
        let kafka = config.output.kafka.unwrap();
        assert_eq!(kafka.brokers, "kafka-1:9092,kafka-2:9092");
        assert_eq!(kafka.topic, "app-logs");
        assert_eq!(kafka.key_var.as_deref(), Some("ipv4"));
        assert_eq!(kafka.acks, "all");
        assert_eq!(kafka.timeout_ms, 10000);
        assert_eq!(kafka.batch_size, 50);
    }

    #[test]
    fn test_config_yaml_file_enhancements() {
        let yaml = r#"
output:
  target: file
  path: /tmp/test.log
  append: false
  rotate_bytes: 1048576
  buffer_size: 16384
count: 50
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.output.append);
        assert_eq!(config.output.rotate_bytes, Some(1048576));
        assert_eq!(config.output.buffer_size, 16384);
    }

    #[test]
    fn test_config_yaml_with_progress() {
        let yaml = r#"
count: 100000
progress: true
progress_interval: 5000
num_threads: 8
output:
  target: file
  path: /tmp/progress.log
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.progress, Some(true));
        assert_eq!(config.progress_interval, 5000);
        assert_eq!(config.num_threads, Some(8));
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
  - "{{ ipv4 }} - {{ status }}"
templates: /tmp/logs
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
        assert_eq!(config.logs, Some(vec!["{{ ipv4 }} - {{ status }}".to_string()]));
        assert_eq!(config.templates, Some("/tmp/logs".to_string()));
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
