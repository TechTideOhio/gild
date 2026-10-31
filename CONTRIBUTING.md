# Contributing to gild

Thank you for your interest in improving gild.

## Standards

1. Pure Rust: No C-FFI or raw dynamic library bindings.
2. Zero Warnings: All code must pass `cargo clippy --all-targets -- -D warnings`.
3. Code Formatting: Keep all formatting consistent with `cargo fmt --all -- --check`.
4. Style: No em dashes or en dashes in code, comments, or documentation.
5. Deterministic Tests: Every new feature or bug fix requires deterministic unit or integration tests.

## Development Workflow

```bash
# Check formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --all-targets -- -D warnings

# Run all unit and integration tests
cargo test --all-targets

# Compile release binary
cargo build --release
```
