use std::collections::HashMap;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use loggen::cli::{apply_cli_args, create_writer, load_base_config, load_attack_config_file, merge_cli_attacks, parse_attack_spec, validate_http_config, validate_kafka_config};
use loggen::{Config, Generator, OutputConfig, ProgressReporter};

#[derive(Parser)]
#[command(name = "loggen", version, about, long_about = None)]
#[command(after_help = "Run 'loggen <subcommand> --help' for subcommand-specific help.
Run 'loggen completions <shell>' to generate shell completion scripts.")]
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
    #[command(after_help = "EXAMPLES:
  loggen generate --count 100
  loggen generate --config examples/example.yaml
  loggen generate --templates ./templates/ --count 10000 --output output.log
  loggen generate --templates ./templates/ --count 100000 --output large.log --progress --threads 8
  loggen generate --var app_name=myapp --var host=web01 --templates ./templates/ --count 500
  loggen generate --attack \"brute=single:{{ ipv4 }} - POST /login {{ status }} :50\"
  loggen generate --attack \"scan=multi:probe port 22\" --attack \"scan=multi:probe port 80\" --count 50
  loggen generate --validate --config examples/example.yaml
  loggen generate --validate --config examples/template-example.yaml
  loggen completions bash > /etc/bash_completion.d/loggen

CONFIG REFERENCE:
  See docs/configuration-reference.md for all config fields
  See docs/template-guide.md for template syntax and variables
  See docs/attack-gallery.md for attack pattern types
  See docs/cli-cheatsheet.md for all CLI flags with examples")]
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

        /// Validate configuration and exit (no generation)
        #[arg(long)]
        validate: bool,

        /// Show progress (auto-enabled for large counts)
        #[arg(long)]
        progress: bool,

        /// Disable progress reporting
        #[arg(long)]
        no_progress: bool,

        /// Number of worker threads for parallel generation
        #[arg(long)]
        threads: Option<usize>,
    },

    /// Send logs to an HTTP endpoint
    #[command(after_help = "EXAMPLES:
  loggen http --config examples/http-output.yaml
  loggen http --url https://logs.example.com/ingest --count 1000")]
    Http {
        /// base url of the http endpoint
        #[arg(short, long)]
        url: String,

        /// Number of log entries to generate
        #[arg(short = 'n', long)]
        count: Option<u64>,
    },

    /// Send logs to a Kafka topic
    #[command(after_help = "EXAMPLES:
  loggen kafka --config examples/kafka-output.yaml")]
    Kafka {
        /// kafka config as a mapping of key value pairs
        #[arg(short, long)]
        kafkaconfig: String,

        /// Number of log entries to generate
        #[arg(short = 'n', long)]
        count: Option<u64>,
    },

    /// Generate shell completion scripts
    #[command(after_help = "Usage:
  loggen completions bash > /etc/bash_completion.d/loggen
  loggen completions zsh > /usr/local/share/zsh/site-functions/_loggen
  loggen completions fish > ~/.config/fish/completions/loggen.fish
  loggen completions powershell > _loggen.ps1
  loggen completions elvish > loggen.elv")]
    Completions {
        /// Shell type: bash, zsh, fish, powershell, elvish
        shell: String,
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

fn validate_config(config: &Config) -> Result<(), String> {
    // Template validation is done in Generator::new()
    let mut errors = Vec::new();

    if config.random_intensity < 0.0 || config.random_intensity > 1.0 {
        errors.push(format!("random_intensity must be between 0.0 and 1.0, got {}", config.random_intensity));
    }

    if let Some(ref attacks) = config.attacks {
        for (i, attack) in attacks.iter().enumerate() {
            if attack.weight < 0.0 || attack.weight > 1.0 {
                errors.push(format!("attack[{}] '{}': weight must be between 0.0 and 1.0", i, attack.name.as_deref().unwrap_or("<unnamed>")));
            }
            match attack.attack_type.as_str() {
                "single_event" => {
                    if attack.template.as_ref().is_none_or(|t| t.is_empty()) {
                        errors.push(format!("attack[{}] '{}': single_event must have a non-empty template", i, attack.name.as_deref().unwrap_or("<unnamed>")));
                    }
                }
                "multi_ordered" => {
                    if attack.sequence.as_ref().is_none_or(|s| s.is_empty()) {
                        errors.push(format!("attack[{}] '{}': multi_ordered must have a non-empty sequence", i, attack.name.as_deref().unwrap_or("<unnamed>")));
                    }
                }
                "threshold_field" => {
                    if attack.threshold.is_none() {
                        errors.push(format!("attack[{}] '{}': threshold_field must have a threshold block", i, attack.name.as_deref().unwrap_or("<unnamed>")));
                    }
                    if attack.template.as_ref().is_none_or(|t| t.is_empty()) {
                        errors.push(format!("attack[{}] '{}': threshold_field must have a non-empty template", i, attack.name.as_deref().unwrap_or("<unnamed>")));
                    }
                }
                other => {
                    errors.push(format!("attack[{}] '{}': unknown attack type '{}'", i, attack.name.as_deref().unwrap_or("<unnamed>"), other));
                }
            }
        }
    }

    match config.output.target.as_str() {
        "http" => {
            if let Err(e) = validate_http_config(&config.output) {
                errors.push(e);
            }
        }
        "kafka" => {
            if let Err(e) = validate_kafka_config(&config.output) {
                errors.push(e);
            }
        }
        _ => {}
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn run_validate(config: &Config) {
    // First validate config structure
    if let Err(e) = validate_config(config) {
        eprintln!("Config validation error:\n{}", e);
        std::process::exit(1);
    }

    // Try to create Generator (this validates templates)
    let gen_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Generator::new(config.clone())
    }));

    match gen_result {
        Ok(_gen) => {
            let num_templates = if config.has_templates() { "present" } else { "none (legacy mode)" };
            let num_attacks = config.attacks.as_ref().map(|a| a.len()).unwrap_or(0);
            eprintln!("Config valid: templates {}, {} attack(s), {} entr(y/ies)", num_templates, num_attacks, config.count);
        }
        Err(panic) => {
            let msg = if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "unknown error".to_string()
            };
            eprintln!("Config validation error: {}", msg);
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
    validate: bool,
    progress: bool,
    no_progress: bool,
    threads: Option<usize>,
) {
    let cli_vars = parse_var_args(var);

    let mut cli_attacks: Vec<loggen::AttackConfig> = Vec::new();
    for spec in &attack {
        if let Some((_name, config)) = parse_attack_spec(spec) {
            cli_attacks.push(config);
        } else {
            eprintln!("Warning: ignoring malformed --attack '{}' (expected name=type:template[:count])", spec);
        }
    }

    if let Some(ref path) = attack_config {
        let file_attacks = load_attack_config_file(path);
        cli_attacks.extend(file_attacks);
    }

    let cli_attacks = merge_cli_attacks(cli_attacks);

    let base = load_base_config(config_path);
    let mut config = apply_cli_args(base, output, count, level, message, cli_vars, templates, cli_attacks, attack_only);

    // Apply CLI progress flags
    if progress {
        config.progress = Some(true);
    }
    if no_progress {
        config.progress = Some(false);
    }

    // Configure threads
    if let Some(num) = threads {
        config.num_threads = Some(num);
        if num > 0 {
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(num)
                .build_global();
        }
    }

    // Handle --validate
    if validate {
        run_validate(&config);
        return;
    }

    let generator = Generator::new(config);
    let mut writer = create_writer(generator.config())
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to create output writer: {}", e);
            std::process::exit(1);
        });

    // Set up progress reporter
    let config_ref = generator.config();
    let total_count = config_ref.count;
    let progress_enabled = config_ref.progress.unwrap_or_else(|| {
        // Auto-enable if count >= 100,000 and not stdout
        total_count >= 100_000 && config_ref.output.target != "stdout"
    });
    let mut progress_reporter = ProgressReporter::new(progress_enabled, total_count, 1.0, config_ref.progress_interval);

    if let Err(e) = generator.generate_to_writer_with_progress(&mut *writer, &mut progress_reporter) {
        eprintln!("\nError: generation failed: {}", e);
        std::process::exit(1);
    }

    progress_reporter.done();
}

fn handle_http(url: String, count: Option<u64>) {
    let config = Config {
        output: OutputConfig {
            target: "http".to_string(),
            url: Some(url),
            ..OutputConfig::default()
        },
        count: count.unwrap_or(100),
        ..Config::default()
    };

    if let Err(e) = validate_http_config(&config.output) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    let generator = Generator::new(config);
    let mut writer = create_writer(generator.config())
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to create HTTP writer: {}", e);
            std::process::exit(1);
        });
    if let Err(e) = generator.generate_to_writer(&mut *writer) {
        eprintln!("Error: generation failed: {}", e);
        std::process::exit(1);
    }
}

fn handle_kafka(_kafkaconfig: String, count: Option<u64>) {
    let config = Config {
        output: OutputConfig {
            target: "kafka".to_string(),
            ..OutputConfig::default()
        },
        count: count.unwrap_or(100),
        ..Config::default()
    };

    let generator = Generator::new(config);
    let mut writer = create_writer(generator.config())
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to create Kafka writer: {}", e);
            std::process::exit(1);
        });
    if let Err(e) = generator.generate_to_writer(&mut *writer) {
        eprintln!("Error: generation failed: {}", e);
        std::process::exit(1);
    }
}

fn handle_completions(shell: String) {
    let shell = match shell.to_lowercase().as_str() {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        "powershell" => Shell::PowerShell,
        "elvish" => Shell::Elvish,
        other => {
            eprintln!("Unknown shell '{}'. Supported: bash, zsh, fish, powershell, elvish", other);
            std::process::exit(1);
        }
    };

    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut std::io::stdout());
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
            validate,
            progress,
            no_progress,
            threads,
        }) => handle_generate(cli.config.as_ref(), output, count, level, message, var, templates, attack, attack_config, attack_only, validate, progress, no_progress, threads),
        Some(Commands::Http { url, count }) => handle_http(url, count),
        Some(Commands::Kafka { kafkaconfig, count }) => handle_kafka(kafkaconfig, count),
        Some(Commands::Completions { shell }) => handle_completions(shell),
        None => {
            println!("No command provided. Use --help for usage information.");
        }
    }
}