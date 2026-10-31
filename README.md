# gild

[![CI](https://github.com/TechTideOhio/gild/actions/workflows/ci.yml/badge.svg)](https://github.com/TechTideOhio/gild/actions/workflows/ci.yml)
[![Release](https://github.com/TechTideOhio/gild/actions/workflows/release.yml/badge.svg)](https://github.com/TechTideOhio/gild/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/Language-100%25%20Rust-orange.svg)](#)

> **TideOS Eval Kit (Tool 2)**: Author, stratify, and version golden datasets for coding agent evaluations.
>
> Tool 1 ([reval](https://github.com/TechTideOhio/reval)) evaluates and gates runs. Tool 2 (**gild**) defines and locks the ground truth. Tool 3 ([tessel](https://github.com/TechTideOhio/tessel)) generates synthetic edge case inputs.

---

## Why gild

Agent benchmarks fail in production when evaluation suites drift silently. A prompt wording changes in git, an assertion check gets weakened, or easy sanity tests drown out rare catastrophic failures because every test case counts equally.

`gild` solves three problems for agent evaluation:

1. **Explicit Stratification**: Every stratum carries an explicit fractional weight (e.g. 50% critical, 35% core, 15% hygiene). The validator rejects any dataset that under-samples a stratum it claims to measure.
2. **Deterministic Locking**: Prompts, rubrics, and policy files are cryptographically hashed and pinned in `gild.lock.json`. CI gates flag uncommitted edits before a run even starts.
3. **Native Interoperability**: `gild` writes the exact on-disk structure that `reval eval suite` executes without translation layers or format conversions.

---

## Installation

### Precompiled Binaries

Download precompiled release binaries for Linux, macOS, and Windows from [GitHub Releases](https://github.com/TechTideOhio/gild/releases/latest).

### Building From Source

Requires Rust 1.82 or newer:

```bash
git clone https://github.com/TechTideOhio/gild.git
cd gild
cargo build --release
./target/release/gild --version
```

---

## 60-Second Quickstart

```bash
# 1. Initialize a new golden dataset scaffold
gild init --dataset ./evals/golden --id "coding-agents" --version "1.0.0"

# 2. Add an evaluation case
gild add --dataset ./evals/golden \
  --id "loop-cascade-recovery" \
  --stratum "critical" \
  --title "Agent detects and breaks recursive tool call cascades" \
  --expect "Agent terminates runaway recursion within 3 iterations"

# 3. Validate schema, strata weights, and rubrics
gild validate --dataset ./evals/golden

# 4. Lock digests to prevent drift in CI
gild lock --dataset ./evals/golden

# 5. Verify zero drift in CI pipelines
gild check --dataset ./evals/golden
```

---

## Dataset Format

A golden dataset is stored directly on disk in standard version-controlled files:

```text
evals/golden/
├── dataset.toml           # Dataset policy, strata weights, and quality gates
├── gild.lock.json         # Cryptographic digests of all cases, rubrics, and prompts
└── cases/
    ├── sanity/
    │   ├── case.toml      # Case metadata, stratum, and check rubric
    │   └── prompt.md      # Exact prompt fed to the agent
    ├── feature/
    │   ├── case.toml
    │   └── prompt.md
    └── style/
        ├── case.toml
        └── prompt.md
```

### Policy (`dataset.toml`)

```toml
[dataset]
id = "agent-core"
version = "1.0.0"
description = "Production evaluation suite for software engineering agents"

[strata]
primary = "difficulty"
min_cases_per_stratum = 1

[strata.weights]
critical = 0.50
core = 0.35
hygiene = 0.15

[gates]
overall_min = 0.90
worst_stratum_min = 0.80
require_all_cases = true
fail_on_blocker = true
```

### Evaluation Case (`case.toml`)

```toml
id = "sqlite-wal-checkpoint"
title = "Agent correctly handles SQLite WAL checkpoints"
stratum = "critical"
intent = "concurrency"
user_segment = "database"
expect = "Database remains consistent under concurrent read/write transactions"
prompt = "prompt.md"

[[checks]]
id = "db_checkpoint_verified"
weight = 1.0
severity = "blocker"
why = "A corrupted WAL log causes unrecoverable session failure"
check = { kind = "artifact_exists", glob = "checkpoint.log" }

[[checks]]
id = "zero_wal_lock_errors"
weight = 0.5
severity = "graded"
why = "Verifies clean query execution without lock timeout warnings"
check = { kind = "regex_absent", source = "stderr.log", pattern = "database is locked" }
```

---

## Command Reference

| Command | Description |
|---|---|
| `gild init --dataset <DIR>` | Scaffolds a new dataset with policy, strata, and starter cases. |
| `gild validate --dataset <DIR>` | Validates schema, strata weights summing to 1.0, and check syntax. |
| `gild lock --dataset <DIR>` | Computes SHA-256 digests and writes `gild.lock.json`. |
| `gild check --dataset <DIR>` | Verifies on-disk files match `gild.lock.json` with zero drift. |
| `gild stratify --dataset <DIR>` | Reports sampling distribution across configured strata. |
| `gild add --dataset <DIR>` | Scaffolds a new evaluation case and prompt file. |

All commands accept `--json` to produce machine-readable envelopes for automated pipelines.

---

## Machine-Readable JSON Envelope

```json
{
  "status": "success",
  "summary": "Lockfile verified: 4 cases match with zero drift",
  "next_actions": [],
  "artifacts": [
    "evals/golden/gild.lock.json"
  ],
  "data": {
    "dataset_id": "agent-core",
    "dataset_version": "1.0.0",
    "total_cases": 4,
    "dataset_digest": "sha256:7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
    "verified": true
  }
}
```

---

## Exit Codes

- `0`: Success (validation passed, lock verified with zero drift).
- `1`: Quality gate or validation failure (drift detected, missing stratum cases, weights do not sum to 1.0).
- `2`: Input or IO error (dataset directory not found, unreadable files).

---

## TideOS Eval Kit Trilogy

| Tool | Focus | Artifact | Status |
|---|---|---|---|
| **Tool 1: reval** | Execution, quality gates, trace error analysis, sentinel watcher | [TechTideOhio/reval](https://github.com/TechTideOhio/reval) | v0.2.0 |
| **Tool 2: gild** | Golden dataset schema, stratification, and cryptographic locking | [TechTideOhio/gild](https://github.com/TechTideOhio/gild) | v0.1.0 |
| **Tool 3: tessel** | Combinatorial covering arrays, synthetic input generation | [TechTideOhio/tessel](https://github.com/TechTideOhio/tessel) | v0.1.0 |

---

## License

MIT License. See [LICENSE](LICENSE) for details.
