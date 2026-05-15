use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{LazyLock, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use regex::Regex;
use tera::{Context, Tera};

use crate::config::{AttackConfig, AttackVarConfig, Config, LogEntry, ThresholdConfig};
use crate::output::{LogWriter, ProgressReporter};

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

fn ts_to_rfc3339(ts: u64) -> String {
    DateTime::<Utc>::from_timestamp_secs(ts as i64)
        .expect("valid unix timestamp")
        .to_rfc3339()
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
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "logtpl"))
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
        let (templates, mut tera) = match load_templates_from_config(&config) {
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

        // Pre-register attack templates in Tera instance
        if let Some(ref attacks) = config.attacks {
            let template_vars = config.template_vars.clone().unwrap_or_default();
            let random_vars = config.random_vars.clone().unwrap_or_default();

            for (attack_idx, attack) in attacks.iter().enumerate() {
                let mut attack_templates: Vec<String> = Vec::new();
                match attack.attack_type.as_str() {
                    "multi_ordered" => {
                        if let Some(ref seq) = attack.sequence {
                            for (seq_idx, tpl) in seq.iter().enumerate() {
                                let name = format!("attack_{}_seq_{}", attack_idx, seq_idx);
                                if tera.add_raw_template(&name, tpl).is_err() {
                                    continue;
                                }
                                attack_templates.push(tpl.clone());
                            }
                        }
                    }
                    _ => {
                        if let Some(ref tpl) = attack.template {
                            let name = format!("attack_{}", attack_idx);
                            if tera.add_raw_template(&name, tpl).is_err() {
                                continue;
                            }
                            attack_templates.push(tpl.clone());
                        }
                    }
                }

                // Validate attack templates against known variables
                let attack_var_names: HashSet<String> = attack.vars.as_ref()
                    .map(|v| v.keys().cloned().collect())
                    .unwrap_or_default();
                let common_names: HashSet<String> = attack.common.as_ref()
                    .map(|c| c.iter().cloned().collect())
                    .unwrap_or_default();
                let mut combined_vars = template_vars.clone();
                for k in attack_var_names.union(&common_names) {
                    combined_vars.entry(k.clone()).or_insert_with(|| "attack_var".to_string());
                }
                let combined_random: HashMap<String, Vec<String>> = random_vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                if let Err(e) = validate_templates(&attack_templates, &combined_vars, &combined_random) {
                    panic!("Attack template validation error: {}", e);
                }
            }
        }

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
        if self.has_attacks() {
            return self.generate_with_attacks();
        }
        if self.templates.is_empty() {
            return self.generate_legacy(self.config.count);
        }
        self.generate_with_templates(self.config.count)
    }

    pub fn generate_with_count(&self, count: u64) -> Vec<LogEntry> {
        if self.has_attacks() {
            return self.generate_with_attacks();
        }
        if self.templates.is_empty() {
            return self.generate_legacy(count);
        }
        self.generate_with_templates(count)
    }

    pub fn generate_to_writer(&self, writer: &mut dyn LogWriter) -> Result<(), Box<dyn std::error::Error>> {
        let mut progress = ProgressReporter::new(false, self.config.count, 1.0, 10000);
        self.generate_to_writer_with_progress(writer, &mut progress)
    }

    pub fn generate_to_writer_with_progress(
        &self,
        writer: &mut dyn LogWriter,
        progress: &mut ProgressReporter,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.has_attacks() {
            self.write_attack_stream(writer, progress)?;
            writer.flush()?;
            return Ok(());
        }
        let count = self.config.count;
        if self.templates.is_empty() {
            self.write_legacy_stream(count, writer, progress)?;
        } else if self.config.random_intensity >= 1.0 {
            self.write_template_parallel_stream(count, writer, progress)?;
        } else {
            self.write_template_stream(count, writer, progress)?;
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

        ctx_values.insert("timestamp".to_string(), tera::Value::String(ts_to_rfc3339(ts)));
        ctx_values.insert("level".to_string(), tera::Value::String(self.config.log_level.clone()));
        ctx_values.insert("index".to_string(), tera::Value::Number(tera::Number::from(i + 1)));
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

    fn write_legacy_stream(&self, count: u64, writer: &mut dyn LogWriter, progress: &mut ProgressReporter) -> Result<(), Box<dyn std::error::Error>> {
        let ts = current_timestamp().to_string();
        for i in 0..count {
            writer.write_entry(&LogEntry {
                timestamp: ts.clone(),
                level: self.config.log_level.clone(),
                message: format!("{} #{}", self.config.message, i + 1),
            })?;
            progress.report(i + 1);
        }
        Ok(())
    }

    fn write_template_stream(&self, count: u64, writer: &mut dyn LogWriter, progress: &mut ProgressReporter) -> Result<(), Box<dyn std::error::Error>> {
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
            progress.report(i + 1);
        }
        Ok(())
    }

    fn write_template_parallel_stream(&self, count: u64, writer: &mut dyn LogWriter, progress: &mut ProgressReporter) -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();
        let chunk_size: u64 = 5000;

        let templates = self.templates.clone();
        let config = self.config.clone();
        let seed = self.seed;
        let rotation = self.rotation;
        let total_count = count;

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

            for chunk_start in (0..total_count).step_by(chunk_size as usize) {
                let chunk_end = std::cmp::min(chunk_start + chunk_size, total_count);
                let entries: Vec<LogEntry> = (chunk_start..chunk_end).into_par_iter().map(|i| {
                    let mut rng = StdRng::seed_from_u64(seed + i);
                    let template_index = match rotation {
                        TemplateRotation::Sequential | TemplateRotation::RoundRobin => (i as usize) % templates.len(),
                        TemplateRotation::Random => rng.gen_range(0..templates.len()),
                    };
                    let template = &templates[template_index];
                    let used_vars = extract_template_vars(template);

                    let mut ctx: HashMap<String, tera::Value> = HashMap::new();
                    ctx.insert("timestamp".into(), tera::Value::String(ts_to_rfc3339(ts)));
                    ctx.insert("level".into(), tera::Value::String(config.log_level.clone()));
                    ctx.insert("index".into(), tera::Value::Number(tera::Number::from(i + 1)));
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

        let mut emitted: u64 = 0;
        while let Ok(entries) = rx.recv() {
            for entry in &entries {
                writer.write_entry(entry)?;
                emitted += 1;
                progress.report(emitted);
            }
        }

        Ok(())
    }
}

// ---- Phase 3: Attack Pattern Generation ----

#[derive(Debug, Clone)]
pub struct AttackCursor {
    sequence_index: usize,
}

impl AttackCursor {
    pub fn new() -> Self {
        AttackCursor { sequence_index: 0 }
    }
}

impl Default for AttackCursor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AttackEngine<'a> {
    #[allow(dead_code)]
    attacks: &'a [AttackConfig],
    rng: StdRng,
    cursors: Vec<AttackCursor>,
    remaining: Vec<u64>,
    threshold_accepted: Vec<u64>,
    var_cycles: Vec<HashMap<String, usize>>,
    common_cache: Vec<Option<HashMap<String, String>>>,
}

impl<'a> AttackEngine<'a> {
    pub fn new(attacks: &'a [AttackConfig], seed: u64, fallback_count: u64) -> Self {
        let count = attacks.len();
        AttackEngine {
            attacks,
            rng: StdRng::seed_from_u64(seed),
            cursors: vec![AttackCursor::new(); count],
            remaining: attacks.iter().map(|a| {
                let mut c = a.count.unwrap_or(fallback_count);
                // For multi_ordered with repeat=once, cap at sequence length
                if a.attack_type == "multi_ordered" && a.repeat == "once" {
                    if let Some(ref seq) = a.sequence {
                        c = c.min(seq.len() as u64);
                    }
                }
                c
            }).collect(),
            threshold_accepted: vec![0; count],
            var_cycles: vec![HashMap::new(); count],
            common_cache: vec![None; count],
        }
    }

    pub fn is_exhausted(&self) -> bool {
        self.remaining.iter().all(|&r| r == 0)
    }

    pub fn attack_remaining(&self, idx: usize) -> u64 {
        self.remaining[idx]
    }
}

fn is_value_in_bucket(val_str: &str, threshold: &ThresholdConfig) -> bool {
    let val: u64 = match val_str.parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let above_min = threshold.min.is_none_or(|m| val >= m);
    let below_max = threshold.max.is_none_or(|m| val <= m);
    above_min && below_max
}

fn pick_attack_var_value(var_config: &AttackVarConfig, cycle_pos: &mut usize, rng: &mut StdRng) -> String {
    if var_config.values.is_empty() {
        return String::new();
    }
    match var_config.mode.as_str() {
        "cycle" => {
            let idx = *cycle_pos % var_config.values.len();
            *cycle_pos += 1;
            var_config.values[idx].clone()
        }
        "weighted" => {
            let n = var_config.values.len();
            let total_weight: usize = (1..=n).sum();
            let roll = rng.gen_range(0..total_weight);
            let mut cum = 0;
            for (i, v) in var_config.values.iter().enumerate() {
                cum += n - i;
                if roll < cum {
                    return v.clone();
                }
            }
            var_config.values.last().cloned().unwrap_or_default()
        }
        _ => {
            var_config.values.choose(rng).cloned().unwrap_or_default()
        }
    }
}

fn render_attack_entry(
    generator: &Generator,
    attack: &AttackConfig,
    entry_index: u64,
    attack_idx: usize,
    engine: &mut AttackEngine,
) -> LogEntry {
    let cursor = &mut engine.cursors[attack_idx];
    let ts = current_timestamp();
    let template_vars = generator.config.template_vars.clone().unwrap_or_default();
    let random_vars = generator.config.random_vars.clone().unwrap_or_default();
    let auto_random: HashSet<String> = AUTO_RANDOM_VARS.iter().map(|s| s.to_string()).collect();
    let all_random_names: HashSet<String> = auto_random.union(&random_vars.keys().cloned().collect()).cloned().collect();
    let attack_vars = attack.vars.clone().unwrap_or_default();
    let attack_var_names: HashSet<String> = attack_vars.keys().cloned().collect();

    // Save pre-advance sequence index for template name computation
    let pre_seq_index = cursor.sequence_index;
    let (template_str, named) = match attack.attack_type.as_str() {
        "multi_ordered" => {
            if let Some(ref seq) = attack.sequence {
                if seq.is_empty() {
                    ("<empty sequence>".to_string(), None)
                } else {
                    let idx = pre_seq_index % seq.len();
                    let t = seq[idx].clone();
                    // advance cursor
                    cursor.sequence_index += 1;
                    let seq_len = seq.len();
                    if cursor.sequence_index >= seq_len {
                        if attack.repeat == "once" {
                            // mark this attack as exhausted
                            if let Some(r) = engine.remaining.get_mut(attack_idx) {
                                *r = 0;
                            }
                        } else {
                            cursor.sequence_index = 0;
                        }
                    }
                    (t, None)
                }
            } else {
                ("<no sequence>".to_string(), None)
            }
        }
        "threshold_field" => {
            let tpl = attack.template.clone().unwrap_or_default();
            // need special threshold handling
            (tpl, Some(attack.threshold.as_ref()))
        }
        _ => {
            // single_event
            (attack.template.clone().unwrap_or_default(), None)
        }
    };

    let used_vars = extract_template_vars(&template_str);

    let mut ctx_values: HashMap<String, tera::Value> = HashMap::new();
    ctx_values.insert("timestamp".to_string(), tera::Value::String(ts_to_rfc3339(ts)));
    ctx_values.insert("level".to_string(), tera::Value::String(generator.config.log_level.clone()));
    ctx_values.insert("index".to_string(), tera::Value::Number(tera::Number::from(entry_index)));
    ctx_values.insert("message".to_string(), tera::Value::String(generator.config.message.clone()));

    // global template_vars
    for (k, v) in &template_vars {
        ctx_values.insert(k.clone(), tera::Value::String(v.clone()));
    }

    // random vars (with intensity)
    for var_name in &used_vars {
        if BUILTIN_VARS.contains(&var_name.as_str()) { continue; }
        if template_vars.contains_key(var_name) { continue; }
        if attack_var_names.contains(var_name) { continue; }
        if !all_random_names.contains(var_name) { continue; }

        let should_randomize = generator.config.random_intensity >= 1.0
            || (generator.config.random_intensity > 0.0 && engine.rng.gen_bool(generator.config.random_intensity));
        if should_randomize {
            let val = generate_random_value(var_name, &generator.config, &mut engine.rng);
            ctx_values.insert(var_name.clone(), tera::Value::String(val));
        }
    }

    // common fields: freeze their values on first entry, reuse cached values thereafter
    if let Some(ref common) = attack.common {
        if engine.common_cache[attack_idx].is_none() {
            let mut cached = HashMap::new();
            for name in common {
                let val = ctx_values.get(name)
                    .map(|v| match v {
                        tera::Value::String(s) => s.clone(),
                        tera::Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                cached.insert(name.clone(), val);
            }
            engine.common_cache[attack_idx] = Some(cached);
        }
        if let Some(ref cached) = engine.common_cache[attack_idx] {
            for (k, v) in cached {
                ctx_values.insert(k.clone(), tera::Value::String(v.clone()));
            }
        }
    }

    // attack vars (strongest override)
    for (k, vc) in &attack_vars {
        let cycle_pos = engine.var_cycles[attack_idx].entry(k.clone()).or_insert(0);
        let val = pick_attack_var_value(vc, cycle_pos, &mut engine.rng);
        ctx_values.insert(k.clone(), tera::Value::String(val.clone()));
    }

    // threshold_field rejection sampling
    if let Some(threshold) = named.flatten() {
        let total = entry_index + 1; // total entries generated for this attack so far
        let in_bucket = engine.threshold_accepted[attack_idx];
        let target_count = (threshold.proportion * total as f64).ceil() as u64;

        if in_bucket < target_count {
            // rejection sampling on the threshold field
            let threshold_var = &threshold.field;
            // Save cycle position before rejection attempts to prevent
            // cycle mode from advancing by >1 per entry
            let saved_cycle_pos = if attack_var_names.contains(threshold_var) {
                engine.var_cycles[attack_idx].get(threshold_var).copied()
            } else {
                None
            };

            const MAX_ATTEMPTS: usize = 100;
            for attempt in 0..MAX_ATTEMPTS {
                // Restore cycle position so each attempt starts from the same place
                if let (true, Some(vc)) = (attack_var_names.contains(threshold_var), attack_vars.get(threshold_var)) {
                    let cycle_pos = engine.var_cycles[attack_idx].entry(threshold_var.clone()).or_insert(0);
                    if let Some(saved) = saved_cycle_pos {
                        *cycle_pos = saved;
                    }
                    let val = pick_attack_var_value(vc, cycle_pos, &mut engine.rng);
                    ctx_values.insert(threshold_var.clone(), tera::Value::String(val));
                } else if all_random_names.contains(threshold_var)
                    && !BUILTIN_VARS.contains(&threshold_var.as_str()) && !template_vars.contains_key(threshold_var) {
                    let val = generate_random_value(threshold_var, &generator.config, &mut engine.rng);
                    ctx_values.insert(threshold_var.clone(), tera::Value::String(val));
                }

                // Check if the value is in the bucket
                let val_str = ctx_values.get(threshold_var)
                    .map(|v| match v {
                        tera::Value::String(s) => s.clone(),
                        tera::Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                if is_value_in_bucket(&val_str, threshold) {
                    engine.threshold_accepted[attack_idx] += 1;
                    break;
                }
                if attempt == MAX_ATTEMPTS - 1 {
                    // last attempt, accept whatever we have
                    let val_str = ctx_values.get(threshold_var)
                        .map(|v| match v {
                            tera::Value::String(s) => s.clone(),
                            tera::Value::Number(n) => n.to_string(),
                            _ => String::new(),
                        })
                        .unwrap_or_default();
                    if is_value_in_bucket(&val_str, threshold) {
                        engine.threshold_accepted[attack_idx] += 1;
                    }
                }
            }
        }
    }

    let context = Context::from_serialize(&ctx_values).expect("failed to create Tera context");

    // Determine template name for this attack entry
    let template_name = match attack.attack_type.as_str() {
        "multi_ordered" => {
            let seq_len = attack.sequence.as_ref().map_or(1, |s| s.len().max(1));
            let seq_idx = pre_seq_index % seq_len;
            format!("attack_{}_seq_{}", attack_idx, seq_idx)
        }
        _ => {
            format!("attack_{}", attack_idx)
        }
    };
    let rendered = match generator.tera.render(&template_name, &context) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Warning: attack template render error: {:?}", e);
            "<attack render error>".to_string()
        }
    };

    LogEntry {
        timestamp: ts.to_string(),
        level: generator.config.log_level.clone(),
        message: rendered,
    }
}

impl Generator {
    pub fn has_attacks(&self) -> bool {
        self.config.attacks.as_ref().is_some_and(|a| !a.is_empty())
    }

    fn generate_attack_only(&self) -> Vec<LogEntry> {
        let attacks = self.config.attacks.as_ref().expect("attacks must be Some");
        let count = self.config.count;
        let mut engine = AttackEngine::new(attacks, self.seed, count);
        let mut entries = Vec::new();

        if attacks.iter().any(|a| a.interleave) {
            self.generate_attack_interleaved(&mut engine, &mut entries);
        } else {
            for (attack_idx, attack) in attacks.iter().enumerate() {
                let remaining = engine.remaining[attack_idx];
                for i in 0..remaining {
                    let entry = render_attack_entry(self, attack, i + 1, attack_idx, &mut engine);
                    entries.push(entry);
                }
            }
        }
        entries
    }

    fn generate_attack_interleaved(&self, engine: &mut AttackEngine, entries: &mut Vec<LogEntry>) {
        let attacks = self.config.attacks.as_ref().expect("attacks must be Some");
        let has_normal = !self.config.attack_only;
        let normal_count = self.config.count;
        let mut normal_emitted: u64 = 0;
        let mut current: HashMap<String, String> = HashMap::new();
        let template_vars = self.config.template_vars.clone().unwrap_or_default();
        let random_vars = self.config.random_vars.clone().unwrap_or_default();
        let auto_random: HashSet<String> = AUTO_RANDOM_VARS.iter().map(|s| s.to_string()).collect();
        let all_random_names: HashSet<String> = auto_random.union(&random_vars.keys().cloned().collect()).cloned().collect();

        loop {
            let mut stream_weights: Vec<(f64, usize)> = Vec::new();

            if has_normal && normal_emitted < normal_count {
                stream_weights.push((1.0, usize::MAX));
            }

            let active_attacks: Vec<usize> = attacks.iter().enumerate()
                .filter(|(i, a)| a.interleave && engine.attack_remaining(*i) > 0)
                .map(|(i, _)| i)
                .collect();

            for &ai in &active_attacks {
                stream_weights.push((attacks[ai].weight, ai));
            }

            if stream_weights.is_empty() {
                break;
            }

            let total_weight: f64 = stream_weights.iter().map(|(w, _)| w).sum();
            let roll = engine.rng.gen::<f64>() * total_weight;
            let mut cum = 0.0;
            let mut chosen: Option<usize> = None;

            for (w, idx) in &stream_weights {
                cum += w;
                if roll < cum {
                    chosen = Some(*idx);
                    break;
                }
            }

            match chosen {
                Some(usize::MAX) => {
                    let entry = if self.templates.is_empty() {
                        let ts = current_timestamp().to_string();
                        LogEntry {
                            timestamp: ts.clone(),
                            level: self.config.log_level.clone(),
                            message: format!("{} #{}", self.config.message, normal_emitted + 1),
                        }
                    } else {
                        let ts = current_timestamp();
                        let mut rng_seeded = StdRng::seed_from_u64(self.seed + normal_emitted);
                        self.render_single_entry(normal_emitted, &template_vars, &all_random_names, &mut rng_seeded, &mut current, ts)
                    };
                    entries.push(entry);
                    normal_emitted += 1;
                }
                Some(attack_idx) => {
                    let attack = &attacks[attack_idx];
                    let entry = render_attack_entry(self, attack, entries.len() as u64 + 1, attack_idx, engine);
                    entries.push(entry);
                    engine.remaining[attack_idx] = engine.remaining[attack_idx].saturating_sub(1);
                }
                None => break,
            }
        }
    }

    fn generate_with_attacks(&self) -> Vec<LogEntry> {
        let attacks = self.config.attacks.as_ref().expect("attacks must be Some");
        let count = self.config.count;

        if self.config.attack_only {
            return self.generate_attack_only();
        }

        let any_interleave = attacks.iter().any(|a| a.interleave);

        if any_interleave {
            let mut engine = AttackEngine::new(attacks, self.seed, count);
            let mut entries = Vec::new();
            self.generate_attack_interleaved(&mut engine, &mut entries);
            entries
        } else {
            // No interleave: normal first, then attacks sequentially
            let mut entries = if self.templates.is_empty() {
                self.generate_legacy(count)
            } else {
                self.generate_with_templates(count)
            };

            let mut engine = AttackEngine::new(attacks, self.seed + count, count);
            for (attack_idx, attack) in attacks.iter().enumerate() {
                let remaining = engine.remaining[attack_idx];
                let base_idx = entries.len() as u64;
                for i in 0..remaining {
                    let entry = render_attack_entry(self, attack, base_idx + i + 1, attack_idx, &mut engine);
                    entries.push(entry);
                }
            }
            entries
        }
    }

    fn write_attack_stream(&self, writer: &mut dyn LogWriter, progress: &mut ProgressReporter) -> Result<(), Box<dyn std::error::Error>> {
        let attacks = self.config.attacks.as_ref().expect("attacks must be Some");
        let count = self.config.count;
        let mut total_emitted: u64 = 0;

        if self.config.attack_only {
            let mut engine = AttackEngine::new(attacks, self.seed, count);
            let any_interleave = attacks.iter().any(|a| a.interleave);

            if any_interleave {
                let _emitted = self.write_attack_interleaved(&mut engine, writer)?;
            } else {
                for (attack_idx, attack) in attacks.iter().enumerate() {
                    let remaining = engine.remaining[attack_idx];
                    for i in 0..remaining {
                        let entry = render_attack_entry(self, attack, i + 1, attack_idx, &mut engine);
                        writer.write_entry(&entry)?;
                        total_emitted += 1;
                        progress.report(total_emitted);
                    }
                }
            }
            return Ok(());
        }

        let any_interleave = attacks.iter().any(|a| a.interleave);

        if any_interleave {
            let mut engine = AttackEngine::new(attacks, self.seed, count);
            let _emitted = self.write_attack_interleaved(&mut engine, writer)?;
        } else {
            // Normal streaming first (use parallel when random_intensity >= 1.0)
            if self.templates.is_empty() {
                self.write_legacy_stream(count, writer, progress)?;
            } else if self.config.random_intensity >= 1.0 {
                self.write_template_parallel_stream(count, writer, progress)?;
            } else {
                self.write_template_stream(count, writer, progress)?;
            }
            total_emitted += count;

            // Then attack entries
            let mut engine = AttackEngine::new(attacks, self.seed + count, count);
            for (attack_idx, attack) in attacks.iter().enumerate() {
                let remaining = engine.remaining[attack_idx];
                for i in 0..remaining {
                    let entry = render_attack_entry(self, attack, i + 1, attack_idx, &mut engine);
                    writer.write_entry(&entry)?;
                    total_emitted += 1;
                    progress.report(total_emitted);
                }
            }
        }

        Ok(())
    }

    fn write_attack_interleaved(&self, engine: &mut AttackEngine, writer: &mut dyn LogWriter) -> Result<u64, Box<dyn std::error::Error>> {
        let attacks = self.config.attacks.as_ref().expect("attacks must be Some");
        let has_normal = !self.config.attack_only;
        let normal_count = self.config.count;
        let mut normal_emitted: u64 = 0;
        let mut total_emitted: u64 = 0;
        let mut current: HashMap<String, String> = HashMap::new();
        let template_vars = self.config.template_vars.clone().unwrap_or_default();
        let random_vars = self.config.random_vars.clone().unwrap_or_default();
        let auto_random: HashSet<String> = AUTO_RANDOM_VARS.iter().map(|s| s.to_string()).collect();
        let all_random_names: HashSet<String> = auto_random.union(&random_vars.keys().cloned().collect()).cloned().collect();

        loop {
            let mut stream_weights: Vec<(f64, usize)> = Vec::new();

            if has_normal && normal_emitted < normal_count {
                stream_weights.push((1.0, usize::MAX));
            }

            let active_attacks: Vec<usize> = (0..attacks.len())
                .filter(|i| attacks[*i].interleave && engine.attack_remaining(*i) > 0)
                .collect();

            for &ai in &active_attacks {
                stream_weights.push((attacks[ai].weight, ai));
            }

            if stream_weights.is_empty() {
                break;
            }

            let total_weight: f64 = stream_weights.iter().map(|(w, _)| w).sum();
            let roll = engine.rng.gen::<f64>() * total_weight;
            let mut cum = 0.0;
            let mut chosen: Option<usize> = None;

            for (w, idx) in &stream_weights {
                cum += w;
                if roll < cum {
                    chosen = Some(*idx);
                    break;
                }
            }

            match chosen {
                Some(usize::MAX) => {
                    let entry = if self.templates.is_empty() {
                        let ts = current_timestamp().to_string();
                        LogEntry {
                            timestamp: ts.clone(),
                            level: self.config.log_level.clone(),
                            message: format!("{} #{}", self.config.message, normal_emitted + 1),
                        }
                    } else {
                        let ts = current_timestamp();
                        let mut rng_seeded = StdRng::seed_from_u64(self.seed + normal_emitted);
                        self.render_single_entry(normal_emitted, &template_vars, &all_random_names, &mut rng_seeded, &mut current, ts)
                    };
                    writer.write_entry(&entry)?;
                    normal_emitted += 1;
                    total_emitted += 1;
                }
                Some(attack_idx) => {
                    let attack = &attacks[attack_idx];
                    let entry = render_attack_entry(self, attack, total_emitted + 1, attack_idx, engine);
                    writer.write_entry(&entry)?;
                    engine.remaining[attack_idx] = engine.remaining[attack_idx].saturating_sub(1);
                    total_emitted += 1;
                }
                None => break,
            }
        }

        Ok(total_emitted)
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
