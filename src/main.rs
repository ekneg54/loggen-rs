use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use loggen::cli::{apply_cli_args, create_writer, load_base_config, validate_http_config, validate_kafka_config};
use loggen::{Config, Generator, OutputConfig, ProgressReporter};
use loggen::generator::parse_delay_range;

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
   loggen generate --validate --config examples/example.yaml
   loggen generate --validate --config examples/template-example.yaml
   loggen generate --count 20 --sim-delay 500-2000
   loggen generate --config examples/simulation-basic.yaml
   loggen completions bash > /etc/bash_completion.d/loggen

CONFIG REFERENCE:
  See docs/configuration-reference.md for all config fields
  See docs/template-guide.md for template syntax and variables

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

        /// Simulation delay in ms (single value or MIN-MAX range, e.g. "100" or "10-500")
        #[arg(long, value_name = "MS")]
        sim_delay: Option<String>,

        /// Simulation rotation mode: none, round_robin, random
        #[arg(long, value_name = "MODE")]
        sim_rotation: Option<String>,
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

    // Phase 7: Simulation validation
    if let Some(ref sim) = config.simulation {
        if let Some(ref delay) = sim.delay {
            if let Err(e) = parse_delay_range(delay) {
                errors.push(e);
            }
        }
        match sim.rotation.as_str() {
            "none" | "round_robin" | "random" => {}
            _ => errors.push(format!("invalid simulation rotation '{}': must be none, round_robin, or random", sim.rotation)),
        }
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
            eprintln!("Config valid: templates {}, {} entr(y/ies)", num_templates, config.count);
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
fn setup_ctrlc(cancel: &Arc<AtomicBool>) {
    let flag = cancel.clone();
    let _ = ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
    });
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
    validate: bool,
    progress: bool,
    no_progress: bool,
    threads: Option<usize>,
    sim_delay: Option<String>,
    sim_rotation: Option<String>,
    cancel: Arc<AtomicBool>,
) {
    let cli_vars = parse_var_args(var);

    let base = load_base_config(config_path);
    let mut config = apply_cli_args(base, output, count, level, message, cli_vars, templates, sim_delay, sim_rotation);

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

    // Validate config before generation
    if let Err(e) = validate_config(&config) {
        eprintln!("Config validation error:\n{}", e);
        std::process::exit(1);
    }

    // Handle --validate
    if validate {
        run_validate(&config);
        return;
    }

    let generator = Generator::new_with_cancel(config, cancel);
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

fn handle_http(url: String, count: Option<u64>, cancel: Arc<AtomicBool>) {
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

    let generator = Generator::new_with_cancel(config, cancel);
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

fn handle_kafka(_kafkaconfig: String, count: Option<u64>, cancel: Arc<AtomicBool>) {
    let config = Config {
        output: OutputConfig {
            target: "kafka".to_string(),
            ..OutputConfig::default()
        },
        count: count.unwrap_or(100),
        ..Config::default()
    };

    let generator = Generator::new_with_cancel(config, cancel);
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

    let cancel = Arc::new(AtomicBool::new(false));
    setup_ctrlc(&cancel);

    match cli.command {
        Some(Commands::Generate {
            output,
            count,
            level,
            message,
            var,
            templates,
            validate,
            progress,
            no_progress,
            threads,
            sim_delay,
            sim_rotation,
        }) => handle_generate(cli.config.as_ref(), output, count, level, message, var, templates, validate, progress, no_progress, threads, sim_delay, sim_rotation, cancel),
        Some(Commands::Http { url, count }) => handle_http(url, count, cancel),
        Some(Commands::Kafka { kafkaconfig, count }) => handle_kafka(kafkaconfig, count, cancel),
        Some(Commands::Completions { shell }) => handle_completions(shell),
        None => {
            println!("No command provided. Use --help for usage information.");
        }
    }
}