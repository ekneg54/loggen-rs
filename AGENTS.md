# loggen-rs

Early-stage Rust log generator. Binary + library crate (`src/main.rs`, `src/lib.rs`).

## Commands

```sh
cargo build              # build
cargo test               # all tests
cargo test --test mod    # integration tests only
```

## Structure

- `src/main.rs` — CLI entrypoint. Uses `clap::Parser` + `clap::Subcommand` with derive.
- `src/lib.rs` — Library root. Defines `Config` struct and `read_yaml_file` helper.
- `tests/` — Integration tests (cargo convention). Currently one test writing/reading `test_config.yaml` on disk (no `tempfile` crate).
- `Cargo.lock` — committed (this is a binary crate — don't remove from gitignore).
- `Plan.md` - the implementation plan for the project.
- `README.md` - the Features of this project.

## Dependencies (only 3)

`clap` (derive), `serde` (derive), `serde_yaml` — no async runtime, templating, or regex yet.

## CI (`.github/workflows/rust.yml`)

Runs on push/PR to `main`. Steps: `cargo build --verbose`, `cargo test --verbose`. No lint/clippy/fmt checks.

## State

Very early Phase 1 of `Plan.md`. Most features (templates, randomization, attack patterns, performance) are not yet implemented. The CLI subcommands (`http`, `kafka`) exist as stubs.
