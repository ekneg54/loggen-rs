# CLI Cheat Sheet

## Basic Usage

```
loggen generate                      # Generate 1 entry to stdout (defaults)
loggen generate --count 100          # Generate 100 entries
loggen generate -c config.yaml       # Load config from YAML file
loggen generate --output output.log  # Write to file
```

## Output Options

| Flag | Description |
|------|-------------|
| `-o, --output FILE` | Output file path (sets target to "file") |
| `--progress` | Show progress reporting (auto-enabled for >= 100,000 entries) |
| `--no-progress` | Disable progress reporting |

## Template Options

| Flag | Description |
|------|-------------|
| `-m, --message TEXT` | Log message template |
| `-l, --level LEVEL` | Log level (default: INFO) |
| `-n, --count N` | Number of entries to generate |
| `--templates PATH` | Template file or directory with `.logtpl` files |
| `--var KEY=VALUE` | Custom template variables (repeatable) |

## Attack Options

| Flag | Description |
|------|-------------|
| `--attack SPEC` | Define inline attack: `name=type:template[:count]` (repeatable) |
| `--attack-config FILE` | Load attacks from YAML file |
| `--attack-only` | Generate only attack entries (no normal logs) |

### Attack Spec Format

```
name=single:template text :count
name=multi:template text :count
name=threshold:template text :count
```

For multi attacks, repeat `--attack` with the same name to build the sequence:
```
--attack scan=multi:"CONNECT port 22" --attack scan=multi:"CONNECT port 80"
```

## Validation

| Flag | Description |
|------|-------------|
| `--validate` | Load config, validate templates and attacks, then exit (no generation) |

## Performance

| Flag | Description |
|------|-------------|
| `--threads N` | Number of worker threads for parallel generation |

## Completions Subcommand

```
loggen completions bash        # Generate bash completions
loggen completions zsh         # Generate zsh completions
loggen completions fish        # Generate fish completions
loggen completions powershell  # Generate PowerShell completions
loggen completions elvish      # Generate elvish completions
```

## Common Examples

```bash
# Legacy mode (no templates)
loggen generate --count 100 --message "Health check" --level INFO

# Template mode with built-in random vars
loggen generate --templates ./templates/ --count 1000 --output logs.txt

# Template mode with custom variable
loggen generate --templates ./templates/ --var app_name=myapp --count 500

# Attack pattern from CLI
loggen generate --attack "brute=single:{{ ipv4 }} - POST /login {{ status }} :50"

# Attack from config file
loggen generate --config examples/attack-brute-force.yaml

# Validate config without generating
loggen generate --validate --config examples/template-example.yaml

# HTTP output
loggen generate --config examples/http-output.yaml

# Kafka output (requires --features kafka)
loggen generate --config examples/kafka-output.yaml

# Progress reporting with parallel generation
loggen generate --templates ./templates/ --count 1000000 --output large.log --progress --threads 8
```
