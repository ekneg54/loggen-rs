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

## Simulation Options

| Flag | Description |
|------|-------------|
| `--sim-delay MS` | Delay between entries: single ms (`500`) or range (`10-500`). Enables infinite streaming |
| `--sim-rotation MODE` | Template cycling mode: `none`, `round_robin`, `random` (default: `none`) |

When `--sim-delay` or `--sim-rotation` is set, generation runs endlessly until Ctrl+C.

## Validation

| Flag | Description |
|------|-------------|
| `--validate` | Load config, validate templates, then exit (no generation) |

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

# Infinite streaming with 200ms delay between entries (Ctrl+C to stop)
loggen generate --templates ./templates/ --sim-delay 200

# Streaming with random delay 50-500ms and round-robin template rotation
loggen generate --templates ./templates/ --sim-delay 50-500 --sim-rotation round_robin

# Validate config without generating
loggen generate --validate --config examples/template-example.yaml

# Simulation from YAML config (infinite, 100ms delay, random templates)
loggen generate --config examples/simulation-with-templates.yaml

# HTTP output
loggen generate --config examples/http-output.yaml

# Kafka output (requires --features kafka)
loggen generate --config examples/kafka-output.yaml

# Progress reporting with parallel generation
loggen generate --templates ./templates/ --count 1000000 --output large.log --progress --threads 8
```
