use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use regex::Regex;
use tera::{Context, Tera};

use crate::config::{Config, LogEntry};

const BUILTIN_VARS: &[&str] = &["timestamp", "level", "index", "message"];
const AUTO_RANDOM_VARS: &[&str] = &["ip", "user_agent", "email", "url", "port", "status"];

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_ip(rng: &mut StdRng) -> String {
    format!("{}.{}.{}.{}", rng.gen_range(1..255), rng.gen_range(0..256), rng.gen_range(0..256), rng.gen_range(1..255))
}

fn random_user_agent(rng: &mut StdRng) -> String {
    let uas = [
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/119.0.0.0 Safari/537.36",
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_1 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148",
        "curl/8.4.0",
        "Python-urllib/3.11",
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0",
    ];
    uas.choose(rng).unwrap().to_string()
}

fn random_email(rng: &mut StdRng) -> String {
    let names = ["alice", "bob", "charlie", "dave", "eve", "frank", "grace", "admin", "user", "test"];
    let domains = ["example.com", "test.org", "mail.net", "corp.io", "web.dev"];
    format!("{}@{}", names.choose(rng).unwrap(), domains.choose(rng).unwrap())
}

fn random_url(rng: &mut StdRng) -> String {
    let paths = [
        "/index.html", "/api/v1/users", "/api/v1/data", "/login", "/search",
        "/products", "/about", "/contact", "/images/logo.png", "/style.css",
        "/api/health", "/dashboard", "/settings", "/logout", "/register",
    ];
    paths.choose(rng).unwrap().to_string()
}

fn random_port(rng: &mut StdRng) -> u16 {
    let common = [80u16, 443, 8080, 8443, 3000, 5000, 9000, 5432, 6379, 22];
    if rng.gen_bool(0.7) {
        *common.choose(rng).unwrap()
    } else {
        rng.gen_range(1024..65535)
    }
}

fn random_status(rng: &mut StdRng) -> u16 {
    let roll: f64 = rng.gen();
    if roll < 0.65 {
        // 2xx
        *[200u16, 201, 204, 206].choose(rng).unwrap()
    } else if roll < 0.80 {
        // 3xx
        *[301u16, 302, 304, 307].choose(rng).unwrap()
    } else if roll < 0.93 {
        // 4xx
        *[400u16, 401, 403, 404, 405, 418, 429].choose(rng).unwrap()
    } else {
        // 5xx
        *[500u16, 502, 503, 504].choose(rng).unwrap()
    }
}

fn extract_template_vars(template: &str) -> BTreeSet<String> {
    let mut vars = BTreeSet::new();
    let re_var = Regex::new(r"\{\{\s*(\w+)").unwrap();
    for cap in re_var.captures_iter(template) {
        if let Some(var) = cap.get(1) {
            vars.insert(var.as_str().to_string());
        }
    }
    let re_if = Regex::new(r"\{\%\s*if\s+(\w+)").unwrap();
    for cap in re_if.captures_iter(template) {
        if let Some(var) = cap.get(1) {
            vars.insert(var.as_str().to_string());
        }
    }
    let re_for = Regex::new(r"\{\%\s*for\s+\w+\s+in\s+(\w+)").unwrap();
    for cap in re_for.captures_iter(template) {
        if let Some(var) = cap.get(1) {
            vars.insert(var.as_str().to_string());
        }
    }
    vars
}

fn load_templates_from_config(config: &Config) -> Result<Vec<String>, String> {
    if let Some(ref logs) = config.logs {
        if !logs.is_empty() {
            return Ok(logs.clone());
        }
    }
    if let Some(ref dir) = config.logs_dir {
        let dir_path = Path::new(dir);
        if !dir_path.is_dir() {
            return Err(format!("logs_dir '{}' is not a directory or does not exist", dir));
        }
        let mut templates = Vec::new();
        let mut entries: Vec<_> = fs::read_dir(dir_path)
            .map_err(|e| format!("failed to read logs_dir '{}': {}", dir, e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "logtpl"))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let content = fs::read_to_string(entry.path())
                .map_err(|e| format!("failed to read '{}': {}", entry.path().display(), e))?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    templates.push(line.to_string());
                }
            }
        }
        if templates.is_empty() {
            return Err(format!("no templates found in logs_dir '{}'", dir));
        }
        return Ok(templates);
    }
    Err("no templates configured".to_string())
}

fn validate_templates(
    templates: &[String],
    template_vars: &HashMap<String, String>,
    random_vars: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let defined_vars: HashSet<String> = template_vars.keys().cloned().collect();
    let random_var_names: HashSet<String> = random_vars.keys().cloned().collect();
    let auto_random: HashSet<String> = AUTO_RANDOM_VARS.iter().map(|s| s.to_string()).collect();
    let builtin: HashSet<String> = BUILTIN_VARS.iter().map(|s| s.to_string()).collect();

    for (i, tpl) in templates.iter().enumerate() {
        let used = extract_template_vars(tpl);
        for var in &used {
            if builtin.contains(var) || defined_vars.contains(var) || auto_random.contains(var) || random_var_names.contains(var) {
                continue;
            }
            return Err(format!(
                "unknown variable '{}' in template {} ('{}')",
                var, i, tpl
            ));
        }
    }
    Ok(())
}

fn generate_random_value(
    var_name: &str,
    config: &Config,
    rng: &mut StdRng,
) -> String {
    if let Some(ref rv) = config.random_vars {
        if let Some(pool) = rv.get(var_name) {
            if !pool.is_empty() {
                return pool.choose(rng).unwrap().clone();
            }
        }
    }
    match var_name {
        "ip" => random_ip(rng),
        "user_agent" => random_user_agent(rng),
        "email" => random_email(rng),
        "url" => random_url(rng),
        "port" => random_port(rng).to_string(),
        "status" => random_status(rng).to_string(),
        _ => String::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TemplateRotation {
    Sequential,
    Random,
    RoundRobin,
}

impl TemplateRotation {
    fn from_str(s: &str) -> Self {
        match s {
            "random" => TemplateRotation::Random,
            "round_robin" => TemplateRotation::RoundRobin,
            _ => TemplateRotation::Sequential,
        }
    }
}

pub struct Generator {
    config: Config,
    templates: Vec<String>,
    tera: Tera,
    seed: u64,
    rotation: TemplateRotation,
}

impl Generator {
    pub fn new(config: Config) -> Self {
        let seed = config.seed.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        });
        let _rng = StdRng::seed_from_u64(seed);

        let (templates, tera) = match load_templates_from_config(&config) {
            Ok(tpls) => {
                let template_vars = config.template_vars.clone().unwrap_or_default();
                let random_vars = config.random_vars.clone().unwrap_or_default();
                if let Err(e) = validate_templates(&tpls, &template_vars, &random_vars) {
                    panic!("Template validation error: {}", e);
                }
                let mut tera = Tera::default();
                for (i, tpl) in tpls.iter().enumerate() {
                    tera.add_raw_template(&format!("tpl_{}", i), tpl)
                        .expect("failed to add template to Tera");
                }
                (tpls, tera)
            }
            Err(_) => {
                (Vec::new(), Tera::default())
            }
        };

        Generator {
            rotation: TemplateRotation::from_str(&config.template_rotation),
            config,
            templates,
            tera,
            seed,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn generate(&self) -> Vec<LogEntry> {
        if self.templates.is_empty() {
            // Legacy mode (Phase 1 fallback)
            return self.generate_legacy(self.config.count);
        }
        self.generate_with_templates(self.config.count)
    }

    pub fn generate_with_count(&self, count: u64) -> Vec<LogEntry> {
        if self.templates.is_empty() {
            return self.generate_legacy(count);
        }
        self.generate_with_templates(count)
    }

    fn generate_legacy(&self, count: u64) -> Vec<LogEntry> {
        let ts = current_timestamp().to_string();
        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            entries.push(LogEntry {
                timestamp: ts.clone(),
                level: self.config.log_level.clone(),
                message: format!("{} #{}", self.config.message, i + 1),
            });
        }
        entries
    }

    fn generate_with_templates(&self, count: u64) -> Vec<LogEntry> {
        let mut entries = Vec::with_capacity(count as usize);
        let template_vars = self.config.template_vars.clone().unwrap_or_default();
        let random_vars = self.config.random_vars.clone().unwrap_or_default();
        let auto_random: HashSet<String> = AUTO_RANDOM_VARS.iter().map(|s| s.to_string()).collect();
        let all_random_names: HashSet<String> = auto_random.union(&random_vars.keys().cloned().collect()).cloned().collect();

        let ts = current_timestamp();
        let mut rng = StdRng::seed_from_u64(self.seed);

        // Initialize current random values from template_vars or random generation
        let mut current: HashMap<String, String> = HashMap::new();

        for i in 0..count {
            let template_index = match self.rotation {
                TemplateRotation::Sequential | TemplateRotation::RoundRobin => {
                    (i as usize) % self.templates.len()
                }
                TemplateRotation::Random => {
                    rng.gen_range(0..self.templates.len())
                }
            };

            let template = &self.templates[template_index];

            // Gather template variable names for this template
            let used_vars = extract_template_vars(template);

            // Build context
            let mut ctx_values: HashMap<String, tera::Value> = HashMap::new();

            // Built-in vars
            ctx_values.insert("timestamp".to_string(), tera::Value::Number(tera::Number::from(ts)));
            ctx_values.insert("level".to_string(), tera::Value::String(self.config.log_level.clone()));
            ctx_values.insert("index".to_string(), tera::Value::Number(tera::Number::from((i + 1) as u64)));
            ctx_values.insert("message".to_string(), tera::Value::String(self.config.message.clone()));

            // Static template_vars
            for (k, v) in &template_vars {
                ctx_values.insert(k.clone(), tera::Value::String(v.clone()));
            }

            // Initialize / randomize values
            for var_name in &used_vars {
                if BUILTIN_VARS.contains(&var_name.as_str()) {
                    continue;
                }
                if template_vars.contains_key(var_name) {
                    continue;
                }
                if !all_random_names.contains(var_name) {
                    continue;
                }

                let should_randomize = self.config.random_intensity >= 1.0
                    || (self.config.random_intensity > 0.0 && rng.gen_bool(self.config.random_intensity));

                if should_randomize || !current.contains_key(var_name) {
                    let val = generate_random_value(var_name, &self.config, &mut rng);
                    current.insert(var_name.clone(), val);
                }

                if let Some(val) = current.get(var_name) {
                    ctx_values.insert(var_name.clone(), tera::Value::String(val.clone()));
                }
            }

            let context = Context::from_serialize(&ctx_values).expect("failed to create Tera context");

            let rendered = match self.tera.render(&format!("tpl_{}", template_index), &context) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Warning: template render error: {:?}", e);
                    format!("<render error>")
                }
            };

            entries.push(LogEntry {
                timestamp: ts.to_string(),
                level: self.config.log_level.clone(),
                message: rendered,
            });
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OutputConfig;

    fn test_config() -> Config {
        Config {
            output: OutputConfig::default(),
            count: 1,
            log_level: "INFO".to_string(),
            message: "test".to_string(),
            logs: None,
            logs_dir: None,
            template_vars: None,
            seed: None,
            random_vars: None,
            random_intensity: 1.0,
            template_rotation: "sequential".to_string(),
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
            logs: Some(vec!["{{ ip }} - {{ status }}".to_string()]),
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
    fn test_extract_template_vars_simple() {
        let vars = extract_template_vars("{{ ip }} - {{ status }}");
        assert!(vars.contains("ip"));
        assert!(vars.contains("status"));
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_extract_template_vars_with_filters() {
        let vars = extract_template_vars("{{ timestamp | date(format=\"%Y\") }}");
        assert!(vars.contains("timestamp"));
    }

    #[test]
    fn test_extract_template_vars_if_for() {
        let vars = extract_template_vars("{% if flag %}{% for x in list %}{{ x }}{% endfor %}{% endif %}");
        assert!(vars.contains("flag"));
        assert!(vars.contains("list"));
        assert!(vars.contains("x"));
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

}
