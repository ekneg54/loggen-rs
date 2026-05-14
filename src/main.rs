use std::collections::HashMap;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use loggen::cli::{apply_cli_args, create_writer, load_base_config, load_attack_config_file, merge_cli_attacks, parse_attack_spec};
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

        /// Define an inline attack (repeatable, name=type:template[:count])
        #[arg(long = "attack", value_name = "ATTACK_SPEC", action = clap::ArgAction::Append)]
        attack: Vec<String>,

        /// Load attacks from YAML file
        #[arg(long = "attack-config", value_name = "FILE")]
        attack_config: Option<PathBuf>,

        /// Generate only attack entries (no normal logs)
        #[arg(long = "attack-only")]
        attack_only: bool,
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
    attack: Vec<String>,
    attack_config: Option<PathBuf>,
    attack_only: bool,
) {
    let cli_vars = parse_var_args(var);

    // Parse inline --attack specs
    let mut cli_attacks: Vec<loggen::AttackConfig> = Vec::new();
    for spec in &attack {
        if let Some((_name, config)) = parse_attack_spec(spec) {
            cli_attacks.push(config);
        } else {
            eprintln!("Warning: ignoring malformed --attack '{}' (expected name=type:template[:count])", spec);
        }
    }

    // Load --attack-config file
    if let Some(ref path) = attack_config {
        let file_attacks = load_attack_config_file(path);
        cli_attacks.extend(file_attacks);
    }

    // Merge multi_ordered attacks with same name
    let cli_attacks = merge_cli_attacks(cli_attacks);

    let base = load_base_config(config_path);
    let config = apply_cli_args(base, output, count, level, message, cli_vars, templates, cli_attacks, attack_only);
    let generator = Generator::new(config);
    let mut writer = create_writer(generator.config())
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to create output writer: {}", e);
            std::process::exit(1);
        });
    if let Err(e) = generator.generate_to_writer(&mut *writer) {
        eprintln!("Error: generation failed: {}", e);
        std::process::exit(1);
    }
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
            attack,
            attack_config,
            attack_only,
        }) => handle_generate(cli.config.as_ref(), output, count, level, message, var, templates, attack, attack_config, attack_only),
        Some(Commands::Http { url }) => handle_http(url),
        Some(Commands::Kafka { kafkaconfig }) => handle_kafka(kafkaconfig),
        None => {
            println!("No command provided. Use --help for usage information.");
        }
    }
}