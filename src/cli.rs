use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::{AttackConfig, OutputConfig};
use crate::output::{FileWriter, StdoutWriter};
use crate::{Config, LogEntry, LogWriter};

pub fn load_base_config(config_path: Option<&PathBuf>) -> Config {
    match config_path {
        Some(path) => Config::from_file(path).unwrap_or_else(|_| {
            eprintln!(
                "Warning: could not read config file '{}', using defaults",
                path.display()
            );
            Config::default()
        }),
        None => Config::default(),
    }
}

pub fn parse_attack_spec(s: &str) -> Option<(String, AttackConfig)> {
    let (name, rest) = s.split_once('=')?;
    let colon_pos = rest.find(':')?;
    let attack_type_str = &rest[..colon_pos];
    let remaining = &rest[colon_pos + 1..];

    // Map short type names
    let attack_type = match attack_type_str {
        "single" => "single_event",
        "multi" => "multi_ordered",
        "threshold" => "threshold_field",
        other => other,
    }
    .to_string();

    // Try to find count at the end.
    // Only treat :NUMBER as a count if preceded by a space, to avoid
    // misparsing colons in URLs, ports, or timestamps (e.g. evil.com:8080).
    let (template_str, count) = if let Some(last_colon) = remaining.rfind(':') {
        let potential_count = &remaining[last_colon + 1..];
        let preceded_by_space = last_colon > 0 && remaining.as_bytes()[last_colon - 1] == b' ';
        if preceded_by_space {
            if let Ok(c) = potential_count.parse::<u64>() {
                // Strip the trailing space before the colon
                let template = &remaining[..last_colon - 1];
                (template, Some(c))
            } else {
                (remaining, None)
            }
        } else {
            (remaining, None)
        }
    } else {
        (remaining, None)
    };

    let mut config = AttackConfig {
        name: Some(name.to_string()),
        attack_type,
        template: None,
        sequence: None,
        count,
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: None,
    };

    match config.attack_type.as_str() {
        "multi_ordered" => {
            config.sequence = Some(vec![template_str.to_string()]);
        }
        "threshold_field" => {
            config.template = Some(template_str.to_string());
        }
        _ => {
            config.template = Some(template_str.to_string());
        }
    }

    Some((name.to_string(), config))
}

pub fn merge_cli_attacks(attacks: Vec<AttackConfig>) -> Vec<AttackConfig> {
    // Group multi_ordered attacks by name and merge their sequences
    let mut seen: HashMap<String, AttackConfig> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for attack in attacks {
        let key = attack.name.clone().unwrap_or_default();
        if let Some(existing) = seen.remove(&key) {
            // Merge into existing
            let mut merged = existing;
            if let Some(seq) = attack.sequence {
                if let Some(ref mut existing_seq) = merged.sequence {
                    existing_seq.extend(seq);
                } else {
                    merged.sequence = Some(seq);
                }
            }
            if merged.count.is_none() && attack.count.is_some() {
                merged.count = attack.count;
            }
            seen.insert(key.clone(), merged);
        } else {
            order.push(key.clone());
            seen.insert(key, attack);
        }
    }

    order.into_iter().filter_map(|k| seen.remove(&k)).collect()
}

pub fn load_attack_config_file(path: &PathBuf) -> Vec<AttackConfig> {
    #[derive(serde::Deserialize)]
    struct AttacksFile {
        attacks: Option<Vec<AttackConfig>>,
    }
    match crate::config::read_yaml_file::<AttacksFile, &PathBuf>(path) {
        Ok(file) => file.attacks.unwrap_or_default(),
        Err(e) => {
            eprintln!("Warning: could not read attack config file '{}': {}", path.display(), e);
            Vec::new()
        }
    }
}

pub fn apply_cli_args(
    mut config: Config,
    output: Option<String>,
    count: Option<u64>,
    level: Option<String>,
    message: Option<String>,
    var: HashMap<String, String>,
    templates: Option<String>,
    attack_configs: Vec<AttackConfig>,
    attack_only: bool,
) -> Config {
    if let Some(c) = count {
        config.count = c;
    }
    if let Some(l) = level {
        config.log_level = l;
    }
    if let Some(m) = message {
        config.message = m;
    }
    config.output = match output {
        Some(path) => OutputConfig {
            target: "file".to_string(),
            path: Some(path),
        },
        None => config.output,
    };

    // Merge CLI --var into template_vars (CLI takes precedence)
    let mut merged = config.template_vars.clone().unwrap_or_default();
    for (k, v) in var {
        merged.insert(k, v);
    }
    if !merged.is_empty() {
        config.template_vars = Some(merged);
    }

    // CLI --templates overrides config file
    if templates.is_some() {
        config.templates = templates;
    }

    // Merge attacks from CLI args into config (CLI overrides file on name collisions)
    if !attack_configs.is_empty() || attack_only {
        let mut base_attacks = config.attacks.clone().unwrap_or_default();
        let mut seen_names: HashMap<String, usize> = HashMap::new();
        for (i, a) in base_attacks.iter().enumerate() {
            if let Some(ref name) = a.name {
                seen_names.insert(name.clone(), i);
            }
        }

        for cli_attack in attack_configs {
            let key = cli_attack.name.clone().unwrap_or_default();
            if key.is_empty() || !seen_names.contains_key(&key) {
                seen_names.insert(key, base_attacks.len());
                base_attacks.push(cli_attack);
            } else {
                let idx = seen_names[&key];
                base_attacks[idx] = cli_attack;
            }
        }

        if !base_attacks.is_empty() {
            config.attacks = Some(base_attacks);
        }
    }

    config.attack_only = attack_only;

    config
}

pub fn create_writer(config: &Config) -> Result<Box<dyn LogWriter>, Box<dyn std::error::Error>> {
    let template_mode = config.has_templates();
    if config.output.target == "file" {
        let path = config.output.path.as_deref().unwrap_or("output.log");
        let mut writer = FileWriter::new(path)?;
        writer.template_mode = template_mode;
        Ok(Box::new(writer))
    } else {
        Ok(Box::new(StdoutWriter { template_mode }))
    }
}

pub fn write_entries(writer: &mut Box<dyn LogWriter>, entries: &[LogEntry]) {
    for entry in entries {
        writer.write_entry(entry).unwrap();
    }
    writer.flush().unwrap();
}
