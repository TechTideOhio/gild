//! `gild`: author, stratify, and version golden datasets for coding-agent evals.
//!
//! Tool 2 of the TideOS Eval Kit. Tool 1 (`reval`) runs a dataset; `gild` builds
//! and maintains it. The two share one on-disk format, so a dataset authored with
//! `gild` is consumable by `reval` without conversion.
//!
//! Three primitives, matching the eval-kit contract:
//!
//! 1. **A versioned golden dataset**: one directory per case, carrying the input
//!    prompt, an expected output or rubric, and metadata (intent, difficulty, user
//!    segment) used for stratification.
//! 2. **Stratification**: every stratum carries an explicit weight, so rare but
//!    critical cases cannot be drowned out by easy ones. The validator refuses a
//!    dataset that under-samples a stratum it claims to measure.
//! 3. **Versioning alongside prompts**: dataset, policy, and per-case prompt/rubric
//!    digests are pinned in `gild.lock.json`; `validate --frozen` recomputes them
//!    and refuses drifted content.

#![forbid(unsafe_code)]
#![deny(clippy::all)]

pub mod cli;
pub mod dataset;
pub mod digest;
pub mod lockfile;

#[doc(hidden)]
pub mod testutil;

pub const NAME: &str = "gild";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SCHEMA: u32 = 1;
