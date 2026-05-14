# loggen-rs

Early-stage Rust log generator. Binary + library crate. Only Phase 1 of `Plan.md` is implemented.

## Commands

```sh
cargo build                          # build
cargo test                           # all tests (unit + integration)
cargo test --lib                     # inline #[cfg(test)] tests only
cargo test --test mod                # integration tests only (test binary is "mod")
cargo run -- generate [--count N]    # run binary
```

## Structure

- `src/main.rs` — CLI entrypoint. `clap::Parser` + `clap::Subcommand` with derive. Uses `loggen::cli::*`.
- `src/lib.rs` — Library root. Re-exports `cli`, `config`, `generator`, `output`.
- `src/cli.rs` — CLI glue: `load_base_config`, `apply_cli_args`, `create_writer`, `write_entries`.
- `src/config.rs` — `Config`, `OutputConfig`, `LogEntry`, YAML deserialization.
- `src/generator.rs` — `Generator` that produces `Vec<LogEntry>` from `Config`.
- `src/output.rs` — `LogWriter` trait, `StdoutWriter`, `FileWriter` (append mode).
- `tests/unit/` — Integration tests (unusual `tests/unit/` hierarchy): `test_config.rs`, `cli.rs`.
- `examples/` — 3 YAML config fixtures used by integration tests.

## Quirks

- `prog <subcommand> help` works (non-standard `try_show_completion_help` workaround in `main.rs:54-62`).
- `--output` flag sets `target: "file"` automatically (`src/cli.rs:26-28`).
- `FileWriter` uses **append** mode (does not truncate).
- `Cargo.lock` is committed despite being in `.gitignore` — do not remove the `.gitignore` entry.
- `Http` and `Kafka` subcommands are stubs (just `println!`).

## CI (`.github/workflows/rust.yml`)

`cargo build --verbose` then `cargo test --verbose` on push/PR to `main`. No clippy, rustfmt, or lint checks.

## Testing quirks

- Tests exist inline (`#[cfg(test)]` in `src/config.rs`, `src/generator.rs`, `src/output.rs`) and as integration tests (`tests/`).
- Tests write to CWD (no `tempfile` crate): `test_config.yaml`, `test_config_unit.yaml`, `test_output.log`, `test_new_file.log`, `test_cli_gen.log`, `output.log`.

## Review

- ensure all examples work
- ensure all examples are outlined in the README.md Usage section
- ensure all tests run
- ensure that test coverage is 100%
- ensure the code is idiomatic rust
- ensure all code is readable and adhere to clean code principles
- ensure documentation is complete, accurate, and helpful
