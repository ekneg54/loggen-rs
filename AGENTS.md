# loggen-rs

Early-stage Rust log generator. Binary + library crate (`src/main.rs`, `src/lib.rs`).

## Commands

```sh
cargo build              # build
cargo test               # all tests (unit + integration)
cargo test --test mod    # integration tests only (test binary is "mod")
```

## Structure

- `src/main.rs` — CLI entrypoint. `clap::Parser` + `clap::Subcommand` with derive.
- `src/lib.rs` — Library root. Exports `Config` struct and `read_yaml_file`.
- `tests/mod.rs` — Integration test entry (compiled as the "mod" test target). Delegates to `tests/unit/` submodule.
- `Cargo.lock` — committed (binary crate); `.gitignore` also lists it but don't remove that entry.
- `Plan.md` — implementation plan with 5 phases. **Only Phase 1 has work started.**

## Dependencies (exactly 3)

`clap` (derive), `serde` (derive), `serde_yaml` — no async, templating, or regex.

## Testing quirks

- Tests create/clean up `test_config.yaml` in CWD (no `tempfile` crate).
- Test module tree: `tests/mod.rs → unit/mod.rs → test_config.rs`.

## CI (`.github/workflows/rust.yml`)

Runs on push/PR to `main`. Steps: `cargo build --verbose`, `cargo test --verbose`. **No clippy, no rustfmt, no lint checks.** Don't assume formatting or lint tools are enforced.

## State

Very early Phase 1 of `Plan.md`. The `Http` and `Kafka` CLI subcommands are stubs (just `println!`).
