# loggen-rs

Early-stage Rust log generator. Binary + library crate. Phase 1 (§1.1–1.2) of `Plan.md` is implemented; §1.3+ and Phase 2 are stubs.

## Commands

```sh
cargo build                           # std build
cargo build --features kafka          # Kafka support (needs librdkafka / system lib)
cargo test                            # all tests
cargo test --lib                      # inline #[cfg(test)] tests only
cargo test --test mod                 # integration tests only (test binary name is "mod")
cargo bench                           # criterion benchmarks (harness=false)
cargo clippy --all-targets -- -D warnings   # CI-recommended lint check
cargo audit                           # CI security audit
loggen generate [--count N]           # run binary
```

## Key modules

- `src/lib.rs` — re-exports all public items
- `src/main.rs` — `clap::Parser` + `Subcommand` derive; `prog <subcommand> help` workaround at `main.rs:135-143`
- `src/cli.rs` — `load_base_config`, `apply_cli_args`, `create_writer`, `write_entries`
- `src/generator.rs` — `Generator` with Tera template mode and legacy fallback; `generate()` returns `Vec<LogEntry>`, CLI uses streaming `generate_to_writer()`
- `src/config.rs` — `Config`, `OutputConfig`, `LogEntry`, YAML deser via `serde_yaml`
- `src/output.rs` — `LogWriter` trait, `StdoutWriter`, `FileWriter` (append by default, configurable truncate + `rotate_bytes`), `BufferedLogWriter`, `HttpWriter` (full impl with batching/retry), `KafkaWriter` (feature-gated via `--features kafka`), `ProgressReporter`

## Quirks

- `--output` flag sets `target: "file"` automatically (`cli.rs:43-49`)
- `FileWriter` default is **append** mode (never truncates unless `output.append: false`)
- Template validation **panics** at `Generator::new()` on unknown variables
- Streaming has 3 paths: `write_legacy_stream` (sequential), `write_template_stream` (sequential), `write_template_parallel_stream` (rayon via `mpsc`, only when `random_intensity >= 1.0` and no simulation)
- Simulation mode (`sim_delay`, `sim_rotation`) makes streaming infinite (until Ctrl+C via `ctrlc`)
- `ProgressReporter` auto-enables when count >= 100,000 and target != stdout
- Default `config.count` is **1** (not user-friendly but deliberate)
- `config.seed` enables reproducible output; no seed = random
- `Config::has_templates()` is the canonical template-mode check
- `Cargo.lock` is committed despite being in `.gitignore` — keep both
- Container image via `nix build .#container` (Nix flake)
- Release workflow cross-compiles 4 targets + container via Nix (`ghcr.io`)

## Testing

- Inline tests (`#[cfg(test)]`) in `config.rs`, `generator.rs`, `output.rs`
- Integration tests in `tests/unit/` (wired via `tests/mod.rs`)
- **Tests write files to CWD** (no `tempfile` crate): `test_config*.yaml`, `test_output*.log`, `test_new_file.log`, `test_template_mode.log`, `test_cli_gen.log`, `output.log`
- Integration tests at `tests/unit/test_config.rs` read from `examples/` — must run from repo root
- Benchmarks: `benches/benchmarks.rs` via criterion

## Dependencies

`clap` (derive), `clap_complete`, `serde`/`serde_yaml`, `tera`, `rand`, `rayon`, `regex`, `chrono`, `ureq` (HTTP), `serde_json`, `ctrlc`. Optional: `rdkafka` behind `kafka` feature.

## Security history

`StdoutWriter::template_mode`, `FileWriter::template_mode`, and all `BufferedLogWriter` fields are `pub(crate)` (were `pub` in earlier versions). Public setter methods (`set_template_mode`) were added for external consumers.
