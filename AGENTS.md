# loggen-rs

Early-stage Rust log generator. Binary + library crate. Phases 1–2 (foundation, template system + randomization) of `Plan.md` are implemented. `Http`/`Kafka` subcommands and Phase 4+ are stubs.

## Commands

```sh
cargo build                          # build
cargo test                           # all tests (unit + integration)
cargo test --lib                     # inline #[cfg(test)] tests only
cargo test --test mod                # integration tests only (test binary is "mod")
loggen generate [--count N]    # run binary
```

## Structure

- `src/lib.rs` — Public API boundary; re-exports all public items from submodules.
- `src/main.rs` — CLI entrypoint. `clap::Parser` + `Subcommand` derive, `prog <subcommand> help` workaround at `main.rs:75-83`.
- `src/cli.rs` — CLI glue: `load_base_config`, `apply_cli_args`, `create_writer`, `write_entries`.
- `src/generator.rs` — `Generator` with Tera template mode and legacy fallback.
- `src/config.rs` — `Config`, `OutputConfig`, `LogEntry`, YAML deser.
- `src/output.rs` — `LogWriter` trait, `StdoutWriter`, `FileWriter` (append mode).
- `tests/unit/` — Integration tests wired via `tests/mod.rs` → `tests/unit/mod.rs`.
- `examples/` — YAML config fixtures.
- `templates/` — 3 `.logtpl` files (apache, nginx, syslog).

## Quirks

- `prog <subcommand> help` works via `try_show_completion_help` workaround.
- `--output` flag sets `target: "file"` automatically (`cli.rs:150-155`).
- `FileWriter` uses **append** mode (never truncates).
- `Cargo.lock` is committed despite being in `.gitignore` — do not remove the `.gitignore` entry.
- `Http` and `Kafka` subcommands are stubs (just `println!`).
- Template validation **panics** at `Generator::new()` on unknown variables.
- Output format depends on `template_mode`: bare message vs `[timestamp] [level] message`.
- Built-in template vars: `timestamp`, `level`, `index`, `message`. Auto-random vars: `ip`/`ipv4`, `ipv6`, `user_agent`, `email`, `url`, `port`, `status`, `user`.
- `Generator::generate()` returns `Vec<LogEntry>` (for tests). CLI uses `generate_to_writer()` which streams entries without buffering all in memory.
- Streaming pipeline has 3 normal paths: `write_legacy_stream` (sequential), `write_template_stream` (sequential, stateful random), `write_template_parallel_stream` (rayon parallel batches via `mpsc` channel, only when `random_intensity >= 1.0`).
- Dependencies: `clap` (derive), `serde`/`serde_yaml`, `tera`, `rand`, `rayon`, `regex`, `chrono`.

## Testing quirks

- Tests exist inline (`#[cfg(test)]` in `src/config.rs`, `src/generator.rs`, `src/output.rs`) and as integration tests in `tests/unit/`.
- Tests write files to CWD (no `tempfile` crate): `test_config*.yaml`, `test_output.log`, `test_new_file.log`, `test_template_mode.log`, `test_cli_gen.log`, `output.log`.
- Integration tests in `tests/unit/test_config.rs` read from `examples/` — must run from repo root.

## CI (`.github/workflows/rust.yml`)

`cargo build --verbose` then `cargo test --verbose` on push/PR to `main`.

## Security (`.github/workflows/security.yml`)

3-job pipeline on push/PR to `main` + weekly Monday 06:00:
- **audit** — `cargo audit` (dependency advisory check via `taiki-e/install-action`).
- **clippy** — `cargo clippy --all-targets -- -D warnings` (deny all lints).
- **build-and-test** — `cargo build --verbose` + `cargo test --verbose`.

## Security audit findings (variable visibility)

All findings below were remediated in a single pass. Fields marked `pub(crate)` are visible within the crate but hidden from external consumers.

| File | Item | Original | Fixed | Risk |
|------|------|----------|-------|------|
| `src/output.rs:16` | `StdoutWriter::template_mode` | `pub` | `pub(crate)` | **Medium** — output format control |
| `src/output.rs:51` | `FileWriter::template_mode` | `pub` | `pub(crate)` | **Medium** — output format control |
| `src/output.rs:123-126` | `BufferedLogWriter` (4 fields) | all `pub` | all `pub(crate)` | **Medium** — buffer bypass |

### API changes
- Added `StdoutWriter::set_template_mode(&mut self, mode: bool)` and `FileWriter::set_template_mode(...)` for external consumers (integration tests, benchmarks).
