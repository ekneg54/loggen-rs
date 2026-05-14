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

pub fn apply_cli_args(config: Config, output: Option<String>, count: u64, level: String, message: String) -> Config {
    Config {
        count,
        log_level: level,
        message,
        output: match output {
            Some(path) => OutputConfig {
                target: "file".to_string(),
                path: Some(path),
            },
            None => config.output,
        },
    }
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
