# loggen-rs

Early-stage Rust log generator. Binary + library crate. Phases 1–3 (foundation, template system + randomization, attack patterns) of `Plan.md` are implemented. `Http`/`Kafka` subcommands and Phase 4+ are stubs.

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
- `src/cli.rs` — CLI glue: `load_base_config`, `apply_cli_args`, `create_writer`, `write_entries`, attack spec parsing.
- `src/generator.rs` — `Generator` with Tera template mode, legacy fallback, and attack engine (`AttackCursor`, `AttackEngine`, `render_attack_entry`).
- `src/config.rs` — `Config`, `OutputConfig`, `LogEntry`, `AttackConfig`, `AttackVarConfig`, `ThresholdConfig`, YAML deser.
- `src/output.rs` — `LogWriter` trait, `StdoutWriter`, `FileWriter` (append mode).
- `tests/unit/` — Integration tests wired via `tests/mod.rs` → `tests/unit/mod.rs`.
- `examples/` — 9 YAML config fixtures.
- `templates/` — 3 `.logtpl` files (apache, nginx, syslog).

## Quirks

- `prog <subcommand> help` works via `try_show_completion_help` workaround.
- `--output` flag sets `target: "file"` automatically (`cli.rs:150-155`).
- `FileWriter` uses **append** mode (never truncates).
- `Cargo.lock` is committed despite being in `.gitignore` — do not remove the `.gitignore` entry.
- `Http` and `Kafka` subcommands are stubs (just `println!`).
- Template validation **panics** at `Generator::new()` on unknown variables (both normal and attack templates).
- Output format depends on `template_mode`: bare message vs `[timestamp] [level] message`.
- Built-in template vars: `timestamp`, `level`, `index`, `message`. Auto-random vars: `ip`/`ipv4`, `ipv6`, `user_agent`, `email`, `url`, `port`, `status`, `user`.
- `Generator::generate()` returns `Vec<LogEntry>` (for tests). CLI uses `generate_to_writer()` which streams entries without buffering all in memory.
- Streaming pipeline has 3 normal paths: `write_legacy_stream` (sequential), `write_template_stream` (sequential, stateful random), `write_template_parallel_stream` (rayon parallel batches via `mpsc` channel, only when `random_intensity >= 1.0` and no interleaving attacks).
- Attack streaming uses serial path exclusively (no rayon). Attack interleaving selects next stream via weighted random per entry.
- Attack `common` field freezes specified variables from the first entry for the entire attack run.
- Attack var modes: `random` (default), `cycle` (sequential wrap-around), `weighted` (first values higher probability).
- `--attack` CLI syntax: `name=type:template [:count]` where type is `single`, `multi`, or `threshold`. Multi attacks with same name merge sequences.
- Dependencies: `clap` (derive), `serde`/`serde_yaml`, `tera`, `rand`, `rayon`, `regex`, `chrono`.

## Testing quirks

- Tests exist inline (`#[cfg(test)]` in `src/config.rs`, `src/generator.rs`, `src/output.rs`) and as integration tests in `tests/unit/`.
- Tests write files to CWD (no `tempfile` crate): `test_config*.yaml`, `test_output.log`, `test_new_file.log`, `test_template_mode.log`, `test_cli_gen.log`, `output.log`.
- Integration tests in `tests/unit/test_config.rs` read from `examples/` — must run from repo root.

## CI (`.github/workflows/rust.yml`)

`cargo build --verbose` then `cargo test --verbose` on push/PR to `main`. No clippy, rustfmt, or lint checks.
