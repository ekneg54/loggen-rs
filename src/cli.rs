use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::{OutputConfig, SimulationConfig};
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

#[allow(clippy::too_many_arguments)]
pub fn apply_cli_args(
    mut config: Config,
    output: Option<String>,
    count: Option<u64>,
    level: Option<String>,
    message: Option<String>,
    var: HashMap<String, String>,
    templates: Option<String>,
    sim_delay: Option<String>,
    sim_rotation: Option<String>,
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

    if sim_delay.is_some() || sim_rotation.is_some() {
        let mut sim = config.simulation.unwrap_or(SimulationConfig {
            delay: None,
            rotation: "none".to_string(),
        });
        if let Some(d) = sim_delay {
            sim.delay = Some(d);
        }
        if let Some(r) = sim_rotation {
            sim.rotation = r;
        }
        config.simulation = Some(sim);
    }

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