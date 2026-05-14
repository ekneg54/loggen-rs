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

### 3.1 Sigma Rule Integration
- Implement Sigma rule parsing capability
- Create Sigma rule to log pattern mapping system
- Build attack pattern generation engine

### 3.2 Attack Templates
- Create library of common attack patterns (SQLi, XSS, DDoS, etc.)
- Implement corresponding log entries for each attack
- Add attack response log entries
- Build attack scenario generation system

### 3.3 Integration Testing
- Test attack pattern generation with real Sigma rules
- Validate generated logs match expected patterns
- Performance testing for attack scenarios

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
