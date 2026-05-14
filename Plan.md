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

- **`single_event`**: Render `attack.template` via Tera. Merge is: (1) global `template_vars`, (2) built-in vars, (3) random vars per `random_intensity`, (4) `attack.vars` on top (strongest override). Each entry is independent. Respects `random_intensity` for non-attack-vars random fields.

- **`multi_ordered`**: Maintain a per-attack `AttackCursor`. Pick template from `sequence[cursor.sequence_index]`, advance cursor by 1. When cursor reaches `sequence.len()`: if `repeat: "once"`, mark attack exhausted (remaining = 0); if `"loop"`, reset cursor to 0. Merge variables in same priority as `single_event`. The cursor tracks per-attack state, so interleaved multi_ordered attacks maintain correct ordering across entry boundaries.

- **`threshold_field`**: Render template via Tera (same merge logic). Extract the rendered value of `threshold.field` from the template context — note: the template variable value is known at render time (it's in `ctx_values` before Tera rendering). Use rejection sampling: regenerate the random variable for `threshold.field` up to 10 times until the drawn value falls in the desired bucket. After the required `proportion` of entries are in-bucket, allow remaining entries to fall anywhere (i.e. stop rejecting). If retries exhausted (10 attempts), emit the last rendered value anyway. This ensures the output proportion converges toward the target without risking infinite loops.

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
- Attack mode **disables** the parallel (`rayon`) path when any attack exists with `interleave: true` or `type: multi_ordered` — ordering cannot be guaranteed under parallel execution.
- For `threshold_field` or `single_event` with `interleave: false`, the serial streaming path is used (attack phase runs after normal phase on the main thread).
- The parallel path is only active when no attacks are configured at all (existing behavior unchanged).

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
- Implement efficient large volume generation
- Add progress reporting system
- Optimize memory usage for large log files
- Implement parallel processing capabilities

### 4.2 Advanced Streaming
- Complete stdout/file output functionality
- Implement HTTP endpoint streaming (basic version)
- Add Kafka broker streaming support
- Create output buffering system

### 4.3 CLI Enhancements
- Complete help system and usage examples
- Add advanced CLI options
- Implement configuration validation
- Add command completion support

## Phase 5: Documentation & Testing (Weeks 9-10)

### 5.1 Comprehensive Documentation
- Create detailed user guide
- Document all configuration options
- Add examples for all features
- Write API documentation

### 5.2 Testing Coverage
- Complete unit test suite (100% coverage)
- Integration test for complete workflow
- Performance benchmarks
- Security testing for attack patterns

### 5.3 Final Polish
- Code review and optimization
- User experience testing
- Documentation review
- Release preparation

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
