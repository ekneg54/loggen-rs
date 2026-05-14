use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::OutputConfig;
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

pub fn apply_cli_args(
    mut config: Config,
    output: Option<String>,
    count: Option<u64>,
    level: Option<String>,
    message: Option<String>,
    var: HashMap<String, String>,
    logs_dir: Option<String>,
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

    // CLI --logs-dir overrides config file
    if logs_dir.is_some() {
        config.logs_dir = logs_dir;
    }

    config
}

pub fn create_writer(config: &Config) -> Box<dyn LogWriter> {
    if config.output.target == "file" {
        let path = config.output.path.as_deref().unwrap_or("output.log");
        Box::new(FileWriter::new(path).unwrap())
    } else {
        Box::new(StdoutWriter)
    }
}

pub fn write_entries(writer: &mut Box<dyn LogWriter>, entries: &[LogEntry]) {
    for entry in entries {
        writer.write_entry(entry).unwrap();
    }
    writer.flush().unwrap();
}
