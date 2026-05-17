# Configuration Reference

The `-c, --config` flag is global and works with all subcommands (`generate`, `http`, `kafka`).
Values from the config file are overridden by environment variables, which are overridden by CLI flags.

## Top-level `Config` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `count` | `u64` | `1` | Number of log entries to generate |
| `log_level` | `String` | `"INFO"` | Log level string — available as `{{ level }}` in templates |
| `message` | `String` | `"Log entry generated"` | Log message — available as `{{ message }}` in templates |
| `output` | `OutputConfig` | (see below) | Output destination configuration |
| `logs` | `Option<Vec<String>>` | `None` | Inline Tera templates. If set (with `templates`), templates take priority over legacy `message`/`log_level` |
| `templates` | `Option<String>` | `None` | Path to a `.logtpl` file or directory of `.logtpl` files |
| `template_vars` | `Option<HashMap<String, String>>` | `None` | Static variable definitions available to all templates |
| `seed` | `Option<u64>` | `None` | RNG seed for reproducible random output |
| `random_vars` | `Option<HashMap<String, Vec<String>>>` | `None` | Custom random pools — a var matching a pool name picks a random element each entry |
| `random_intensity` | `f64` | `1.0` | Probability (0.0–1.0) of randomizing auto-vars per entry per variable. 0.0 = no randomization, 1.0 = always randomize |
| `template_rotation` | `String` | `"sequential"` | Template selection strategy: `"sequential"`, `"random"`, or `"round_robin"` |
| `num_threads` | `Option<usize>` | `None` | Rayon thread pool size. `None` = system default |
| `progress` | `Option<bool>` | `None` | Enable/disable progress reporting. `None` = auto-enable for count >= 100,000 |
| `progress_interval` | `u64` | `10000` | Entry count between progress updates (min 1000) |

## `OutputConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `target` | `String` | `"stdout"` | Output destination: `"stdout"`, `"file"`, `"http"`, or `"kafka"` |
| `path` | `Option<String>` | `None` | File path (required when `target: "file"`) |
| `buffer_size` | `u64` | `8192` | Output buffer in bytes before flush (0 = no buffering) |
| `progress` | `Option<bool>` | `None` | Per-output progress override |
| `progress_interval` | `u64` | `10000` | Per-output progress interval override |
| `url` | `Option<String>` | `None` | HTTP endpoint URL (required when `target: "http"`) |
| `batch_size` | `u64` | `100` | Max entries per POST request (HTTP) or per flush (Kafka) |
| `format` | `String` | `"ndjson"` | HTTP body format: `"ndjson"`, `"json"`, or `"raw"` |
| `headers` | `Option<HashMap<String, String>>` | `None` | Custom HTTP headers |
| `retry_attempts` | `u32` | `3` | Max retries on failed HTTP POST |
| `retry_delay_ms` | `u64` | `1000` | Delay between HTTP retries (ms) |
| `kafka` | `Option<KafkaOutputConfig>` | `None` | Kafka-specific settings (required when `target: "kafka"`) |
| `append` | `bool` | `true` | Append to existing file vs truncate |
| `rotate_bytes` | `Option<u64>` | `None` | Rotate file (rename to `.1`) after this many bytes |

## HTTP Output

When `output.target: "http"`, logs are sent as HTTP POST requests to the configured URL.
Supports batching, retries, custom headers, and multiple body formats.

```yaml
output:
  target: http
  url: https://logs.example.com/ingest
  batch_size: 500
  format: ndjson          # ndjson (default), json, or raw
  headers:
    Authorization: "Bearer token123"
  retry_attempts: 3
  retry_delay_ms: 2000
```

These fields can also be set via the `loggen http` subcommand with CLI flags or environment variables.

| Field | CLI Flag | Env Var | Default | Description |
|-------|----------|---------|---------|-------------|
| `url` | `--url` | — | required | HTTP endpoint URL |
| `batch_size` | `--batch-size` | `LOGGEN_HTTP_BATCH_SIZE` | `100` | Max entries per POST request |
| `format` | `--format` | `LOGGEN_HTTP_FORMAT` | `"ndjson"` | Body format: `"ndjson"`, `"json"`, or `"raw"` |
| `headers` | `--header KEY=VALUE` | `LOGGEN_HTTP_HEADERS` | `None` | Custom HTTP headers (repeatable) |
| `retry_attempts` | `--retry-attempts` | `LOGGEN_HTTP_RETRY_ATTEMPTS` | `3` | Max retries on failed POST |
| `retry_delay_ms` | `--retry-delay-ms` | `LOGGEN_HTTP_RETRY_DELAY_MS` | `1000` | Delay between retries (ms) |

Progress reporting is always enabled for HTTP output.

## `KafkaOutputConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `brokers` | `String` | `"localhost:9092"` | Comma-separated list of broker addresses |
| `topic` | `String` | required | Kafka topic name |
| `key_var` | `Option<String>` | `None` | Template variable name to use as message key |
| `acks` | `String` | `"1"` | Required acks: `"0"`, `"1"`, or `"all"` |
| `timeout_ms` | `u64` | `5000` | Message delivery timeout (ms) |
| `batch_size` | `u64` | `100` | Max messages to buffer before flush |

## `SimulationConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `delay` | `Option<String>` | `None` | Delay between entries: single ms value (`"500"`) or range (`"100-500"`) |
| `rotation` | `String` | `"none"` | Template selection when simulation is active: `"none"`, `"round_robin"`, or `"random"` |

When `simulation` is configured, generation runs infinitely (stop with Ctrl+C). The `delay` adds a random sleep between each log entry output. The `rotation` controls how templates are cycled (`"none"` = always use first template, `"round_robin"` = cycle in order, `"random"` = pick randomly).

Simulation works with all output targets (`stdout`, `file`, `http`, `kafka`). With `http`/`kafka`, the per-entry `flush()` is skipped so batching is preserved. Progress shows `~N entries` (no fixed goal) and auto-enables.
