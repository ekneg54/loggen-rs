# loggen-rs

Early-stage Rust log generator. Binary + library crate. Phase 1 (foundation) and Phase 2 (template system + randomization) of `Plan.md` are implemented. Phase 3+ (attack patterns, advanced streaming) are stubs.

## Commands

```sh
cargo build                          # build
cargo test                           # all tests (unit + integration)
cargo test --lib                     # inline #[cfg(test)] tests only
cargo test --test mod                # integration tests only (test binary is "mod")
cargo run -- generate [--count N]    # run binary
```

## Structure

- `src/main.rs` — CLI entrypoint. `clap::Parser` + `clap::Subcommand` with derive.
- `src/cli.rs` — CLI glue: `load_base_config`, `apply_cli_args`, `create_writer`, `write_entries`.
- `src/config.rs` — `Config`, `OutputConfig`, `LogEntry`, YAML deserialization.
- `src/generator.rs` — `Generator` with template (Tera) and legacy modes.
- `src/output.rs` — `LogWriter` trait, `StdoutWriter`, `FileWriter` (append mode).
- `tests/unit/` — Integration tests in `tests/unit/` subdirectory, wired via `tests/mod.rs` → `tests/unit/mod.rs`.
- `examples/` — 4 YAML config fixtures (`example.yaml`, `file-output.yaml`, `minimal.yaml`, `template-example.yaml`).
- `templates/` — 3 `.logtpl` template files (apache, nginx, syslog).

## Quirks

- `prog <subcommand> help` works (non-standard `try_show_completion_help` workaround in `main.rs:63-71`).
- `--output` flag sets `target: "file"` automatically (`cli.rs:39-44`).
- `FileWriter` uses **append** mode (never truncates).
- `Cargo.lock` is committed despite being in `.gitignore` — do not remove the `.gitignore` entry.
- `Http` and `Kafka` subcommands are stubs (just `println!`).
- Template validation **panics** on unknown variables at `Generator::new()` (`generator.rs:261-263`).
- Output format depends on `template_mode`: bare message vs `[timestamp] [level] message`.
- Built-in template vars: `timestamp`, `level`, `index`, `message`. Auto-random vars: `ip`/`ipv4`, `ipv6`, `user_agent`, `email`, `url`, `port`, `status`, `user`.
- `Generator::generate()` returns `Vec<LogEntry>` (for tests). CLI uses `generate_to_writer()` which streams entries without buffering all in memory.
- Streaming pipeline has 3 paths: `write_legacy_stream` (sequential), `write_template_stream` (sequential, stateful random), `write_template_parallel_stream` (rayon parallel batches via `mpsc` channel, only when `random_intensity >= 1.0`).
- Dependencies: `clap` (derive), `serde`/`serde_yaml`, `tera`, `rand`, `rayon`, `regex`.

## Testing quirks

- Tests exist inline (`#[cfg(test)]` in `src/config.rs`, `src/generator.rs`, `src/output.rs`) and as integration tests in `tests/unit/`.
- Tests write files to CWD (no `tempfile` crate): `test_config.yaml`, `test_config_unit.yaml`, `test_output.log`, `test_new_file.log`, `test_template_mode.log`, `test_cli_gen.log`, `output.log`.
- Integration tests in `tests/unit/test_config.rs` read from `examples/` — must run from repo root.

## CI (`.github/workflows/rust.yml`)

`cargo build --verbose` then `cargo test --verbose` on push/PR to `main`. No clippy, rustfmt, or lint checks.
