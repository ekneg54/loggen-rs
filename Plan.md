# Phased Implementation Plan for loggen-rs

## Phase 1: Foundation & Core Features

### 1.1 Project Setup & Core Architecture

**Project setup:**
- Initialize Rust project with proper structure, Cargo.toml with dependencies
- Configure build system and CI/CD pipeline
- Implement basic code documentation standards

**Core architecture:**
- Core data structures for log entries
- Basic log generation engine (legacy mode: `message`/`log_level`)
- YAML configuration parser (`Config`, `OutputConfig`, `LogEntry`)
- CLI interface (`clap` derive, `Generate` subcommand with `--count`, `--config`, `--output`, `--level`, `--message`)
- Stdout and file output streams (`LogWriter` trait, `StdoutWriter`, `FileWriter` with append mode)
- Example config demonstrating core functionality

**Basic testing:** Unit tests for core components, basic integration tests.

### 1.2 Template System & Randomization

**Template engine:**
- Uses **Tera** crate (Jinja2-inspired: `{{ var }}`, filters, `{% if %}`, `{% for %}`)
- Config: `logs: Vec<String>` (inline templates), `templates: String` (path to `.logtpl` dir), `template_vars: HashMap<String, String>`, `seed: u64`
- CLI: `--var key=value` (repeatable), `--templates`
- Strict validation: all template vars must be defined or startup fails
- Pipeline: `Generator` loads templates or falls back to legacy mode; per-entry rotation, render, output

**Built-in variables:**
| Variable | Description |
|---|---|
| `timestamp` | Current Unix timestamp (use `{{ timestamp \| date(format="...") }}`) |
| `level` | From `log_level` config / `--level` |
| `index` | 1-based counter per run |
| `message` | From `message` config / `--message` (backwards compat) |

**Random variables** (auto-generated when not explicitly set):
`ipv4`, `ipv6`, `user_agent`, `email`, `url`, `port`, `status` (weighted HTTP), `user`

**Randomization:**
- `random_intensity: f64` (0.0–1.0, default 1.0): controls per-entry randomize probability
- `template_rotation: "sequential" | "random" | "round_robin"` (default "sequential")
- User-defined random pools via `random_vars: { name: [val1, val2] }`

**Default templates:** `templates/` directory with Apache combined, Nginx combined, Syslog (RFC 3164). Example configs referencing them.

**Concurrent design:** Streaming pipeline — read all templates, then produce output with templates applied (no buffering all entries in memory). Rayon parallel path via `mpsc` channel when `random_intensity >= 1.0`.

**Dependencies:** `tera`, `rand`

### 1.3 Performance & Advanced Features

**BufferedLogWriter:** Wrapper implementing `LogWriter`. Flushes inner writer when buffer exceeds `buffer_size` (default 8192 bytes). Transparently wraps `FileWriter`/`StdoutWriter` in `create_writer`.

**Progress reporting:** `ProgressReporter` emits to stderr every 1s or `progress_interval` entries (default 10K). Format: `[loggen] N / TOTAL entries (P%) [Ts elapsed, R/s]`. Final summary on completion. Auto-enabled at ≥100K entries. Silent when output is stderr.

**Parallelism tuning:** `--threads` / `num_threads` config to control rayon thread pool. Only relevant for parallel streaming path.

**Timestamp caching:** Compute RFC 3339 timestamp once before streaming loops (both sequential paths).

**HttpWriter:** `LogWriter` impl using `ureq`. Batches entries (configurable `batch_size`, default 100), POSTs as ndjson/json/raw. Retries up to `retry_attempts` (default 3) with `retry_delay_ms` backoff. Configurable custom headers.

**KafkaWriter:** `LogWriter` impl using `rdkafka` (base feature). Produces messages to configured topic. Optional `key_var` for partitioning. Configurable brokers, acks, timeout.

**File output enhancements:**
- `append: bool` (default true). When false, truncate instead of append.
- `rotate_bytes: Option<u64>` — rename to `{path}.1` and start new file when exceeded.

**Shell completions:** `loggen completions <bash|zsh|fish|powershell|elvish>` subcommand via `clap_complete`.

**Config validation:** `--validate` flag checks template vars, output config consistency (`http`→url, `kafka`→topic block), `random_intensity` range.

**Help system improvements:** `after_help` on `Generate` subcommand with usage examples. Top-level help reference.

**OutputConfig field additions:**

| Field | Type | Default | Description |
|---|---|---|---|
| `buffer_size` | `u64` | `8192` | Output buffer in bytes (0 = no buffering) |
| `progress` | `Option<bool>` | `None` | Enable progress (None = auto at ≥100K) |
| `progress_interval` | `u64` | `10000` | Entry count between progress updates |
| `url` | `Option<String>` | `None` | HTTP endpoint (required when `target: "http"`) |
| `batch_size` | `u64` | `100` | Max entries per POST request |
| `format` | `String` | `"ndjson"` | Body format: `ndjson`, `json`, `raw` |
| `headers` | `Option<HashMap<String, String>>` | `None` | Custom HTTP headers |
| `retry_attempts` | `u32` | `3` | Max retries on failed POST |
| `retry_delay_ms` | `u64` | `1000` | Delay between retries |
| `kafka` | `Option<KafkaOutputConfig>` | `None` | Kafka-specific settings |
| `append` | `bool` | `true` | Append vs truncate file |
| `rotate_bytes` | `Option<u64>` | `None` | Rotate after N bytes |

**`KafkaOutputConfig` fields:** `brokers` ("localhost:9092"), `topic` (required), `key_var` (None), `acks` ("1"), `timeout_ms` (5000), `batch_size` (100).

**Top-level Config additions:** `num_threads: Option<usize>`, `progress: Option<bool>`, `progress_interval: u64`.

**`create_writer` (`cli.rs`):** Routes to `HttpWriter` / `KafkaWriter` / `BufferedLogWriter<FileWriter>` / `BufferedLogWriter<StdoutWriter>` based on `output.target`.

**Dependencies:** `ureq` (2), `rdkafka` (0.37, `base`), `clap_complete` (4).

### 1.4 Documentation & Testing

**Documentation:** Configuration reference (all config fields), template & variable guide (Tera filters, random vars), CLI cheat sheet.

**Testing:**
- Criterion benchmarks for all performance targets
- Regression suite for streaming and randomization logic
- Writer integration tests (HttpWriter mock server, KafkaWriter with local broker)
- Boundary/stress testing (10M+ entry load test, edge cases)
- Coverage audit

**Final polish:** CI/CD verification, dependency/binary size audit, UX review of help text and completions, release preparation.

### Implementation Order

1. Project setup + core architecture (legacy mode)
2. Template system + randomization
3. BufferedLogWriter + progress reporting
4. File rotation + append mode
5. HttpWriter + KafkaWriter
6. Config validation (`--validate`)
7. Shell completions + help system
8. Documentation, benchmarks, integration tests

---

## Phase 2: Simulation & Timing Control

### 2.1 SimulationConfig (`src/config.rs`)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct SimulationConfig {
    #[serde(default)]
    pub delay: Option<String>,       // "min-max" ms, e.g. "100-500"
    #[serde(default = "default_sim_rotation")]
    pub rotation: String,            // "none", "round_robin", "random"
}
```

Added to `Config`:
```rust
#[serde(default)]
pub simulation: Option<SimulationConfig>,
```

**Example YAML:**
```yaml
simulation:
  delay: "200-1000"
  rotation: round_robin
count: 10000
templates: ./templates/
```

### 2.2 Delay Implementation

- Parse `delay` string `"min-max"` → `(u64, u64)` in milliseconds.
- In all three streaming paths in `generator.rs` (`write_legacy_stream`, `write_template_stream`, `write_template_parallel_stream`), after each entry write, sleep for a random duration in `[min, max]`:

```rust
if let Some((min, max)) = &delay_range {
    let sleep_ms = rng.gen_range(min..=max);
    std::thread::sleep(Duration::from_millis(sleep_ms));
}
```

- For the parallel path, delay is applied on the receiver side (after receiving each batch from the channel), so wall-clock timing is realistic.

### 2.3 Rotation Mode

Controls how simulation cycles through multiple log sources:
- `"none"`: No cycling — use primary/default source.
- `"round_robin"`: Cycle through sources in order, one per cycle.
- `"random"`: Pick a random source per entry.

### 2.4 CLI Additions (`src/main.rs`)

```rust
#[arg(long, value_name = "MIN-MAX")]
sim_delay: Option<String>,

#[arg(long, value_name = "MODE")]
sim_rotation: Option<String>,
```

Passed through `apply_cli_args` → `config.simulation`.

### 2.5 Config Validation

- `delay` parses as `u64-u64` with `min <= max`.
- `rotation` is one of `"none"`, `"round_robin"`, `"random"`.

### 2.6 Example Files

Three new YAML files under `examples/`:
- `simulation-basic.yaml` — delay with legacy mode
- `simulation-with-templates.yaml` — delay + rotation with templates
- `simulation-file-output.yaml` — simulation with file output

Also update `main.rs` help text with a simulation example.

### 2.7 Tests

| Test | Scenario | Validation |
|---|---|---|
| `test_simulation_delay_parsing` | `"100-500"` → `(100, 500)` | Correct parsed range |
| `test_simulation_delay_invalid` | `"abc"`, `"100"`, `"200-100"` | Error on bad format |
| `test_simulation_rotation_default` | No simulation config | `rotation` defaults to `"none"` |
| `test_simulation_rotation_values` | `"round_robin"`, `"random"`, `"none"` | All accepted |
| `test_simulation_rotation_invalid` | `"foo"` | Error |
| `test_simulation_delay_stream` | 5 entries with delay `"0-1"` | All 5 written, total wall-time ≥ 0ms |
| `test_simulation_yaml_deser` | Full YAML with simulation block | Deserializes correctly |
| `test_simulation_cli_override` | CLI `--sim-delay` overrides config | Delay config updated |

### 2.8 Required Updates

| File | Changes |
|---|---|
| `src/config.rs` | New `SimulationConfig` struct, `Config.simulation` field, `default_sim_rotation` |
| `src/generator.rs` | Parse delay range, apply sleep in all 3 streaming methods |
| `src/cli.rs` | `apply_cli_args` accepts `sim_delay`/`sim_rotation` |
| `src/main.rs` | Add `--sim-delay`, `--sim-rotation` flags, pass to `apply_cli_args`, validation |
| `examples/` | 3 new YAML files |

### 2.9 Implementation Order

1. `SimulationConfig` struct + deser + defaults
2. Config validation (delay format, rotation values)
3. CLI flags + `apply_cli_args` wiring
4. Delay logic in streaming paths
5. Rotation mode logic
6. Example files + CLI help update
7. Tests

---

## Key Dependencies

### Core Rust Crates:
- `clap` (derive) for CLI
- `serde` + `serde_yaml` for configuration
- `tera` for Jinja2-like templating
- `regex` for pattern matching
- `rand` for randomization
- `chrono` for timestamps
- `rayon` for parallel streaming
- `ureq` (2) for HTTP output
- `rdkafka` (0.37, `base`) for Kafka output
- `clap_complete` (4) for shell completions

### Testing Tools:
- `cargo test` for unit/integration tests
- `criterion` for benchmarks
