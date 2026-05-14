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
pub struct ThresholdConfig {
    pub field: String,
    #[serde(default)]
    pub min: Option<u64>,
    #[serde(default)]
    pub max: Option<u64>,
    #[serde(default = "default_threshold_proportion")]
    pub proportion: f64,
}

fn default_threshold_proportion() -> f64 {
    0.5
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttackVarConfig {
    pub values: Vec<String>,
    #[serde(default = "default_attack_var_mode")]
    pub mode: String,
}

fn default_attack_var_mode() -> String {
    "random".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttackConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub attack_type: String,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub sequence: Option<Vec<String>>,
    #[serde(default)]
    pub count: Option<u64>,
    #[serde(default)]
    pub interleave: bool,
    #[serde(default = "default_attack_weight")]
    pub weight: f64,
    #[serde(default = "default_attack_repeat")]
    pub repeat: String,
    #[serde(default)]
    pub threshold: Option<ThresholdConfig>,
    #[serde(default)]
    pub vars: Option<HashMap<String, AttackVarConfig>>,
}

fn default_attack_weight() -> f64 {
    0.5
}

fn default_attack_repeat() -> String {
    "loop".to_string()
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

    // Phase 3: Attack Patterns
    #[serde(default)]
    pub attacks: Option<Vec<AttackConfig>>,
    #[serde(default)]
    pub attack_only: bool,
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
            attacks: None,
            attack_only: false,
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        read_yaml_file(path)
    }

    pub fn has_templates(&self) -> bool {
        self.logs.as_ref().map_or(false, |v| !v.is_empty()) || self.templates.is_some()
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
        assert!(config.attacks.is_none());
        assert!(!config.attack_only);
    }

    #[test]
    fn test_config_yaml_with_attacks() {
        let yaml = r#"
count: 50
attacks:
  - name: brute-force
    type: single_event
    template: '{{ ipv4 }} - POST /login {{ status }}'
    count: 10
    interleave: true
    weight: 0.3
    vars:
      status:
        values: ["401", "401", "401", "200"]
        mode: weighted
  - name: port-scan
    type: multi_ordered
    sequence:
      - 'probe {{ ipv4 }}:22'
      - 'probe {{ ipv4 }}:80'
    count: 20
    repeat: once
  - name: ddos
    type: threshold_field
    template: '{{ ipv4 }} - {{ status }}'
    threshold:
      field: status
      min: 500
      proportion: 0.7
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let attacks = config.attacks.unwrap();
        assert_eq!(attacks.len(), 3);

        assert_eq!(attacks[0].name.as_deref(), Some("brute-force"));
        assert_eq!(attacks[0].attack_type, "single_event");
        assert_eq!(attacks[0].count, Some(10));
        assert!(attacks[0].interleave);
        assert!((attacks[0].weight - 0.3).abs() < 1e-6);
        let vars = attacks[0].vars.as_ref().unwrap();
        assert_eq!(vars["status"].values, vec!["401", "401", "401", "200"]);
        assert_eq!(vars["status"].mode, "weighted");

        assert_eq!(attacks[1].attack_type, "multi_ordered");
        let seq = attacks[1].sequence.as_ref().unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(attacks[1].repeat, "once");

        assert_eq!(attacks[2].attack_type, "threshold_field");
        let th = attacks[2].threshold.as_ref().unwrap();
        assert_eq!(th.field, "status");
        assert_eq!(th.min, Some(500));
        assert!((th.proportion - 0.7).abs() < 1e-6);
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
