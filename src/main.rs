use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use loggen::config::OutputConfig;
use loggen::output::{FileWriter, StdoutWriter};
use loggen::{Config, Generator, LogWriter};

#[derive(Parser)]
#[command(name = "loggen", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to YAML config file
    #[arg(short, long, value_name = "FILE", global = true)]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate log entries
    Generate {
        /// Output file path (default: stdout)
        #[arg(short, long, value_name = "FILE")]
        output: Option<String>,

        /// Number of log entries to generate
        #[arg(short = 'n', long, default_value_t = 1)]
        count: u64,

        /// Log level
        #[arg(short, long, default_value = "INFO")]
        level: String,

        /// Log message
        #[arg(short, long, default_value = "Log entry generated")]
        message: String,
    },

    /// Send logs to an HTTP endpoint (not yet implemented)
    Http {
        /// base url of the http endpoint
        #[arg(short, long)]
        url: String,
    },

    /// Send logs to a Kafka topic (not yet implemented)
    Kafka {
        /// kafka config as a mapping of key value pairs
        #[arg(short, long)]
        kafkaconfig: String,
    },
}

fn try_show_completion_help() -> Option<clap::Command> {
    let args: Vec<String> = std::env::args().collect();
    if args.last().map(|s| s.as_str()) != Some("help") {
        return None;
    }
    let subcmd_name = args.iter().skip(1).find(|a| !a.starts_with('-'))?;
    let mut cmd = Cli::command();
    cmd.find_subcommand_mut(subcmd_name).cloned()
}

fn main() {
    if let Some(mut subcmd) = try_show_completion_help() {
        subcmd.print_help().unwrap();
        println!();
        return;
    }

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Generate {
            output,
            count,
            level,
            message,
        }) => {
            let config = match cli.config {
                Some(ref path) => Config::from_file(path).unwrap_or_else(|_| {
                    eprintln!("Warning: could not read config file '{}', using defaults", path.display());
                    Config::default()
                }),
                None => Config::default(),
            };

            let merged = Config {
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
            };

            let generator = Generator::new(merged);
            let entries = generator.generate();

            let output_cfg = generator.config();
            let mut writer: Box<dyn LogWriter> = if output_cfg.output.target == "file" {
                let path = output_cfg
                    .output
                    .path
                    .as_deref()
                    .unwrap_or("output.log");
                Box::new(FileWriter::new(path).unwrap())
            } else {
                Box::new(StdoutWriter)
            };

            for entry in &entries {
                writer.write_entry(entry).unwrap();
            }
            writer.flush().unwrap();
        }
        Some(Commands::Http { url: _ }) => {
            println!("Sending logs to http endpoint (not yet implemented)");
        }
        Some(Commands::Kafka { kafkaconfig: _ }) => {
            println!("Sending logs to kafka topic (not yet implemented)");
        }
        None => {
            println!("No command provided. Use --help for usage information.");
        }
    }
}
