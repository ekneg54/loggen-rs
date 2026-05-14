use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{LazyLock, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use regex::Regex;
use tera::{Context, Tera};

use crate::config::{Config, LogEntry};
use crate::output::LogWriter;

const BUILTIN_VARS: &[&str] = &["timestamp", "level", "index", "message"];
const AUTO_RANDOM_VARS: &[&str] = &["ip", "ipv4", "ipv6", "user_agent", "email", "url", "port", "status", "user"];

static RE_TEMPLATE_VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{\s*(\w+)").unwrap());
static RE_IF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\%\s*if\s+(\w+)").unwrap());
static RE_FOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\%\s*for\s+\w+\s+in\s+(\w+)").unwrap());

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_ipv4(rng: &mut StdRng) -> String {
    format!("{}.{}.{}.{}", rng.gen_range(1..255), rng.gen_range(0..256), rng.gen_range(0..256), rng.gen_range(1..255))
}

fn random_ipv6(rng: &mut StdRng) -> String {
    format!("{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
        rng.gen_range(0x0..0x10000),
        rng.gen_range(0x0..0x10000),
        rng.gen_range(0x0..0x10000),
        rng.gen_range(0x0..0x10000),
        rng.gen_range(0x0..0x10000),
        rng.gen_range(0x0..0x10000),
        rng.gen_range(0x0..0x10000),
        rng.gen_range(0x0..0x10000),
    )
}

fn random_user(rng: &mut StdRng) -> String {
    let users = ["alice", "bob", "charlie", "dave", "eve", "frank", "grace", "henry", "admin", "jenny", "karl", "liam", "mia", "nina", "oscar"];
    users.choose(rng).unwrap().to_string()
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
        *[200u16, 201, 204, 206].choose(rng).unwrap()
    } else if roll < 0.80 {
        *[301u16, 302, 304, 307].choose(rng).unwrap()
    } else if roll < 0.93 {
        *[400u16, 401, 403, 404, 405, 418, 429].choose(rng).unwrap()
    } else {
        *[500u16, 502, 503, 504].choose(rng).unwrap()
    }
}

fn extract_template_vars(template: &str) -> BTreeSet<String> {
    let mut vars = BTreeSet::new();
    for cap in RE_TEMPLATE_VAR.captures_iter(template) {
        if let Some(var) = cap.get(1) {
            vars.insert(var.as_str().to_string());
        }
    }
    for cap in RE_IF.captures_iter(template) {
        if let Some(var) = cap.get(1) {
            vars.insert(var.as_str().to_string());
        }
    }
    for cap in RE_FOR.captures_iter(template) {
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
    if let Some(ref path_str) = config.templates {
        let path = Path::new(path_str);
        if path.is_file() {
            let content = fs::read_to_string(path)
                .map_err(|e| format!("failed to read template file '{}': {}", path_str, e))?;
            let templates: Vec<String> = content.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if templates.is_empty() {
                return Err(format!("no templates found in '{}'", path_str));
            }
            return Ok(templates);
        } else if path.is_dir() {
            let mut templates = Vec::new();
            let mut entries: Vec<_> = fs::read_dir(path)
                .map_err(|e| format!("failed to read directory '{}': {}", path_str, e))?
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
                return Err(format!("no templates found in directory '{}'", path_str));
            }
            return Ok(templates);
        } else {
            return Err(format!("'{}' is not a file or directory", path_str));
        }
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
        "ip" | "ipv4" => random_ipv4(rng),
        "ipv6" => random_ipv6(rng),
        "user_agent" => random_user_agent(rng),
        "email" => random_email(rng),
        "url" => random_url(rng),
        "port" => random_port(rng).to_string(),
        "status" => random_status(rng).to_string(),
        "user" => random_user(rng),
        _ => format!("<missing-generator:{}>", var_name),
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
            Err(e) => {
                eprintln!("Warning: template loading failed (falling back to legacy mode): {}", e);
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

    pub fn generate_to_writer(&self, writer: &mut dyn LogWriter) -> Result<(), Box<dyn std::error::Error>> {
        let count = self.config.count;
        if self.templates.is_empty() {
            self.write_legacy_stream(count, writer)?;
        } else if self.config.random_intensity >= 1.0 {
            self.write_template_parallel_stream(count, writer)?;
        } else {
            self.write_template_stream(count, writer)?;
        }
        writer.flush()?;
        Ok(())
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
        let template_vars = self.config.template_vars.clone().unwrap_or_default();
        let random_vars = self.config.random_vars.clone().unwrap_or_default();
        let auto_random: HashSet<String> = AUTO_RANDOM_VARS.iter().map(|s| s.to_string()).collect();
        let all_random_names: HashSet<String> = auto_random.union(&random_vars.keys().cloned().collect()).cloned().collect();
        let ts = current_timestamp();
        let mut rng = StdRng::seed_from_u64(self.seed);
        let mut current: HashMap<String, String> = HashMap::new();

        (0..count)
            .map(|i| self.render_single_entry(i, &template_vars, &all_random_names, &mut rng, &mut current, ts))
            .collect()
    }

    fn render_single_entry(
        &self,
        i: u64,
        template_vars: &HashMap<String, String>,
        all_random_names: &HashSet<String>,
        rng: &mut StdRng,
        current: &mut HashMap<String, String>,
        ts: u64,
    ) -> LogEntry {
        let template_index = match self.rotation {
            TemplateRotation::Sequential | TemplateRotation::RoundRobin => {
                (i as usize) % self.templates.len()
            }
            TemplateRotation::Random => {
                rng.gen_range(0..self.templates.len())
            }
        };

        let template = &self.templates[template_index];
        let used_vars = extract_template_vars(template);

        let mut ctx_values: HashMap<String, tera::Value> = HashMap::new();

        ctx_values.insert("timestamp".to_string(), tera::Value::Number(tera::Number::from(ts)));
        ctx_values.insert("level".to_string(), tera::Value::String(self.config.log_level.clone()));
        ctx_values.insert("index".to_string(), tera::Value::Number(tera::Number::from((i + 1) as u64)));
        ctx_values.insert("message".to_string(), tera::Value::String(self.config.message.clone()));

        for (k, v) in template_vars {
            ctx_values.insert(k.clone(), tera::Value::String(v.clone()));
        }

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
                let val = generate_random_value(var_name, &self.config, rng);
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
                "<render error>".to_string()
            }
        };

        LogEntry {
            timestamp: ts.to_string(),
            level: self.config.log_level.clone(),
            message: rendered,
        }
    }

    fn write_legacy_stream(&self, count: u64, writer: &mut dyn LogWriter) -> Result<(), Box<dyn std::error::Error>> {
        let ts = current_timestamp().to_string();
        for i in 0..count {
            writer.write_entry(&LogEntry {
                timestamp: ts.clone(),
                level: self.config.log_level.clone(),
                message: format!("{} #{}", self.config.message, i + 1),
            })?;
        }
        Ok(())
    }

    fn write_template_stream(&self, count: u64, writer: &mut dyn LogWriter) -> Result<(), Box<dyn std::error::Error>> {
        let template_vars = self.config.template_vars.clone().unwrap_or_default();
        let random_vars = self.config.random_vars.clone().unwrap_or_default();
        let auto_random: HashSet<String> = AUTO_RANDOM_VARS.iter().map(|s| s.to_string()).collect();
        let all_random_names: HashSet<String> = auto_random.union(&random_vars.keys().cloned().collect()).cloned().collect();
        let ts = current_timestamp();
        let mut rng = StdRng::seed_from_u64(self.seed);
        let mut current: HashMap<String, String> = HashMap::new();

        for i in 0..count {
            let entry = self.render_single_entry(i, &template_vars, &all_random_names, &mut rng, &mut current, ts);
            writer.write_entry(&entry)?;
        }
        Ok(())
    }

    fn write_template_parallel_stream(&self, count: u64, writer: &mut dyn LogWriter) -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();
        let chunk_size: u64 = 5000;

        let templates = self.templates.clone();
        let config = self.config.clone();
        let seed = self.seed;
        let rotation = self.rotation;

        std::thread::spawn(move || {
            let ts = current_timestamp();
            let template_vars = config.template_vars.clone().unwrap_or_default();
            let random_vars = config.random_vars.clone().unwrap_or_default();
            let auto_random: HashSet<String> = AUTO_RANDOM_VARS.iter().map(|s| s.to_string()).collect();
            let all_random_names: HashSet<String> = auto_random.union(&random_vars.keys().cloned().collect()).cloned().collect();

            let mut tera = Tera::default();
            for (i, tpl) in templates.iter().enumerate() {
                if tera.add_raw_template(&format!("tpl_{}", i), tpl).is_err() {
                    return;
                }
            }

            for chunk_start in (0..count).step_by(chunk_size as usize) {
                let chunk_end = std::cmp::min(chunk_start + chunk_size, count);
                let entries: Vec<LogEntry> = (chunk_start..chunk_end).into_par_iter().map(|i| {
                    let mut rng = StdRng::seed_from_u64(seed + i);
                    let template_index = match rotation {
                        TemplateRotation::Sequential | TemplateRotation::RoundRobin => (i as usize) % templates.len(),
                        TemplateRotation::Random => rng.gen_range(0..templates.len()),
                    };
                    let template = &templates[template_index];
                    let used_vars = extract_template_vars(template);

                    let mut ctx: HashMap<String, tera::Value> = HashMap::new();
                    ctx.insert("timestamp".into(), tera::Value::Number(tera::Number::from(ts)));
                    ctx.insert("level".into(), tera::Value::String(config.log_level.clone()));
                    ctx.insert("index".into(), tera::Value::Number(tera::Number::from((i + 1) as u64)));
                    ctx.insert("message".into(), tera::Value::String(config.message.clone()));

                    for (k, v) in &template_vars {
                        ctx.insert(k.clone(), tera::Value::String(v.clone()));
                    }

                    for var_name in &used_vars {
                        if BUILTIN_VARS.contains(&var_name.as_str()) || template_vars.contains_key(var_name) { continue; }
                        if !all_random_names.contains(var_name) { continue; }
                        let val = generate_random_value(var_name, &config, &mut rng);
                        ctx.insert(var_name.clone(), tera::Value::String(val));
                    }

                    let context = Context::from_serialize(&ctx).expect("context");
                    let rendered = tera.render(&format!("tpl_{}", template_index), &context).unwrap_or_else(|_| "<render error>".to_string());

                    LogEntry {
                        timestamp: ts.to_string(),
                        level: config.log_level.clone(),
                        message: rendered,
                    }
                }).collect();

                if tx.send(entries).is_err() { return; }
            }
        });

        while let Ok(entries) = rx.recv() {
            for entry in &entries {
                writer.write_entry(&entry)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
