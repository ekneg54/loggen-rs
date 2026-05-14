use std::collections::HashMap;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use loggen::cli::{apply_cli_args, create_writer, load_base_config, write_entries};
use loggen::Generator;

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
        #[arg(short = 'n', long)]
        count: Option<u64>,

        /// Log level
        #[arg(short, long)]
        level: Option<String>,

        /// Log message
        #[arg(short, long)]
        message: Option<String>,

        /// Template variable (repeatable, KEY=VALUE)
        #[arg(long = "var", value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
        var: Vec<String>,

        /// Template file or directory containing .logtpl template files
        #[arg(long = "templates", value_name = "PATH")]
        templates: Option<String>,
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

fn parse_var_args(var: Vec<String>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in var {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        } else {
            eprintln!("Warning: ignoring malformed --var '{}' (expected KEY=VALUE)", pair);
        }
    }
    map
}

fn handle_generate(
    config_path: Option<&PathBuf>,
    output: Option<String>,
    count: Option<u64>,
    level: Option<String>,
    message: Option<String>,
    var: Vec<String>,
    templates: Option<String>,
) {
    let cli_vars = parse_var_args(var);
    let base = load_base_config(config_path);
    let config = apply_cli_args(base, output, count, level, message, cli_vars, templates);
    let generator = Generator::new(config);
    let entries = generator.generate();
    let mut writer = create_writer(generator.config())
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to create output writer: {}", e);
            std::process::exit(1);
        });
    write_entries(&mut writer, &entries);
}

fn handle_http(_url: String) {
    println!("Sending logs to http endpoint (not yet implemented)");
}

fn handle_kafka(_kafkaconfig: String) {
    println!("Sending logs to kafka topic (not yet implemented)");
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
            var,
            templates,
        }) => handle_generate(cli.config.as_ref(), output, count, level, message, var, templates),
        Some(Commands::Http { url }) => handle_http(url),
        Some(Commands::Kafka { kafkaconfig }) => handle_kafka(kafkaconfig),
        None => {
            println!("No command provided. Use --help for usage information.");
        }
    }
}
