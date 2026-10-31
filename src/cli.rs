use crate::dataset::{Coverage, Dataset};
use crate::lockfile::{Lockfile, LockfileError, LOCK_FILE};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "gild",
    about = "TideOS Eval Kit (Tool 2): Golden dataset authoring, stratification, and versioning",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new golden dataset scaffold
    Init {
        /// Target dataset directory
        #[arg(short, long, default_value = "./golden")]
        dataset: PathBuf,

        /// Dataset identifier
        #[arg(long, default_value = "golden-dataset")]
        id: String,

        /// Initial dataset version
        #[arg(long, default_value = "1.0.0")]
        version: String,
    },

    /// Validate dataset schema, strata coverage, and check rubrics
    Validate {
        /// Path to dataset root
        #[arg(short, long, default_value = ".")]
        dataset: PathBuf,

        /// Elevate warnings to errors
        #[arg(long)]
        strict: bool,

        /// Verify against gild.lock.json in addition to schema validation
        #[arg(long)]
        frozen: bool,

        /// Emit structured JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Compute cryptographic digests and pin gild.lock.json
    Lock {
        /// Path to dataset root
        #[arg(short, long, default_value = ".")]
        dataset: PathBuf,

        /// Allow updating lock if dataset version changed
        #[arg(long)]
        replace_previous_version: bool,

        /// Emit structured JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Check on-disk dataset against existing gild.lock.json with zero drift
    Check {
        /// Path to dataset root
        #[arg(short, long, default_value = ".")]
        dataset: PathBuf,

        /// Emit structured JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Report strata distribution, weights, and sample floors
    Stratify {
        /// Path to dataset root
        #[arg(short, long, default_value = ".")]
        dataset: PathBuf,

        /// Emit structured JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Scaffold a new evaluation case inside the dataset
    Add {
        /// Path to dataset root
        #[arg(short, long, default_value = ".")]
        dataset: PathBuf,

        /// Case identifier (alphanumeric and dashes)
        #[arg(long)]
        id: String,

        /// Target stratum key
        #[arg(long)]
        stratum: String,

        /// Human-readable title
        #[arg(long)]
        title: String,

        /// Expected behavior description
        #[arg(long)]
        expect: String,
    },
}

#[derive(Debug, Serialize)]
pub struct JsonEnvelope<T: Serialize> {
    pub status: String,
    pub summary: String,
    pub next_actions: Vec<String>,
    pub artifacts: Vec<String>,
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct ValidateData {
    pub dataset_id: String,
    pub dataset_version: String,
    pub total_cases: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub coverage: Vec<Coverage>,
    pub frozen_verified: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LockData {
    pub dataset_id: String,
    pub dataset_version: String,
    pub total_cases: usize,
    pub dataset_digest: String,
    pub policy_digest: String,
    pub lockfile_path: String,
}

#[derive(Debug, Serialize)]
pub struct CheckData {
    pub dataset_id: String,
    pub dataset_version: String,
    pub total_cases: usize,
    pub dataset_digest: String,
    pub verified: bool,
}

#[derive(Debug, Serialize)]
pub struct StratifyData {
    pub dataset_id: String,
    pub dataset_version: String,
    pub total_cases: usize,
    pub strata: Vec<Coverage>,
    pub overall_min_gate: f64,
    pub worst_stratum_gate: f64,
}

pub fn run(cli: Cli) -> Result<(), anyhow::Error> {
    match cli.command {
        Commands::Init {
            dataset,
            id,
            version,
        } => handle_init(&dataset, &id, &version),
        Commands::Validate {
            dataset,
            strict,
            frozen,
            json,
        } => handle_validate(&dataset, strict, frozen, json),
        Commands::Lock {
            dataset,
            replace_previous_version,
            json,
        } => handle_lock(&dataset, replace_previous_version, json),
        Commands::Check { dataset, json } => handle_check(&dataset, json),
        Commands::Stratify { dataset, json } => handle_stratify(&dataset, json),
        Commands::Add {
            dataset,
            id,
            stratum,
            title,
            expect,
        } => handle_add(&dataset, &id, &stratum, &title, &expect),
    }
}

fn handle_init(dir: &Path, id: &str, version: &str) -> Result<(), anyhow::Error> {
    if dir.exists() && dir.join("dataset.toml").exists() {
        anyhow::bail!("dataset already exists at {}", dir.display());
    }

    fs::create_dir_all(dir)?;
    fs::create_dir_all(dir.join("cases/sanity"))?;
    fs::create_dir_all(dir.join("cases/feature"))?;
    fs::create_dir_all(dir.join("cases/style"))?;

    let policy_toml = format!(
        r#"[dataset]
id = "{id}"
version = "{version}"
description = "Golden evaluation suite for coding agents"

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
"#
    );
    fs::write(dir.join("dataset.toml"), policy_toml)?;

    let sanity_toml = r#"id = "sanity"
title = "Basic Agent Sanity Verification"
stratum = "critical"
intent = "sanity"
user_segment = "core"
expect = "Agent returns exit code 0 and writes expected artifact"

[[checks]]
id = "artifact_check"
weight = 1.0
severity = "blocker"
why = "Ensures agent produces required output artifact"
check = { kind = "artifact_exists", glob = "result.json" }
"#;
    fs::write(dir.join("cases/sanity/case.toml"), sanity_toml)?;
    fs::write(
        dir.join("cases/sanity/prompt.md"),
        "# Task\nVerify system connectivity and produce result.json.\n",
    )?;

    let feature_toml = r#"id = "feature"
title = "Core Feature Implementation"
stratum = "core"
intent = "feature"
user_segment = "core"
expect = "Agent implements core logic without regressions"

[[checks]]
id = "feature_check"
weight = 1.0
severity = "graded"
why = "Validates feature implementation passes acceptance criteria"
check = { kind = "artifact_exists", glob = "output.txt" }
"#;
    fs::write(dir.join("cases/feature/case.toml"), feature_toml)?;
    fs::write(
        dir.join("cases/feature/prompt.md"),
        "# Task\nImplement core feature logic.\n",
    )?;

    let style_toml = r#"id = "style"
title = "Code Hygiene and Clean Formatting"
stratum = "hygiene"
intent = "hygiene"
user_segment = "dev"
expect = "Agent adheres to formatting and zero lint warnings"

[[checks]]
id = "hygiene_check"
weight = 1.0
severity = "graded"
why = "Enforces code hygiene and lint cleanliness"
check = { kind = "artifact_exists", glob = "report.json" }
"#;
    fs::write(dir.join("cases/style/case.toml"), style_toml)?;
    fs::write(
        dir.join("cases/style/prompt.md"),
        "# Task\nEnsure all code passes formatting and hygiene gates.\n",
    )?;

    println!("Initialized golden dataset at {}", dir.display());
    println!("  - Policy: {}/dataset.toml", dir.display());
    println!("  - Stratified starter cases: sanity, feature, style");
    Ok(())
}

fn handle_validate(
    dir: &Path,
    strict: bool,
    frozen: bool,
    json: bool,
) -> Result<(), anyhow::Error> {
    let ds = match Dataset::load(dir) {
        Ok(d) => d,
        Err(e) => {
            if json {
                emit_json(
                    "error",
                    &format!("Failed to load dataset: {e}"),
                    vec!["Ensure dataset.toml and case directories exist".into()],
                    vec![],
                    serde_json::json!({ "error": e.to_string() }),
                );
            } else {
                eprintln!("Error loading dataset at {}: {e}", dir.display());
            }
            std::process::exit(2);
        }
    };

    let mut val = ds.validate();
    if strict {
        val = val.strict();
    }

    let mut frozen_ok = None;
    if frozen {
        let lock_path = dir.join(LOCK_FILE);
        match Lockfile::load(&lock_path) {
            Ok(lock) => match lock.verify(&ds) {
                Ok(_) => frozen_ok = Some(true),
                Err(e) => {
                    frozen_ok = Some(false);
                    val.errors.push(format!("Frozen lock check failed: {e}"));
                }
            },
            Err(e) => {
                frozen_ok = Some(false);
                val.errors
                    .push(format!("Failed to read {}: {e}", LOCK_FILE));
            }
        }
    }

    let ok = val.ok();
    let status = if ok {
        "success"
    } else if val.errors.is_empty() {
        "warning"
    } else {
        "error"
    };

    let summary = format!(
        "Dataset `{}` v{}: {} cases, {} errors, {} warnings",
        ds.policy.dataset.id,
        ds.version(),
        ds.cases.len(),
        val.errors.len(),
        val.warnings.len()
    );

    let data = ValidateData {
        dataset_id: ds.policy.dataset.id.clone(),
        dataset_version: ds.version().to_string(),
        total_cases: ds.cases.len(),
        errors: val.errors.clone(),
        warnings: val.warnings.clone(),
        coverage: val.coverage.clone(),
        frozen_verified: frozen_ok,
    };

    if json {
        let next_actions = if ok {
            vec!["Proceed with eval suite execution using reval".into()]
        } else {
            vec!["Fix rubric and strata errors before running evals".into()]
        };
        emit_json(status, &summary, next_actions, vec![], data);
    } else {
        println!("{summary}");
        for cov in &val.coverage {
            println!(
                "  [{}] weight: {:.0}%, cases: {}",
                cov.key,
                cov.weight * 100.0,
                cov.cases
            );
        }
        for w in &val.warnings {
            println!("  [WARN] {w}");
        }
        for e in &val.errors {
            eprintln!("  [ERROR] {e}");
        }
    }

    if !ok {
        std::process::exit(1);
    }
    Ok(())
}

fn handle_lock(
    dir: &Path,
    replace_previous_version: bool,
    json: bool,
) -> Result<(), anyhow::Error> {
    let ds = match Dataset::load(dir) {
        Ok(d) => d,
        Err(e) => {
            if json {
                emit_json(
                    "error",
                    &format!("Failed to load dataset: {e}"),
                    vec![],
                    vec![],
                    serde_json::json!({ "error": e.to_string() }),
                );
            } else {
                eprintln!("Error loading dataset: {e}");
            }
            std::process::exit(2);
        }
    };

    let val = ds.validate();
    if !val.ok() {
        let msg = format!("Cannot lock invalid dataset: {} errors", val.errors.len());
        if json {
            emit_json(
                "error",
                &msg,
                vec!["Run `gild validate` to inspect errors".into()],
                vec![],
                val,
            );
        } else {
            eprintln!("{msg}");
            for err in val.errors {
                eprintln!("  - {err}");
            }
        }
        std::process::exit(1);
    }

    let lock_path = dir.join(LOCK_FILE);
    if lock_path.is_file() && !replace_previous_version {
        if let Ok(existing) = Lockfile::load(&lock_path) {
            if existing.dataset_id == ds.policy.dataset.id
                && existing.dataset_version == ds.version()
                && existing.dataset_digest != ds.digest()
            {
                let err = format!(
                    "Refusing to overwrite existing lock with same id/version ({}:{}). Bump version or pass --replace-previous-version",
                    ds.policy.dataset.id, ds.version()
                );
                if json {
                    emit_json(
                        "error",
                        &err,
                        vec!["Bump dataset version in dataset.toml".into()],
                        vec![],
                        (),
                    );
                } else {
                    eprintln!("{err}");
                }
                std::process::exit(1);
            }
        }
    }

    let lock = Lockfile::generate(&ds);
    if let Err(e) = lock.save(&lock_path) {
        eprintln!("Failed to save {}: {e}", lock_path.display());
        std::process::exit(2);
    }

    let summary = format!(
        "Pinned {} cases for `{}` v{} to {}",
        lock.cases.len(),
        lock.dataset_id,
        lock.dataset_version,
        LOCK_FILE
    );

    let data = LockData {
        dataset_id: lock.dataset_id.clone(),
        dataset_version: lock.dataset_version.clone(),
        total_cases: lock.cases.len(),
        dataset_digest: lock.dataset_digest.clone(),
        policy_digest: lock.policy_digest.clone(),
        lockfile_path: lock_path.display().to_string(),
    };

    if json {
        emit_json(
            "success",
            &summary,
            vec!["Commit gild.lock.json to version control".into()],
            vec![lock_path.display().to_string()],
            data,
        );
    } else {
        println!("{summary}");
        println!("  Dataset digest: {}", lock.dataset_digest);
        println!("  Policy digest:  {}", lock.policy_digest);
    }
    Ok(())
}

fn handle_check(dir: &Path, json: bool) -> Result<(), anyhow::Error> {
    let ds = match Dataset::load(dir) {
        Ok(d) => d,
        Err(e) => {
            if json {
                emit_json(
                    "error",
                    &format!("Dataset load error: {e}"),
                    vec![],
                    vec![],
                    (),
                );
            } else {
                eprintln!("Dataset load error: {e}");
            }
            std::process::exit(2);
        }
    };

    let lock_path = dir.join(LOCK_FILE);
    let lock = match Lockfile::load(&lock_path) {
        Ok(l) => l,
        Err(e) => {
            let msg = format!("Failed to read lockfile: {e}");
            if json {
                emit_json(
                    "error",
                    &msg,
                    vec!["Run `gild lock` to generate gild.lock.json".into()],
                    vec![],
                    (),
                );
            } else {
                eprintln!("{msg}");
            }
            std::process::exit(2);
        }
    };

    match lock.verify(&ds) {
        Ok(report) => {
            let summary = format!(
                "Lockfile verified: {} cases match with zero drift",
                report.total_cases
            );
            let data = CheckData {
                dataset_id: report.dataset_id,
                dataset_version: report.dataset_version,
                total_cases: report.total_cases,
                dataset_digest: report.dataset_digest,
                verified: true,
            };
            if json {
                emit_json(
                    "success",
                    &summary,
                    vec![],
                    vec![lock_path.display().to_string()],
                    data,
                );
            } else {
                println!("{summary}");
            }
            Ok(())
        }
        Err(LockfileError::Drift(discrepancy)) => {
            let summary = format!("Drift detected: {discrepancy}");
            if json {
                emit_json(
                    "error",
                    &summary,
                    vec![
                        "Review modifications or run `gild lock --replace-previous-version`".into(),
                    ],
                    vec![],
                    (),
                );
            } else {
                eprintln!("[DRIFT] {summary}");
            }
            std::process::exit(1);
        }
        Err(e) => {
            let summary = format!("Verification failed: {e}");
            if json {
                emit_json("error", &summary, vec![], vec![], ());
            } else {
                eprintln!("{summary}");
            }
            std::process::exit(1);
        }
    }
}

fn handle_stratify(dir: &Path, json: bool) -> Result<(), anyhow::Error> {
    let ds = match Dataset::load(dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Dataset load error: {e}");
            std::process::exit(2);
        }
    };

    let val = ds.validate();
    let data = StratifyData {
        dataset_id: ds.policy.dataset.id.clone(),
        dataset_version: ds.version().to_string(),
        total_cases: ds.cases.len(),
        strata: val.coverage.clone(),
        overall_min_gate: ds.policy.gates.overall_min,
        worst_stratum_gate: ds.policy.gates.worst_stratum_min,
    };

    let summary = format!(
        "Stratification for `{}` v{}: {} strata across {} cases",
        ds.policy.dataset.id,
        ds.version(),
        data.strata.len(),
        data.total_cases
    );

    if json {
        emit_json("success", &summary, vec![], vec![], data);
    } else {
        println!("{summary}");
        println!("Stratum Breakdown:");
        for s in &data.strata {
            let pct = s.weight * 100.0;
            println!(
                "  - {:<12} weight: {:>5.1}%, cases: {:>3}",
                s.key, pct, s.cases
            );
        }
        println!("Gates:");
        println!(
            "  - Overall Min:       {:.1}%",
            data.overall_min_gate * 100.0
        );
        println!(
            "  - Worst Stratum Min: {:.1}%",
            data.worst_stratum_gate * 100.0
        );
    }
    Ok(())
}

fn handle_add(
    dir: &Path,
    id: &str,
    stratum: &str,
    title: &str,
    expect: &str,
) -> Result<(), anyhow::Error> {
    let case_dir = dir.join("cases").join(id);
    if case_dir.exists() {
        anyhow::bail!("Case `{id}` already exists at {}", case_dir.display());
    }

    fs::create_dir_all(&case_dir)?;

    let case_toml = format!(
        r#"id = "{id}"
title = "{title}"
stratum = "{stratum}"
expect = "{expect}"

[[checks]]
id = "primary_check"
weight = 1.0
severity = "graded"
why = "Primary validation check for {id}"
check = {{ kind = "artifact_exists", glob = "result.txt" }}
"#
    );
    fs::write(case_dir.join("case.toml"), case_toml)?;

    let prompt_md = format!("# Goal\n{expect}\n");
    fs::write(case_dir.join("prompt.md"), prompt_md)?;

    println!("Scaffolded new case `{id}` in {}", case_dir.display());
    println!("  - Specification: {}/case.toml", case_dir.display());
    println!("  - Prompt:        {}/prompt.md", case_dir.display());
    Ok(())
}

fn emit_json<T: Serialize>(
    status: &str,
    summary: &str,
    next_actions: Vec<String>,
    artifacts: Vec<String>,
    data: T,
) {
    let env = JsonEnvelope {
        status: status.to_string(),
        summary: summary.to_string(),
        next_actions,
        artifacts,
        data,
    };
    if let Ok(str) = serde_json::to_string_pretty(&env) {
        println!("{str}");
    }
}
