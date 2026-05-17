# Phased Implementation Plan for loggen-rs

## Phase 1: Foundation & Core Functionality (Weeks 1-2)

### 1.1 Project Setup
- Initialize Rust project with proper structure
- Set up Cargo.toml with dependencies (clap for CLI, serde for YAML, etc.)
- Configure build system and CI/CD pipeline
- Implement basic code documentation standards

### 1.2 Core Architecture
- Define core data structures for log entries
- Implement basic log generation engine
- Create YAML configuration parser
- Build CLI interface with basic options
- Implement stdout and file output streams
- Build an example config to demonstrate the core functionality and CLI interface

### 1.3 Basic Testing
- Unit tests for core components
- Basic integration tests
- Documentation generation setup

## Phase 2: Template System & Randomization (Weeks 3-4)

### 2.1 Template Engine Implementation

**Config changes:**
- Add optional `logs: Vec<String>` field to `Config` — each string is an inline template. If `logs` or `templates` are set, `message`/`log_level` are ignored (templates take over). If neither is set, fall back to Phase 1 behavior (backwards compatible).
- Add optional `templates: String` field — path to a directory of `.logtpl` template files. The files are red line by line and every line is a log entry with its jinja template variables. Each file contains one or a set of log entry with template variables.
- Add optional `template_vars: HashMap<String, String>` field — static variable definitions in YAML (e.g. `template_vars: { app_name: "myapp", host: "web01" }`).
- Add optional `seed: u64` for reproducible random generation.

**Template syntax:**
- Use **Tera** crate (Jinja2-inspired, supports `{{ var }}`, filters, `{% if %}`, `{% for %}`). Replaces `handlebars` from the deps list.
- Support: `{{ var }}` substitution, `{{ var | filter(args) }}`, `{% if %}`, `{% for %}`.
- Strict validation: any `{{ var }}` used in a template must be defined in one of: (a) `template_vars` in config, (b) CLI `--var` args, (c) built-in variables. Unknown variables cause a startup error.

**Built-in auto-available variables:**
- `{{ timestamp }}` — current Unix timestamp (format via filter: `{{ timestamp | date(format="%Y-%m-%d") }}`)
- `{{ level }}` — from `log_level` config / CLI `--level` (default `"INFO"`)
- `{{ index }}` — 1-based counter within a generation run
- `{{ message }}` — from `message` config / CLI `--message` (backwards compat)

**CLI additions:**
- Add `--var key=value` (repeatable) for arbitrary template variables.
- `--message` still works and maps to `{{ message }}`.
- Add `--templates` option to reflect the config changes in cli 

**Pipeline changes (`generator.rs`):**
- `Generator::generate()` loads templates from `config.logs` or files in `config.templates`, or falls back to legacy single-template (`message`/`log_level`) behavior.
- Creates a Tera instance, registers all templates, validates all referenced variables against the merged variable set.
- For each of `count` entries: pick template per rotation strategy, render with current variables, produce an output string.
- `LogEntry.message` holds the fully rendered template string; `write_entry` writes it directly.

### 2.2 Randomization Features

**Built-in random variable generators:**
Certain variable names trigger automatic random generation if not explicitly set by the user:
- `{{ ipv4 }}` → random IPv4
- `{{ ipv6 }}` → random IPv6
- `{{ user_agent }}` → random UA string from a built-in list
- `{{ email }}` → random email
- `{{ url }}` → random URL path
- `{{ port }}` → random port number
- `{{ status }}` → random HTTP status (weighted: 200 most common, then 4xx, 5xx)
- `{{ user }}` -> random user names

User-defined random pools via config: `random_vars: { codes: [200, 201, 404] }` — a var matching a pool name picks a random element each entry.

**Randomization intensity:**
- Config field `random_intensity: f64` (0.0–1.0, default 1.0):
  - 1.0 = all applicable variables get random values every entry
  - 0.5 = ~50% chance per-entry per-variable that it randomizes (else keeps template default / last value)
  - 0.0 = no randomization

**Template rotation:**
- Config field `template_rotation: "sequential" | "random" | "round_robin"` (default `"sequential"`):
  - `"sequential"`: render templates in order, repeat from start
  - `"random"`: pick a random template per entry
  - `"round_robin"`: cycle through templates in order, one per entry

### 2.3 Default Templates
- Create `templates/` directory with `.logtpl` files: Apache combined, Nginx combined, Syslog (RFC 3164).
- Each uses built-in variables to demonstrate usage.
- Add example configs referencing them via `templates`.

### 2.4 Concurrent design
- the app should be able to produce high load through concurrency via tokio or rayon
- the app should be memory efficient by not render all in memory and then give to output. instead it should be a stream process that first read all templates, and then produces the output with applied templates

### Dependency additions
- `tera` (replaces `handlebars`)
- `rand`

## Phase 3: Attack Pattern Generation (Weeks 5-6)

### 3.1 Config Schema for Attacks

**New Config field:**
- Add `attacks: Option<Vec<AttackConfig>>` to `Config` struct.
- When `attacks` is `Some` and non-empty, attack generation is active.
- Attack entries can be interleaved with normal template entries or generated standalone.

**`AttackConfig` struct (serde Deserialize):**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | `Option<String>` | `None` | Optional label for logging / debugging |
| `type` | `String` | required | One of: `"single_event"`, `"multi_ordered"`, `"threshold_field"` |
| `template` | `Option<String>` | `None` | Inline Tera template (`single_event` / `threshold_field`) |
| `sequence` | `Option<Vec<String>>` | `None` | Ordered list of Tera templates (`multi_ordered`) |
| `count` | `Option<u64>` | `None` | Per-attack entry count; falls back to top-level `count` if `None` |
| `interleave` | `bool` | `false` | If `true`, mix attack entries with normal entries during generation |
| `weight` | `f64` | `0.5` | Relative probability of picking this attack when interleaving (0.0–1.0). Normalized against other attack weights + normal stream. |
| `repeat` | `String` | `"loop"` | For `multi_ordered`:`"once"` (stop after sequence consumed) or `"loop"` (wrap around). |
| `threshold` | `Option<ThresholdConfig>` | `None` | For `threshold_field`: controls what proportion of entries fall in a value bucket |
| `vars` | `Option<HashMap<String, AttackVarConfig>>` | `None` | Per-attack variable definitions (override global `template_vars` and random defaults) |

**`ThresholdConfig` struct:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `field` | `String` | required | Template variable name to threshold on (e.g. `"status"`) |
| `min` | `Option<u64>` | `None` | Inclusive lower bound for the threshold bucket |
| `max` | `Option<u64>` | `None` | Inclusive upper bound for the threshold bucket |
| `proportion` | `f64` | `0.5` | Target proportion of entries in the threshold bucket (0.0–1.0) |

At least one of `min` / `max` must be set. Behavior:
- `min` only: bucket = value >= min
- `max` only: bucket = value <= max
- Both: bucket = min <= value <= max

**`AttackVarConfig` struct:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `values` | `Vec<String>` | required | Pool of values to draw from |
| `mode` | `String` | `"random"` | `"random"` (uniform random), `"cycle"` (sequential, restart from beginning), `"weighted"` (first value has highest probability, weights decay) |

**Example YAML illustrating all three attack types:**

```yaml
count: 500
log_level: INFO
output:
  target: file
  path: attack_output.log
templates: ./templates/
attacks:
  - name: brute-force
    type: single_event
    template: '{{ ipv4 }} - - [{{ timestamp | date(format="%d/%b/%Y:%H:%M:%S %z") }}] "POST /login HTTP/1.1" {{ status }} {{ port }} "-" "{{ user_agent }}"'
    count: 50
    interleave: true
    weight: 0.3
    vars:
      status:
        values: ["401", "401", "401", "200"]
        mode: weighted
      ipv4:
        values: ["192.168.1.100"]
        mode: cycle

  - name: port-scan
    type: multi_ordered
    sequence:
      - '{{ ipv4 }} - - [{{ timestamp }}] "CONNECT 10.0.0.1:22 HTTP/1.1" 200 0 "-" "nmap/7.80"'
      - '{{ ipv4 }} - - [{{ timestamp }}] "CONNECT 10.0.0.1:80 HTTP/1.1" 200 0 "-" "nmap/7.80"'
      - '{{ ipv4 }} - - [{{ timestamp }}] "CONNECT 10.0.0.1:443 HTTP/1.1" 200 0 "-" "nmap/7.80"'
      - '{{ ipv4 }} - - [{{ timestamp }}] "CONNECT 10.0.0.1:8080 HTTP/1.1" 200 0 "-" "nmap/7.80"'
    count: 100
    repeat: loop
    interleave: true
    weight: 0.2

  - name: ddos
    type: threshold_field
    count: 10000
    interleave: false
    template: '{{ ipv4 }} - - [{{ timestamp | date(format="%d/%b/%Y:%H:%M:%S %z") }}] "GET /api/health HTTP/1.1" {{ status }} {{ port }} "-" "{{ user_agent }}"'
    threshold:
      field: status
      min: 500
      proportion: 0.7
```

### 3.2 Generator Pipeline Changes

**New types:**

```
AttackCursor { sequence_index: usize }
AttackEngine {
    attacks: Vec<AttackConfig>,
    rng: StdRng,
    cursors: Vec<AttackCursor>,       // per-attack sequence position (multi_ordered)
    remaining: Vec<u64>,              // per-attack remaining count
}
```

- `AttackCursor` tracks the current index within a `multi_ordered` sequence.
- Each attack gets an independent cursor on construction, initialized to 0.

**Generator additions:**

| Method | Purpose |
|--------|---------|
| `has_attacks() -> bool` | True if `config.attacks` is `Some` and non-empty |
| `generate_attack_entry(&self, attack: &AttackConfig, index: u64, cursor: &mut AttackCursor, rng: &mut StdRng) -> LogEntry` | Render a single attack entry |
| `generate_with_attacks(count: u64) -> Vec<LogEntry>` | Top-level generation: normal entries + attack entries, potentially interleaved |
| `generate_attack_to_writer(&self, writer: &mut dyn LogWriter) -> Result<()>` | Streaming version of attack-aware generation |

**Per-type rendering logic:**

- **`single_event`**: Render `attack.template` via Tera. Merge is: (1) global `template_vars`, (2) built-in vars, (3) random vars per `random_intensity`, (4) `attack.vars` on top (strongest override). Each entry is independent. Respects `random_intensity` for non-attack-vars random fields. The `common` field describes the fields that do not change during attack. the `common` field is a list of fields, that are populated only once by the template engine and do not change during the attack.

- **`multi_ordered`**: Maintain a per-attack `AttackCursor`. Pick template from `sequence[cursor.sequence_index]`, advance cursor by 1. When cursor reaches `sequence.len()`: if `repeat: "once"`, mark attack exhausted (remaining = 0); if `"loop"`, reset cursor to 0. Merge variables in same priority as `single_event`. The cursor tracks per-attack state, so interleaved multi_ordered attacks maintain correct ordering across entry boundaries. The `common` field describes the fields that do not change during attack.

- **`threshold_field`**: Render template via Tera (same merge logic). Extract the rendered value of `threshold.field` from the template context — note: the template variable value is known at render time (it's in `ctx_values` before Tera rendering). Use rejection sampling: regenerate the random variable for `threshold.field` up to 10 times until the drawn value falls in the desired bucket. After the required `proportion` of entries are in-bucket, allow remaining entries to fall anywhere (i.e. stop rejecting). If retries exhausted (10 attempts), emit the last rendered value anyway. This ensures the output proportion converges toward the target without risking infinite loops. The `common` field describes the fields that do not change during attack.

**Interleaving logic (when `interleave: true` on any attack):**

1. Build a list of active streams: the normal stream (if normal templates exist or legacy mode), plus one stream per attack.
2. Weight each stream: normal has weight 1.0, each attack has its configured `weight`.
3. Each iteration: normalize weights to sum to 1.0, roll a random `f64`, select the stream whose cumulative probability covers the roll.
4. Draw one entry from the selected stream.
5. Decrement `remaining` for the selected attack. If it reaches 0, remove the stream.
6. For the normal stream, cap at `config.count` entries total.
7. Continue until all streams are exhausted.

When `interleave: false` globally (all attacks), generate all normal entries first, then all attack entries (in config order, each attack fully completes before the next starts).

**Parallel streaming interaction:**
- Attack mode does not **disables** the parallel (`rayon`) path when any attack exists with `interleave: true` or `type: multi_ordered` — ordering is guaranteed under parallel execution also and attack phase and normal phase is mixed by a random pattern.

### 3.3 Built-in Attack Example Configs

Create these YAML config files under `examples/`:

| File | Attack type | `type` | Key characteristics |
|------|-------------|--------|---------------------|
| `attack-brute-force.yaml` | Brute force login | `single_event` | Repeated POST /login from fixed IP, mostly 401, occasional 200 success |
| `attack-port-scan.yaml` | Port scan | `multi_ordered` | CONNECT / SYN probes to sequential ports from single IP, nmap UA, ordered sequence |
| `attack-ddos.yaml` | DDoS ramp-up | `threshold_field` | GET /index.html from rotating IPs, status distribution shifting: 30% 200, 50% 503, 20% 500 |
| `attack-sqli-probe.yaml` | SQL injection probe | `multi_ordered` | GET with SQL metacharacters (`'`, `OR 1=1`, `UNION`) in query params, 200/500 responses |
| `attack-credential-stuffing.yaml` | Credential stuffing | `single_event` | POST /auth from distributed IPs, varied user:pass combos, 401/403 with occasional 200 |

Each file is self-contained (includes base `count`, `output`, and `attacks`) with YAML comments explaining the simulated attack pattern.

### 3.4 CLI Additions

**On the `Generate` subcommand, add:**

```
  --attack <ATTACK_SPEC>           Define an inline attack (repeatable)
  --attack-config <FILE>           Load attacks from YAML file
  --attack-only                    Generate only attack entries (no normal logs)
```

- `--attack` format: `name=type:template[:count]` where type is `single`, `multi`, or `threshold`. For `multi`, multiple `--attack` flags with the same name are collected in order to form `sequence`. Example: `--attack scan=multi:"GET /probe HTTP/1.1":100 --attack scan=multi:"GET /probe2 HTTP/1.1"`.

- `--attack-config` path to a YAML file containing an `attacks:` key with a list of `AttackConfig` objects. These are merged with any inline `attacks` from the main `--config` YAML (CLI-loaded attacks take precedence on name collisions).

- `--attack-only` flag sets `interleave: false` on all attacks and disables normal template/legacy generation. The total output is the sum of all per-attack counts (or top-level `--count` distributed evenly if no per-attack counts set).

**`apply_cli_args` changes:**
- Add `attack_configs: Vec<AttackConfig>` and `attack_only: bool` parameters.
- Merge `attack_configs` into `config.attacks` (CLI wins on name collisions).
- Set an internal `attack_only` flag on `Config` that `Generator` checks to skip normal generation.

### 3.5 Integration Testing

**New test file `tests/unit/test_attack.rs`:**

| Test | Scenario | Validation |
|------|----------|------------|
| `test_attack_config_deser` | Full `AttackConfig` YAML deserialization — all 3 types | All fields deserialize correctly, defaults applied |
| `test_single_event_count` | Single event attack with `count: 10` | Exactly 10 entries produced, all use the configured template |
| `test_multi_ordered_sequence_order` | Multi-ordered attack with 4 templates, `count: 8`, `repeat: loop` | Entries 0-3 match templates 0-3, entries 4-7 match templates 0-3 again |
| `test_multi_ordered_once` | Multi-ordered with 3 templates, `repeat: "once"`, `count: 10` | Only 3 entries produced (sequence exhausted) |
| `test_threshold_field_proportion` | threshold_field on `status`, `min: 500`, `proportion: 0.7`, `count: 1000` | Between 650–750 entries have status >= 500 (statistical bound) |
| `test_interleaving_total_count` | Normal (100) + two attacks (50, 30) with `interleave: true` | Total 180 entries |
| `test_interleaving_no_normal` | `attack_only: true` with two attacks (50, 30) | Exactly 80 entries, no normal entries |
| `test_attack_var_override` | Per-attack `vars` override global `template_vars` | Rendered output contains the per-attack value, not the global one |
| `test_attack_parallel_fallback` | Config with attacks and `random_intensity >= 1.0` | Serial path used (no rayon), entries produced correctly |
| `test_attack_no_interleave_ordering` | Two attacks with `interleave: false` | All entries from attack A come before all entries from attack B |

**Validation methodology:**
- Deserialize attack configs from inline YAML strings (same pattern as `test_config_yaml_with_templates`).
- Generate small counts (10–100) for deterministic tests, larger (1000) for statistical proportion tests.
- Assert entry count, field values, ordering invariants, and string content matches expected templates.
- Use regex patterns to validate rendered entry format where applicable.
- For proportion tests, use a tolerance band (e.g., +/- 5 percentage points) rather than exact values.

## Phase 4: Performance & Advanced Features (Weeks 7-8)

### 4.1 Performance Optimization

#### 4.1.1 Buffered Streaming Writes
- Introduce `BufferedLogWriter<W: LogWriter>` wrapper struct that buffers output entries before flushing to the underlying writer.
- Configurable `buffer_size` (bytes, default 8192). Flush to inner writer when buffer exceeds this threshold.
- Implements `LogWriter` — transparent to the generator pipeline.
- Automatically wraps `FileWriter` and `StdoutWriter` in `generate_to_writer()` (but not `HttpWriter`/`KafkaWriter` which have their own batching).

**Config changes** — add to `OutputConfig`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `buffer_size` | `u64` | `8192` | Output buffer in bytes before flush (0 = no buffering). |

#### 4.1.2 Progress Reporting
- Add `ProgressReporter` struct that emits progress lines to stderr.
- Configurable via `--progress` CLI flag and `progress: true` config field.
- Format (single line, overwritten with `\r`):
  ```
  [loggen] 50,000 / 100,000 entries (50%) [2.3s elapsed, 21,739/s]
  ```
- Reports every 1 second (wall-clock) or every `progress_interval` entries (default 10,000), whichever comes first.
- Shows final summary on completion: `[loggen] Done: 100,000 entries in 4.1s (24,390/s)`
- **Silent** when output target is stderr (to avoid corrupting log output). Auto-detects: if `output.target == "stdout"`, progress goes to stderr; if file/HTTP/Kafka, progress also goes to stderr.
- Uses `AtomicBool` flag checked from the streaming loop — minimal overhead per entry (one atomic load).
- When no `--progress` flag and no config `progress` field: auto-enable if count >= 100,000 and output is not stdout.

**Config additions:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `progress` | `Option<bool>` | `None` | Enable/disable progress. `None` = auto (on for count >= 100K, off otherwise). |
| `progress_interval` | `u64` | `10000` | Entry count between progress updates (min 1000). |

**CLI addition:**
```
  --progress                  Show progress (auto-enabled for large counts)
  --no-progress               Disable progress reporting
```

#### 4.1.3 Parallelism Tuning
- Add `--threads` CLI flag and `num_threads` config field to control rayon thread pool size.
- When set, configure rayon's global pool via `rayon::ThreadPoolBuilder::new().num_threads(N).build_global()` before any parallel work.
- Defaults to `std::thread::available_parallelism().unwrap_or(4)`.
- Only relevant for the `write_template_parallel_stream` path (random_intensity >= 1.0, no interleaving attacks).

**Config addition:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `num_threads` | `Option<usize>` | `None` | Rayon thread count. `None` = system default. |

#### 4.1.4 Timestamp Caching
- In the streaming paths (`write_template_stream`, `write_legacy_stream`), compute the RFC 3339 timestamp string once before the loop, not once per entry.
- In the parallel path, each rayon worker computes its own timestamp per batch (already amortized over 5000 entries).
- In attack rendering, compute once per call to `render_attack_entry` (no change needed there).

**Performance targets** (measured on a modern 4+ core system, release build):
- Legacy mode (`message`/`level` only): ≥ 500,000 entries/sec
- Single template with 2 static vars: ≥ 300,000 entries/sec
- Template with random vars (`ip`, `status`, `user_agent`): ≥ 100,000 entries/sec
- Parallel path (intensity=1.0, 4+ templates): ≥ 200,000 entries/sec
- Attack `single_event` (serial, 3 random vars): ≥ 80,000 entries/sec
- Memory: peak < 50 MB RSS for 1M entries streamed to file

### 4.2 Advanced Streaming

#### 4.2.1 HTTP Output (`HttpWriter`)

**Architecture decision:** HTTP is implemented as a `LogWriter`, not a separate subcommand. The existing `Http` subcommand is removed.

**Implementation:**
- New struct `HttpWriter` implementing `LogWriter`.
- Uses the `ureq` crate (blocking HTTP client, no async runtime required).
- Entries are **batched**: accumulate up to `batch_size` entries in a `Vec<String>`, then POST them as a JSON array or NDJSON.
- POST body format (controlled by `format`):
  - `"ndjson"` (default): each log entry on its own line, `Content-Type: application/x-ndjson`
  - `"json"`: single JSON array of entry objects, `Content-Type: application/json`
  - `"raw"`: raw text body, one entry per line, `Content-Type: text/plain`
- Each entry in the batch is the `LogEntry.message` field (already rendered).
- On HTTP error (non-2xx): retry up to `retry_attempts` times with `retry_delay_ms` backoff between attempts.
- After exhausting retries: stop log generation with an error message (not a panic).
- Connection timeout: 5 seconds. Read timeout: 10 seconds.

**Config additions** — add to `OutputConfig`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | `Option<String>` | `None` | HTTP endpoint URL (required when `target: "http"`). |
| `batch_size` | `u64` | `100` | Max entries per POST request. |
| `format` | `String` | `"ndjson"` | Body format: `"ndjson"`, `"json"`, or `"raw"`. |
| `headers` | `Option<HashMap<String, String>>` | `None` | Custom HTTP headers (e.g. `Authorization: Bearer ...`). |
| `retry_attempts` | `u32` | `3` | Max retries on failed POST. |
| `retry_delay_ms` | `u64` | `1000` | Delay between retries (ms). |

**Example YAML:**
```yaml
output:
  target: http
  url: https://logs.example.com/api/v1/ingest
  batch_size: 500
  format: ndjson
  headers:
    Authorization: "Bearer token123"
    X-Source: "loggen"
  retry_attempts: 3
  retry_delay_ms: 2000
count: 10000
templates: ./templates/
```

#### 4.2.2 Kafka Output (`KafkaWriter`)

**Architecture decision:** Same as HTTP — a `LogWriter` implementation. The existing `Kafka` subcommand is removed.

**Implementation:**
- New struct `KafkaWriter` implementing `LogWriter`.
- Uses the `rdkafka` crate with `base` feature (no tokio, no async).
- Connects on construction using configured brokers.
- Each log entry is produced as a single message to the configured topic.
- Optional key: if `key_var` is set, the value of that template variable from the last-rendered entry is used as the Kafka message key. This enables log partitioning by e.g. source IP.
- Producer config:
  - `acks`: `"1"` (default, leader acknowledges). Configurable: `"0"`, `"1"`, `"all"`.
  - `queue.buffering.max.ms`: 100 (flush every 100ms).
  - `message.timeout.ms`: 5000.
- On producer error: log to stderr and continue (non-fatal). Count failures.
- On construction failure (e.g. unreachable broker): exit with error message.

**Config additions** — add to `OutputConfig`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `kafka` | `Option<KafkaOutputConfig>` | `None` | Kafka-specific settings (required when `target: "kafka"`). |

**`KafkaOutputConfig` struct:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `brokers` | `String` | `"localhost:9092"` | Comma-separated list of broker addresses. |
| `topic` | `String` | required | Kafka topic name. |
| `key_var` | `Option<String>` | `None` | Template variable name to use as message key. |
| `acks` | `String` | `"1"` | Required acks: `"0"`, `"1"`, `"all"`. |
| `timeout_ms` | `u64` | `5000` | Message delivery timeout (ms). |
| `batch_size` | `u64` | `100` | Max messages to buffer before flushing to librdkafka. |

**Example YAML:**
```yaml
output:
  target: kafka
  kafka:
    brokers: "kafka-1:9092,kafka-2:9092"
    topic: "app-logs"
    key_var: "ipv4"
    acks: "all"
count: 50000
templates: ./templates/
```

#### 4.2.3 File Output Enhancements

- Add `output.append: bool` (default `true` for backward compatibility).
- When `append: false`, use `OpenOptions::new().write(true).create(true).truncate(true)` instead of `append(true)`.
- Add `output.rotate_bytes: Option<u64>` (default `None`, i.e. no rotation).
  - When set, track bytes written to the current output file.
  - After each `write_entry` call, if cumulative bytes exceed `rotate_bytes`:
    1. Rename current file to `{path}.1` (overwriting previous `.1` if it exists).
    2. Open a new file at the original path.
  - No limit on number of backups — `.1` is always the single backup.

**Config additions** to `OutputConfig`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `append` | `bool` | `true` | Append to existing file vs truncate. |
| `rotate_bytes` | `Option<u64>` | `None` | Rotate (rename to .1) after this many bytes. |

### 4.3 CLI Enhancements

#### 4.3.1 Shell Completion Support

- Add new subcommand `Completions`:
  ```rust
  /// Generate shell completion script
  Completions {
      /// Shell type: bash, zsh, fish, powershell, elvish
      shell: String,
  }
  ```
- Uses the `clap_complete` crate to generate and print the completion script to stdout.
- User pipes it to their shell config:
  ```bash
  loggen completions bash > /etc/bash_completion.d/loggen
  loggen completions zsh > /usr/local/share/zsh/site-functions/_loggen
  loggen completions fish > ~/.config/fish/completions/loggen.fish
  ```
- Detection: if running interactively and no subcommand given, suggest running `loggen completions <shell>`.

#### 4.3.2 Config Validation

- Add `--validate` flag to `Generate` subcommand.
  ```rust
  /// Validate configuration and exit (no generation)
  #[arg(long)]
  validate: bool,
  ```
- When `--validate` is set:
  1. Load and merge config (same logic as normal run).
  2. Attempt `Generator::new(config)` — this panics on unknown template vars, catches the panic and prints a clean error message.
  3. Print validation summary: `"Config valid: N template(s), M attack(s), K entry/ies"`.
  4. Exit with code 0 on success, 1 on failure.
- Validation checks (in addition to existing template var validation):
  - Attack config consistency:
    - `threshold_field` must have `threshold` block.
    - `multi_ordered` must have non-empty `sequence`.
    - `single_event` must have non-empty `template`.
    - Attack counts must not exceed `count` if `attack_only` is false and `interleave` is false (warn, not error).
  - Output config consistency:
    - `target: "http"` requires `url`.
    - `target: "kafka"` requires `kafka` block with `topic`.
  - `random_intensity` must be 0.0–1.0.
  - `weight` values in attacks must be 0.0–1.0.

#### 4.3.3 Help System Improvements

- Add `after_help` to `Generate` subcommand with usage examples:
  ```
  EXAMPLES:
    loggen generate --count 100
    loggen generate -c examples/example.yaml
    loggen generate --templates ./templates/ --count 10000 --output output.log
    loggen generate --attack "brute=single:{{ ipv4 }} - POST /login {{ status }} :50"
  ```
- Add `after_help` to `Http` and `Kafka` subcommands.
- Remove the `try_show_completion_help` workaround if clap's native `SubcommandRequiredElseHelp` handles it properly; otherwise keep it.
- Add a top-level `after_help` showing:
  ```
  Run 'loggen <subcommand> --help' for subcommand-specific help.
  Run 'loggen completions <shell>' to generate shell completion scripts.
  ```

### 4.4 Config Struct Changes

New fields in `OutputConfig` (existing: `target`, `path`):

```rust
// New fields for OutputConfig
#[serde(default)]
pub buffer_size: u64,
#[serde(default)]
pub progress: Option<bool>,
#[serde(default)]
pub progress_interval: u64,
#[serde(default)]
pub url: Option<String>,
#[serde(default)]
pub batch_size: u64,
#[serde(default = "default_output_format")]
pub format: String,
#[serde(default)]
pub headers: Option<HashMap<String, String>>,
#[serde(default)]
pub retry_attempts: u32,
#[serde(default)]
pub retry_delay_ms: u64,
#[serde(default)]
pub kafka: Option<KafkaOutputConfig>,
#[serde(default = "default_append")]
pub append: bool,
#[serde(default)]
pub rotate_bytes: Option<u64>,
```

New structs:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct KafkaOutputConfig {
    #[serde(default = "default_kafka_brokers")]
    pub brokers: String,
    pub topic: String,
    #[serde(default)]
    pub key_var: Option<String>,
    #[serde(default = "default_kafka_acks")]
    pub acks: String,
    #[serde(default = "default_kafka_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_kafka_batch")]
    pub batch_size: u64,
}
```

New top-level Config fields:

```rust
#[serde(default)]
pub num_threads: Option<usize>,
#[serde(default)]
pub progress: Option<bool>,
#[serde(default = "default_progress_interval")]
pub progress_interval: u64,
```

### 4.5 New Types / Structs

```
BufferedLogWriter<W: LogWriter> {
    inner: W,
    buffer: Vec<u8>,
    buffer_size: u64,
}

HttpWriter {
    url: String,
    client: ureq::Agent,
    batch: Vec<String>,
    batch_size: u64,
    format: String,
    headers: HashMap<String, String>,
    retry_attempts: u32,
    retry_delay_ms: u64,
    fallback_path: Option<String>,
    entries_sent: u64,
}

KafkaWriter {
    producer: rdkafka::producer::FutureProducer,
    topic: String,
    key_var: Option<String>,
    batch: Vec<String>,
    batch_size: u64,
    entries_produced: u64,
    errors: u64,
}

ProgressReporter {
    start: Instant,
    last_report: Instant,
    interval: Duration,
    entry_interval: u64,
    total: u64,
    last_reported_entry: u64,
    enabled: bool,
}
```

### 4.6 Factory Changes

`create_writer` in `cli.rs` grows to handle new targets:

```rust
pub fn create_writer(config: &Config) -> Result<Box<dyn LogWriter>, Box<dyn std::error::Error>> {
    match config.output.target.as_str() {
        "http" => {
            validate_http_config(&config.output)?;
            let writer = HttpWriter::new(&config.output)?;
            Ok(Box::new(writer))
        }
        "kafka" => {
            validate_kafka_config(&config.output)?;
            let writer = KafkaWriter::new(&config.output)?;
            Ok(Box::new(writer))
        }
        "file" => {
            let path = config.output.path.as_deref().unwrap_or("output.log");
            let writer = FileWriter::new(path, !config.output.append, config.output.rotate_bytes)?;
            let writer = BufferedLogWriter::new(writer, config.output.buffer_size);
            Ok(Box::new(writer))
        }
        _ => { // "stdout"
            let writer = StdoutWriter::new();
            let writer = BufferedLogWriter::new(writer, config.output.buffer_size);
            Ok(Box::new(writer))
        }
    }
}
```

### 4.7 Integration Testing

#### Test file `tests/unit/test_phase4.rs` or extend existing files:

**Progress reporting:**

| Test | Scenario | Validation |
|------|----------|------------|
| `test_progress_basic_output` | 1000 entries with `progress: true` | stderr contains at least one progress line matching the format `[loggen]` |
| `test_progress_disabled` | 10 entries without progress flag | No `[loggen]` output on stderr |
| `test_progress_auto_enable` | 150000 entries, no progress flag, file output | Progress auto-enabled (stderr has progress) |
| `test_progress_summary_line` | Capture final line after generation completes | Matches `Done: N entries in Xs (Y/s)` |

**Buffering:**

| Test | Scenario | Validation |
|------|----------|------------|
| `test_buffered_writer_flush_on_size` | Write 100 small entries with `buffer_size: 50` | Inner writer called fewer than 100 times |
| `test_buffered_writer_flush_on_drop` | Write entries, drop writer | All entries flushed before drop |

**HTTP writer (requires mock HTTP server):**

| Test | Scenario | Validation |
|------|----------|------------|
| `test_http_writer_send_single` | 1 entry, `batch_size: 1` | POST body contains the entry, Content-Type is `application/x-ndjson` |
| `test_http_writer_batching` | 250 entries, `batch_size: 100` | Exactly 3 POST requests received |
| `test_http_writer_retry` | Server returns 503 twice, then 200 | Exactly 3 requests made, entries delivered |
| `test_http_writer_retry_exhausted` | Server returns 500 always | After 3 retries, entries logged as failed (stderr check) |

**Kafka writer (test block skipped if no broker available):**

| Test | Scenario | Validation |
|------|----------|------------|
| `test_kafka_config_deser` | Full Kafka output config YAML | All fields deserialize correctly |
| `test_kafka_writer_connect_failure` | Invalid broker address | Returns error on construction |
| `test_kafka_writer_produce` | Valid broker, 10 entries, `batch_size: 5` | (integration, requires Kafka) 10 messages produced to topic |

**File rotation:**

| Test | Scenario | Validation |
|------|----------|------------|
| `test_file_append_mode` | Write to file with `append: false` (truncate) | Only new entries present |
| `test_file_rotation` | `rotate_bytes: 100`, write 200 bytes | `output.log` exists (new file) and `output.log.1` exists (rotated) |
| `test_file_rotation_single_entry` | `rotate_bytes: 1000`, write 1 small entry | No rotation occurs, only `output.log` |

**Shell completions:**

| Test | Scenario | Validation |
|------|----------|------------|
| `test_completions_subcommand_exists` | Parse `Cli` with `completions bash` | Subcommand parsed, shell value is `"bash"` |
| `test_completions_bash_output` | Run completions subcommand with `bash` | Output contains `_loggen()` bash function |
| `test_completions_zsh_output` | Run completions subcommand with `zsh` | Output contains `#compdef _loggen` |

**Config validation (`--validate`):**

| Test | Scenario | Validation |
|------|----------|------------|
| `test_validate_valid_config` | Valid config file + `--validate` | Exit code 0, success message to stderr |
| `test_validate_unknown_var` | Config with `{{ nonexistent }}` + `--validate` | Exit code 1, error message mentions `nonexistent` |
| `test_validate_http_no_url` | `target: "http"` without `url` + `--validate` | Exit code 1, error about missing URL |
| `test_validate_threshold_no_threshold` | Attack `type: threshold_field` without `threshold` block | Exit code 1, error about missing threshold |
| `test_validate_multi_no_sequence` | Attack `type: multi_ordered` without `sequence` | Exit code 1, error about missing sequence |

**Performance benchmarks (criterion, `benches/` directory):**

| Benchmark | Input | Expectation |
|-----------|-------|-------------|
| `bench_legacy_100k` | Legacy mode, 100K entries | Throughput ≥ 500K/s |
| `bench_template_simple_100k` | 1 template with 2 static vars, 100K entries | Throughput ≥ 300K/s |
| `bench_template_random_100k` | 1 template with `ip`, `status`, `user_agent`, 100K entries | Throughput ≥ 100K/s |
| `bench_parallel_500k` | 4 templates, `random_intensity: 1.0`, 500K entries | Throughput ≥ 200K/s |
| `bench_attack_single_50k` | 1 `single_event` attack, 3 random vars, 50K entries | Throughput ≥ 80K/s |

### 4.8 Dependency Additions

| Crate | Version | Features | Used By |
|-------|---------|----------|---------|
| `ureq` | `2` | (default) | `HttpWriter` |
| `rdkafka` | `0.37` | `base` | `KafkaWriter` |
| `clap_complete` | `4` | (default) | Shell completions |

`indicatif` was considered for progress bars but rejected due to terminal-width complexity. The simple line-based reporter is more robust for pipe/redirect scenarios.

### 4.10 Required Updates to Existing Code

#### `src/output.rs`
- `FileWriter::new()` signature changes to accept `truncate: bool` and `rotate_bytes: Option<u64>`.
- `StdoutWriter` wrapped in `BufferedLogWriter` by `create_writer` (no struct changes needed).
- New public types: `BufferedLogWriter<W>`, `HttpWriter`, `KafkaWriter`, `ProgressReporter`.

#### `src/config.rs`
- `OutputConfig` gains all fields from §4.4. New default functions: `default_output_format`, `default_append`, `default_kafka_brokers`, `default_kafka_acks`, `default_kafka_timeout`, `default_kafka_batch`, `default_progress_interval`.
- New struct `KafkaOutputConfig` with serde derives.
- `Config` gains `num_threads`, `progress`, `progress_interval`.
- `Config::default()` updated to include new fields.

#### `src/cli.rs`
- `create_writer()` restructured per §4.6 factory pseudocode.
- `parse_attack_spec`, `merge_cli_attacks`, `load_attack_config_file`, `apply_cli_args` unchanged.
- New validation functions: `validate_http_config`, `validate_kafka_config`.

#### `src/generator.rs`
- `generate_to_writer()` and streaming methods integrate `ProgressReporter` checks.
- No structural changes to core generate/attack logic.
- Timestamp caching: compute `ts_to_rfc3339(current_timestamp())` once before loops in `write_template_stream`, `write_legacy_stream`.

#### `src/main.rs`
- `Cli` gains `Completions` subcommand.
- `Generate` gains `--validate`, `--progress`, `--no-progress`, `--threads` flags.
- `handle_generate` updated to pass new flags, handle `--validate` early exit, configure rayon threads.
- `handle_http` / `handle_kafka` updated to construct config and call `generate_to_writer` instead of printing stubs.
- New `handle_completions` function.

### 4.9 Implementation Order

1. **BufferedLogWriter + progress reporting** — no new deps, can be done first.
2. **File rotation + append mode** — no new deps, modifies `FileWriter`.
3. **CLI validation (`--validate`)** — no new deps, adds validation logic.
4. **HttpWriter** — requires `ureq` dep.
5. **KafkaWriter** — requires `rdkafka` dep.
6. **Shell completions** — requires `clap_complete` dep.
7. **Help system examples** — pure CLI text, no deps.
8. **Performance benchmarks** — requires `criterion` dev-dep.
9. **Integration tests** — throughout the phase, as each component is built.

## Phase 5: Documentation & Testing (Weeks 9-10)

### 5.1 Comprehensive Documentation
- **Configuration Reference:** Detailed catalog of all `Config`, `OutputConfig`, `AttackConfig`, `KafkaOutputConfig`, and `ThresholdConfig` fields, including types, defaults, and constraints.
- **Template & Variable Guide:** Documentation on using Tera filters (e.g., `date`) and utilizing all built-in random variables (`ipv4`, `user_agent`, etc.).
- **Attack Scenario Gallery:** An annotated collection of `.yaml` examples in `examples/` simulating realistic attack patterns (Brute force, DDoS, etc.).
- **CLI Cheat Sheet:** Quick reference for all `Generate` and `Completions` subcommands and flags.

### 5.2 Exhaustive Testing & Validation
- **Performance Benchmarking:** Using `criterion` to validate all targets defined in §4.1.5 (Legacy, Template, Parallel, and Attack throughput).
- **Regression Suite:** Automated integration tests ensuring attack interleaving, `common` field freezing, and `raw_intensity` logic remain intact.
- **Writer Integration Tests:**
  - `HttpWriter`: Test batching, `ndjson`/`json` formats, and retry mechanisms using a mock HTTP server.
  - `KafkaWriter`: Test message production and `key_var` partitioning with a local Kafka instance.
- **Boundary & Stress Testing:**
  - Fuzzing `threshold_field` boundaries and `proportion` values (0.0, 1.0, and edge cases).
  - Large-scale load testing (e.g., 10M+ entries) to monitor memory/RSS stability.
- **Coverage Audit:** Ensure unit and integration test coverage for all new structs (`BufferedLogWriter`, `HttpWriter`, `KafkaWriter`, `ProgressReporter`).

### 5.3 Final Polish & Release
- **CI/CD Verification:** Ensure `rust.yml` executes the full suite of unit, integration, and benchmarking tests on every PR.
- **Dependency & Binary Audit:** Review the impact of `ureq`, `rdkafka`, and `tera` on binary size and compile times; optimize features where possible.
- **User Experience (UX) Review:** Verify all `--help` and `after_help` text is clear and all `Completions` scripts work as expected across shells.
- **Release Preparation:** Tagging the version in Git and preparing the GitHub release notes.

## Phase 6: Remove Attack Pattern Feature (Refactor)

This phase removes the entire attack pattern generation system (formerly Phase 3), including all associated config types, CLI flags, template validation, generator engine, tests, documentation, and example files. This simplifies the codebase, removes ~1700 lines, and eliminates the `--attack`/`--attack-config`/`--attack-only` CLI surface.

**Why:** The attack feature added significant complexity (3 attack types, interleaving logic, rejection sampling, variable modes, common fields) and is no longer needed in the project scope.

### 6.1 Remove Config Types and Fields (`src/config.rs`)

1. Delete the following structs:
   - `ThresholdConfig` + `default_threshold_proportion()`
   - `AttackVarConfig` + `default_attack_var_mode()`
   - `AttackConfig` + `default_attack_weight()` + `default_attack_repeat()`

2. Remove from `Config` struct: `attacks` and `attack_only` fields.

3. Remove from `Config::default()`: `attacks: None` and `attack_only: false`.

4. Delete inline test `test_config_yaml_with_attacks`.

5. Remove attack assertions from `test_config_defaults`.

### 6.2 Remove Attack Engine (`src/generator.rs`)

1. Remove import of `AttackConfig`, `AttackVarConfig`, `ThresholdConfig`.

2. Remove attack template registration + validation loop in `Generator::new()`.

3. Redirect `generate()` + `generate_to_writer()` to remove attack dispatch (remove `if self.has_attacks()` branches).

4. Remove all types and functions:
   - `AttackCursor` struct, impl, Default
   - `AttackEngine` struct + impl (`new`, `is_exhausted`, `attack_remaining`)
   - `is_value_in_bucket()`
   - `pick_attack_var_value()`
   - `render_attack_entry()`
   - `has_attacks()`
   - `generate_attack_only()`
   - `generate_attack_interleaved()`
   - `generate_with_attacks()`
   - `write_attack_stream()`
   - `write_attack_interleaved()`

### 6.3 Remove Attack CLI Parsing (`src/cli.rs`)

1. Remove `AttackConfig` import.

2. Delete functions:
   - `parse_attack_spec()`
   - `merge_cli_attacks()`
   - `load_attack_config_file()`

3. Simplify `apply_cli_args`: remove `attack_configs` and `attack_only` params + merging logic.

### 6.4 Remove Attack CLI Flags (`src/main.rs`)

1. Remove `--attack`, `--attack-config`, `--attack-only` CLI flags from `Generate` subcommand and struct fields.

2. Remove attack parsing + attack-config loading from `handle_generate()`.

3. Remove attack validation block from `validate_config()`.

4. Remove `config.attacks` count from `run_validate()` output.

5. Update imports and match arm destructuring.

### 6.5 Update Public API (`src/lib.rs`)

1. Remove from config re-exports: `AttackConfig`, `AttackVarConfig`, `ThresholdConfig`.

2. Remove from generator re-exports: `AttackCursor`, `AttackEngine`.

### 6.6 Remove Test Files and Test References

1. **Delete `tests/unit/test_attack.rs`** entirely.

2. **Remove from `tests/unit/mod.rs`**: `pub mod test_attack;`.

3. **`tests/unit/cli.rs`** — Remove `test_apply_cli_args_attack_only` and `test_parse_attack_spec_edge_cases`.

4. **Remove `attacks`/`attack_only` fields** from `test_config()` / `base_config()` helpers in `tests/unit/test_generator.rs` and `tests/unit/test_date_filter.rs`.

### 6.7 Remove Attack Example Files

Delete 5 YAML files: `examples/attack-brute-force.yaml`, `examples/attack-port-scan.yaml`, `examples/attack-ddos.yaml`, `examples/attack-sqli-probe.yaml`, `examples/attack-credential-stuffing.yaml`.

### 6.8 Update Documentation

1. Delete `docs/attack-gallery.md`.

2. `docs/configuration-reference.md` — Remove attack-related config rows and tables.

3. `docs/cli-cheatsheet.md` — Remove `--attack`, `--attack-config`, `--attack-only` rows.

### 6.9 Update `AGENTS.md`

1. Update Phase description (line 3): change "Phases 1–3" to "Phases 1–2".

2. Remove attack function descriptions in Structure section.

3. Remove attack streaming and quirk bullet points.

4. Remove `AttackCursor`/`AttackEngine` security audit findings and API changes section.

### 6.10 Update `Plan.md`

1. Update top-level description (line 3) to reflect new phase structure.

2. Remove old Phase 3 content (lines 96–295) or replace with a stub noting it was removed in Phase 6.

### 6.11 Build and Test Verification

```sh
cargo build
cargo test --lib
cargo test --test mod
cargo clippy --all-targets -- -D warnings
```

No new dependencies required. This is purely a deletion/refactor phase.

---

## Key Dependencies to Consider

### Core Rust Crates:
- `clap` or `structopt` for CLI
- `serde` and `serde_yaml` for configuration
- `tera` for Jinja2-like templating
- `tokio` for async operations
- `regex` for pattern matching
- `rand` for randomization
- `chrono` for timestamps

### Testing Tools:
- `cargo test` for unit tests
- `criterion` for benchmarking
- `mockall` for mocking dependencies

This phased approach ensures we build a solid foundation first, then gradually add more sophisticated features while maintaining quality and test coverage throughout the development process.
