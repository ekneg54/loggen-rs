# loggen-rs

A high-performance log generator written in Rust. Generates realistic log entries
using Tera (Jinja2) templates and built-in random variables — streamed to stdout,
file, HTTP, or Kafka.

## Installation

### Pre-built binaries

Download the latest release tarball for your platform from the
[releases page](https://github.com/ekneg54/loggen-rs/releases) and extract the
`loggen` binary.

### Container image

A container image is available via GitHub Container Registry:

```bash
docker pull ghcr.io/ekneg54/loggen-rs:latest
```

### Build from source

```bash
cargo install --git https://github.com/ekneg54/loggen-rs
```

Or clone and build manually (see [Building](#building)).

## Quick Start

```bash
loggen generate --count 100                                                   # 100 basic entries
loggen generate --templates ./templates/ --count 1000 --output out.log        # Template mode
loggen generate --config examples/template-example.yaml                       # From YAML
loggen http --url http://localhost:8080/logs --count 1000                     # HTTP output
loggen kafka --topic logs --count 500                                         # Kafka output
```

## Built-in Variables

Automatically available in templates:

| Variable | Type | Description |
|----------|------|-------------|
| `{{ timestamp }}` | datetime | Current UTC timestamp (use `| date(format="...")`) |
| `{{ level }}` | string | Log level from config (`log_level`) |
| `{{ index }}` | u64 | 1-based entry counter |
| `{{ message }}` | string | Message from config (`message`) |
| `{{ ip }}` / `{{ ipv4 }}` | string | Random IPv4 address |
| `{{ ipv6 }}` | string | Random IPv6 address |
| `{{ user_agent }}` | string | Random User-Agent |
| `{{ email }}` | string | Random email |
| `{{ url }}` | string | Random URL path |
| `{{ port }}` | u16 | Random port number |
| `{{ status }}` | u16 | Weighted random HTTP status |
| `{{ user }}` | string | Random username |

## Configuration

All features are configured via YAML or CLI flags. See full reference:

- [Configuration Reference](docs/configuration-reference.md)
- [Template & Variable Guide](docs/template-guide.md)
- [CLI Cheat Sheet](docs/cli-cheatsheet.md)

### Minimal config

```yaml
count: 100
log_level: INFO
message: "Example log entry"
```

### Template mode with random vars

```yaml
count: 1000
logs:
  - '{{ ipv4 }} - {{ email }} [{{ timestamp | date(format="%d/%b/%Y:%H:%M:%S %z") }}] "{{ method }} {{ url }} HTTP/1.1" {{ status }} {{ port }}'
  - '{{ level }} | {{ ipv4 }} | {{ user_agent }} | {{ message }}'
template_vars:
  app_name: loggen
random_intensity: 1.0
template_rotation: round_robin
```

## Key Features

- **Template Engine:** Tera (Jinja2-inspired) with `{{ var }}`, filters, `{% if %}`, `{% for %}`
- **Auto-Randomization:** Built-in generators for realistic IPs, UAs, emails, URLs, ports, status codes
- **Streaming Output:** Memory-efficient pipeline — no buffering all entries in memory
- **Parallel Generation:** Rayon-based parallel batch processing for high throughput
- **Multiple Targets:** stdout, file (with rotation), HTTP (with batching/retry), Kafka
- **Progress Reporting:** Real-time stats to stderr for large generation tasks
- **Seeded RNG:** Reproducible output via `seed` config or fixed seed
- **Shell Completions:** Generate bash/zsh/fish/powershell/elvish completion scripts

## Example Configs

Browse `examples/` for ready-to-run YAML files:

| File | Description |
|------|-------------|
| `minimal.yaml` | Minimal config (5 entries, defaults) |
| `example.yaml` | Basic config with all core fields |
| `file-output.yaml` | File output destination |
| `file-output-enhanced.yaml` | File rotation, buffering, truncation |
| `template-example.yaml` | Template mode with built-in random vars |
| `http-output.yaml` | HTTP output with NDJSON batching |
| `kafka-output.yaml` | Kafka output (requires `--features kafka`) |


## Default Templates

`templates/` directory includes Apache combined, Nginx combined, and Syslog (RFC 3164)
format templates ready for use:

```bash
loggen generate --templates ./templates/ --count 100
```

## CLI Overview

```bash
# Generate
loggen generate [-n COUNT] [-l LEVEL] [-m MESSAGE] [-o FILE]
               [--templates PATH] [--var KEY=VALUE]
               [--validate] [--progress] [--no-progress] [--threads N]

# HTTP output
loggen http --url <URL> [-n COUNT]

# Kafka output
loggen kafka [-n COUNT]

# Shell completions
loggen completions <bash|zsh|fish|powershell|elvish>
```

## Building

```bash
cargo build                    # Standard build
cargo build --features kafka   # With Kafka support (requires librdkafka)
cargo test                     # Run all tests
```

## Documentation

- `docs/configuration-reference.md` — All config fields with types, defaults, and constraints
- `docs/template-guide.md` — Tera template syntax, built-in vars, filters, randomization
- `docs/cli-cheatsheet.md` — CLI flags, common examples
