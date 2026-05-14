# loggen-rs

Early-stage Rust log generator. Binary + library crate (`src/main.rs`, `src/lib.rs`).

## Commands

```sh
cargo build                          # build
cargo test                           # all tests (unit + integration)
cargo test --lib                     # unit tests only
cargo test --test mod                # integration tests only (test binary is "mod")
cargo run -- generate [--count N]    # run the binary directly
```

## Structure

- `src/main.rs` — CLI entrypoint. `clap::Parser` + `clap::Subcommand` with derive. `--config` is global; `generate` subcommand has `--output`, `--count` (`-n`), `--level`, `--message`. `prog <subcommand> help` also shows help (not just `--help`).
- `src/lib.rs` — Library root. Re-exports `config`, `generator`, `output`. Public API: `Config`, `LogEntry`, `OutputConfig`, `Generator`, `LogWriter`, `read_yaml_file`.
- `src/config.rs` — `Config`, `OutputConfig`, `LogEntry` structs, YAML deserialization, `read_yaml_file`.
- `src/generator.rs` — `Generator` struct that produces `Vec<LogEntry>` from config.
- `src/output.rs` — `LogWriter` trait, `StdoutWriter`, `FileWriter` implementations.
- `tests/mod.rs` — Integration test entry (compiled as the "mod" test target). Delegates to `tests/unit/` submodule.
- `Cargo.lock` — committed (binary crate); `.gitignore` also lists it but don't remove that entry.
- `examples/` — YAML configs (`example.yaml`, `file-output.yaml`, `minimal.yaml`). Used by integration tests as fixtures.
- `Plan.md` — implementation plan with 5 phases. **Only Phase 1 has work started.**

## Dependencies (exactly 3)

`clap` (derive), `serde` (derive), `serde_yaml` — no async, templating, or regex.

## Testing quirks

- Tests create/clean up `test_config.yaml`, `test_config_unit.yaml`, `test_output.log`, `test_new_file.log` in CWD (no `tempfile` crate).
- Test module tree: `tests/mod.rs → unit/mod.rs → test_config.rs`.

## CI (`.github/workflows/rust.yml`)

Runs on push/PR to `main`. Steps: `cargo build --verbose`, `cargo test --verbose`. **No clippy, no rustfmt, no lint checks.** Don't assume formatting or lint tools are enforced.

## State

Very early Phase 1 of `Plan.md`. The `Http` and `Kafka` CLI subcommands are stubs (just `println!`).
