# Configuration Reference

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
| `attacks` | `Option<Vec<AttackConfig>>` | `None` | Attack pattern configurations |
| `attack_only` | `bool` | `false` | If true, generate only attack entries (no normal logs) |
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

## `KafkaOutputConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `brokers` | `String` | `"localhost:9092"` | Comma-separated list of broker addresses |
| `topic` | `String` | required | Kafka topic name |
| `key_var` | `Option<String>` | `None` | Template variable name to use as message key |
| `acks` | `String` | `"1"` | Required acks: `"0"`, `"1"`, or `"all"` |
| `timeout_ms` | `u64` | `5000` | Message delivery timeout (ms) |
| `batch_size` | `u64` | `100` | Max messages to buffer before flush |

## `AttackConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | `Option<String>` | `None` | Optional label for logging / debugging |
| `type` | `String` | required | One of: `"single_event"`, `"multi_ordered"`, `"threshold_field"` |
| `template` | `Option<String>` | `None` | Inline Tera template (`single_event` / `threshold_field`) |
| `sequence` | `Option<Vec<String>>` | `None` | Ordered list of Tera templates (`multi_ordered`) |
| `count` | `Option<u64>` | `None` | Per-attack entry count; falls back to top-level `count` |
| `interleave` | `bool` | `false` | If true, mix attack entries with normal entries during generation |
| `weight` | `f64` | `0.5` | Relative probability of picking this attack when interleaving (0.0–1.0) |
| `repeat` | `String` | `"loop"` | For `multi_ordered`: `"once"` or `"loop"` |
| `threshold` | `Option<ThresholdConfig>` | `None` | For `threshold_field`: controls proportion of entries in a value bucket |
| `vars` | `Option<HashMap<String, AttackVarConfig>>` | `None` | Per-attack variable definitions (override global vars) |
| `common` | `Option<Vec<String>>` | `None` | Variable names to freeze after first entry (persistent across attack run) |

## `ThresholdConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `field` | `String` | required | Template variable name to threshold on |
| `min` | `Option<u64>` | `None` | Inclusive lower bound for the threshold bucket |
| `max` | `Option<u64>` | `None` | Inclusive upper bound for the threshold bucket |
| `proportion` | `f64` | `0.5` | Target proportion of entries in the threshold bucket (0.0–1.0) |

At least one of `min`/`max` must be set:
- `min` only: bucket = value >= min
- `max` only: bucket = value <= max
- Both: bucket = min <= value <= max

## `AttackVarConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `values` | `Vec<String>` | required | Pool of values to draw from |
| `mode` | `String` | `"random"` | Selection mode: `"random"` (uniform), `"cycle"` (sequential wrap-around), `"weighted"` (first values higher probability) |
