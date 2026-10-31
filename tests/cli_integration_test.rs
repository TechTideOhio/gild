use gild::dataset::Dataset;
use gild::lockfile::{Lockfile, LOCK_FILE};
use gild::testutil::TempDir;
use std::fs;

#[test]
fn test_dataset_lifecycle_init_validate_lock_check() {
    let tmp = TempDir::new().expect("create temp dir");
    let ds_path = tmp.path().join("golden");

    // 1. Init
    let cli_init = gild::cli::Cli {
        command: gild::cli::Commands::Init {
            dataset: ds_path.clone(),
            id: "lifecycle-test".into(),
            version: "1.0.0".into(),
        },
    };
    gild::cli::run(cli_init).expect("init succeeds");
    assert!(ds_path.join("dataset.toml").is_file());
    assert!(ds_path.join("cases/sanity/case.toml").is_file());
    assert!(ds_path.join("cases/sanity/prompt.md").is_file());

    // 2. Validate
    let ds = Dataset::load(&ds_path).expect("dataset loads");
    let val = ds.validate();
    assert!(val.ok(), "validation failed: {:?}", val.errors);
    assert_eq!(val.errors.len(), 0);

    // 3. Lock
    let lock_path = ds_path.join(LOCK_FILE);
    let lock = Lockfile::generate(&ds);
    lock.save(&lock_path).expect("lock saves");
    assert!(lock_path.is_file());

    // 4. Check
    let loaded_lock = Lockfile::load(&lock_path).expect("lock loads");
    let report = loaded_lock.verify(&ds).expect("lock verifies");
    assert!(report.verified);
    assert_eq!(report.total_cases, 3);

    // 5. Add a case
    let cli_add = gild::cli::Cli {
        command: gild::cli::Commands::Add {
            dataset: ds_path.clone(),
            id: "case-four".into(),
            stratum: "core".into(),
            title: "Fourth Evaluation Case".into(),
            expect: "Fourth case succeeds".into(),
        },
    };
    gild::cli::run(cli_add).expect("add succeeds");
    assert!(ds_path.join("cases/case-four/case.toml").is_file());

    // 6. Check drift on unpinned addition
    let ds_updated = Dataset::load(&ds_path).expect("dataset loads");
    let drift_err = loaded_lock.verify(&ds_updated).unwrap_err();
    assert!(drift_err.to_string().contains("Drift") || drift_err.to_string().contains("drift"));

    // 7. Re-lock and re-check
    let relock = Lockfile::generate(&ds_updated);
    relock.save(&lock_path).expect("relock saves");
    let reloaded = Lockfile::load(&lock_path).expect("reloaded loads");
    let report2 = reloaded.verify(&ds_updated).expect("relock verifies");
    assert_eq!(report2.total_cases, 4);
}

#[test]
fn test_validation_fails_when_strata_weights_do_not_sum_to_one() {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();

    fs::create_dir_all(root.join("cases/c1")).unwrap();
    fs::write(
        root.join("dataset.toml"),
        r#"
[dataset]
id = "bad-weights"
version = "1.0.0"

[strata.weights]
critical = 0.50
core = 0.20
"#,
    )
    .unwrap();

    fs::write(
        root.join("cases/c1/case.toml"),
        r#"
id = "c1"
title = "Case 1"
stratum = "critical"
expect = "it works"

[[checks]]
id = "c1_check"
check = { kind = "artifact_exists", glob = "result.json" }
"#,
    )
    .unwrap();

    let ds = Dataset::load(root).expect("dataset loads");
    let val = ds.validate();
    assert!(!val.ok());
    assert!(val.errors.iter().any(|e| e.contains("sum to 1.0")));
}

#[test]
fn test_validation_detects_missing_prompt_file() {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();

    fs::create_dir_all(root.join("cases/c1")).unwrap();
    fs::write(
        root.join("dataset.toml"),
        r#"
[dataset]
id = "missing-prompt"
version = "1.0.0"

[strata.weights]
critical = 1.0
"#,
    )
    .unwrap();

    fs::write(
        root.join("cases/c1/case.toml"),
        r#"
id = "c1"
title = "Case 1"
stratum = "critical"
expect = "it works"
prompt = "nonexistent_prompt.md"

[[checks]]
id = "c1_check"
check = { kind = "artifact_exists", glob = "result.json" }
"#,
    )
    .unwrap();

    let ds = Dataset::load(root).expect("dataset loads");
    let val = ds.validate();
    assert!(!val.ok());
    assert!(val.errors.iter().any(|e| e.contains("missing")));
}

#[test]
fn test_validation_detects_duplicate_check_id() {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();

    fs::create_dir_all(root.join("cases/c1")).unwrap();
    fs::write(
        root.join("dataset.toml"),
        r#"
[dataset]
id = "dup-checks"
version = "1.0.0"

[strata.weights]
critical = 1.0
"#,
    )
    .unwrap();

    fs::write(
        root.join("cases/c1/case.toml"),
        r#"
id = "c1"
title = "Case 1"
stratum = "critical"
expect = "it works"

[[checks]]
id = "same_id"
check = { kind = "artifact_exists", glob = "out1.txt" }

[[checks]]
id = "same_id"
check = { kind = "artifact_exists", glob = "out2.txt" }
"#,
    )
    .unwrap();

    let ds = Dataset::load(root).expect("dataset loads");
    let val = ds.validate();
    assert!(!val.ok());
    assert!(val.errors.iter().any(|e| e.contains("duplicate check id")));
}
