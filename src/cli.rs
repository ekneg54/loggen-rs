use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::{AttackConfig, OutputConfig};
use crate::output::{BufferedLogWriter, FileWriter, HttpWriter, StdoutWriter};
use crate::{Config, LogWriter};

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

    let attack_type = match attack_type_str {
        "single" => "single_event",
        "multi" => "multi_ordered",
        "threshold" => "threshold_field",
        other => other,
    }
    .to_string();

    let (template_str, count) = if let Some(last_colon) = remaining.rfind(':') {
        let potential_count = &remaining[last_colon + 1..];
        let preceded_by_space = last_colon > 0 && remaining.as_bytes()[last_colon - 1] == b' ';
        if preceded_by_space {
            if let Ok(c) = potential_count.parse::<u64>() {
                let template = remaining[..last_colon - 1].trim_end();
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
        common: None,
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
    let mut seen: HashMap<String, AttackConfig> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for attack in attacks {
        let key = attack.name.clone().unwrap_or_default();
        if let Some(existing) = seen.remove(&key) {
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
            ..OutputConfig::default()
        },
        None => config.output,
    };

    let mut merged = config.template_vars.clone().unwrap_or_default();
    for (k, v) in var {
        merged.insert(k, v);
    }
    if !merged.is_empty() {
        config.template_vars = Some(merged);
    }

    if templates.is_some() {
        config.templates = templates;
    }

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

pub fn validate_http_config(output: &OutputConfig) -> Result<(), String> {
    if output.url.is_none() {
        return Err("HTTP output requires 'url' to be set".to_string());
    }
    if !["ndjson", "json", "raw"].contains(&output.format.as_str()) {
        return Err(format!("Invalid HTTP format '{}': must be ndjson, json, or raw", output.format));
    }
    Ok(())
}

pub fn validate_kafka_config(output: &OutputConfig) -> Result<(), String> {
    let kafka = output.kafka.as_ref().ok_or("Kafka output requires 'kafka' config block")?;
    if kafka.topic.is_empty() {
        return Err("Kafka output requires 'topic' to be set".to_string());
    }
    if !["0", "1", "all"].contains(&kafka.acks.as_str()) {
        return Err(format!("Invalid Kafka acks '{}': must be 0, 1, or all", kafka.acks));
    }
    Ok(())
}

pub fn create_writer(config: &Config) -> Result<Box<dyn LogWriter>, Box<dyn std::error::Error>> {
    let template_mode = config.has_templates();
    match config.output.target.as_str() {
        "http" => {
            validate_http_config(&config.output)
                .map_err(|e| format!("Config validation error: {}", e))?;
            let writer = HttpWriter::new(
                config.output.url.as_deref().unwrap_or(""),
                config.output.batch_size,
                &config.output.format,
                config.output.headers.as_ref(),
                config.output.retry_attempts,
                config.output.retry_delay_ms,
            )?;
            Ok(Box::new(writer))
        }
        "kafka" => {
            validate_kafka_config(&config.output)
                .map_err(|e| format!("Config validation error: {}", e))?;
            let kafka = config.output.kafka.as_ref().unwrap();
            let writer = crate::output::KafkaWriter::new(
                &kafka.brokers,
                &kafka.topic,
                kafka.key_var.as_deref(),
                &kafka.acks,
                kafka.timeout_ms,
                kafka.batch_size,
            )?;
            Ok(Box::new(writer))
        }
        "file" => {
            let path = config.output.path.as_deref().unwrap_or("output.log");
            let writer = FileWriter::new(path, !config.output.append, config.output.rotate_bytes)?;
            let mut writer = BufferedLogWriter::new(writer, config.output.buffer_size);
            writer.inner.template_mode = template_mode;
            Ok(Box::new(writer))
        }
        _ => {
            // stdout
            let writer = StdoutWriter::new();
            let mut writer = BufferedLogWriter::new(writer, config.output.buffer_size);
            writer.inner.template_mode = template_mode;
            Ok(Box::new(writer))
        }
    }
}

pub fn write_entries(writer: &mut Box<dyn LogWriter>, entries: &[crate::LogEntry]) {
    for entry in entries {
        writer.write_entry(entry).unwrap();
    }
    writer.flush().unwrap();
}