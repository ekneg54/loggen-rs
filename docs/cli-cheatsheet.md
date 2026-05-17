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

## Simulation Options (all subcommands)

| Flag | Description |
|------|-------------|
| `--sim-delay MS` | Delay between entries: single ms (`500`) or range (`10-500`). Enables infinite streaming |
| `--sim-rotation MODE` | Template cycling mode: `none`, `round_robin`, `random` (default: `none`) |

Available on `generate`, `http`, and `kafka` subcommands. When active, generation runs endlessly until Ctrl+C. Progress shows `~N entries` (no goal) and auto-enables regardless of count.

## Validate Configuration

| Flag | Subcommand | Description |
|------|------------|-------------|
| `--validate` | `generate` | Load config, validate templates, then exit (no generation) |

## Performance

| Flag | Subcommand | Description |
|------|------------|-------------|
| `--threads N` | `generate` | Number of worker threads for parallel generation |

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

## HTTP Output Subcommand

Send logs to an HTTP endpoint with full configuration via CLI flags, environment variables, or a config file.

```
loggen http --config examples/http-output.yaml              # Load from config
loggen http --url https://logs.example.com/ingest --count 1000  # Minimal CLI
```

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `-c, --config FILE` | — | — | Path to YAML config file (global) |
| `-u, --url URL` | — | required | HTTP endpoint URL |
| `-n, --count N` | — | 100 | Number of entries |
| `--batch-size N` | `LOGGEN_HTTP_BATCH_SIZE` | 100 | Max entries per POST |
| `--format FMT` | `LOGGEN_HTTP_FORMAT` | ndjson | Body format: `ndjson`, `json`, `raw` |
| `--header KEY=VALUE` | `LOGGEN_HTTP_HEADERS` | — | Custom HTTP header (repeatable) |
| `--retry-attempts N` | `LOGGEN_HTTP_RETRY_ATTEMPTS` | 3 | Max retries on failed POST |
| `--retry-delay-ms N` | `LOGGEN_HTTP_RETRY_DELAY_MS` | 1000 | Delay between retries (ms) |
| `--sim-delay MS` | — | — | Delay between entries (enables infinite streaming) |
| `--sim-rotation MODE` | — | `none` | Template cycling: `none`, `round_robin`, `random` |

Precedence: CLI arg > env var > config file > default. Progress always shown.

## Kafka Output Subcommand

Send logs to a Kafka topic. Requires building with `--features kafka` and `librdkafka`.

```
loggen kafka --config examples/kafka-output.yaml               # Load from config
loggen kafka --topic app-logs --count 1000                      # Minimal CLI
```

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `-c, --config FILE` | — | — | Path to YAML config file (global) |
| `-n, --count N` | — | 100 | Number of entries |
| `--brokers HOSTS` | `LOGGEN_KAFKA_BROKERS` | `localhost:9092` | Bootstrap servers |
| `--topic TOPIC` | `LOGGEN_KAFKA_TOPIC` | required | Kafka topic name |
| `--key-var NAME` | `LOGGEN_KAFKA_KEY_VAR` | — | Template var for message key |
| `--acks N` | `LOGGEN_KAFKA_ACKS` | `1` | Producer acks: `0`, `1`, `all` |
| `--timeout-ms N` | `LOGGEN_KAFKA_TIMEOUT_MS` | 5000 | Message timeout (ms) |
| `--batch-size N` | `LOGGEN_KAFKA_BATCH_SIZE` | 100 | Max messages per flush |
| `--sim-delay MS` | — | — | Delay between entries (enables infinite streaming) |
| `--sim-rotation MODE` | — | `none` | Template cycling: `none`, `round_robin`, `random` |

Precedence: CLI arg > env var > config file > default. Progress always shown.

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

# HTTP output from config
loggen http --config examples/http-output.yaml

# HTTP output with inline CLI args
loggen http --url https://logs.example.com/ingest --count 5000 --batch-size 200 --format json

# HTTP infinite streaming with delay
loggen http --url https://logs.example.com/ingest --sim-delay 1000

# Kafka output from config (requires --features kafka)
loggen kafka --config examples/kafka-output.yaml

# Kafka output with inline CLI args
loggen kafka --topic app-logs --brokers kafka-1:9092 --count 5000

# Kafka infinite streaming with delay
loggen kafka --topic app-logs --sim-delay 500 --sim-rotation random

# Progress reporting with parallel generation
loggen generate --templates ./templates/ --count 1000000 --output large.log --progress --threads 8
```
