# Changelog

All notable changes to gild are documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning.

## [0.1.0] - 2026-10-30

### Added
- Golden dataset authoring CLI (`init`, `validate`, `lock`, `check`, `stratify`, `add`).
- Strict schema validation with strata weight summation and sample coverage verification.
- Content-addressed SHA-256 cryptographic locking (`gild.lock.json`) with zero drift detection.
- Structured single envelope JSON output for CI pipelines and agent harnesses.
- Pure Rust 2021 implementation with zero FFI dependencies.
- Multi-platform precompiled release binaries for Linux, macOS, and Windows.
