use gild::dataset::Dataset;
use gild::testutil::TempDir;
use std::fs;

#[test]
fn test_stratification_weights_and_min_cases() {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();

    fs::create_dir_all(root.join("cases/c1")).unwrap();
    fs::create_dir_all(root.join("cases/c2")).unwrap();
    fs::create_dir_all(root.join("cases/c3")).unwrap();

    fs::write(
        root.join("dataset.toml"),
        r#"
[dataset]
id = "strat-test"
version = "2.0.0"

[strata]
primary = "difficulty"
min_cases_per_stratum = 1

[strata.weights]
critical = 0.50
core = 0.30
hygiene = 0.20

[gates]
overall_min = 0.85
worst_stratum_min = 0.75
"#,
    )
    .unwrap();

    fs::write(
        root.join("cases/c1/case.toml"),
        r#"
id = "c1"
title = "Critical Case"
stratum = "critical"
expect = "passes"
[[checks]]
id = "k1"
check = { kind = "artifact_exists", glob = "result.json" }
"#,
    )
    .unwrap();

    fs::write(
        root.join("cases/c2/case.toml"),
        r#"
id = "c2"
title = "Core Case"
stratum = "core"
expect = "passes"
[[checks]]
id = "k2"
check = { kind = "artifact_exists", glob = "result.json" }
"#,
    )
    .unwrap();

    fs::write(
        root.join("cases/c3/case.toml"),
        r#"
id = "c3"
title = "Hygiene Case"
stratum = "hygiene"
expect = "passes"
[[checks]]
id = "k3"
check = { kind = "artifact_exists", glob = "result.json" }
"#,
    )
    .unwrap();

    let ds = Dataset::load(root).expect("dataset loads");
    let val = ds.validate();
    assert!(val.ok(), "validation errors: {:?}", val.errors);
    assert_eq!(val.coverage.len(), 3);

    let critical_cov = val.coverage.iter().find(|c| c.key == "critical").unwrap();
    assert_eq!(critical_cov.cases, 1);
    assert!((critical_cov.weight - 0.50).abs() < 1e-6);

    let core_cov = val.coverage.iter().find(|c| c.key == "core").unwrap();
    assert_eq!(core_cov.cases, 1);
    assert!((core_cov.weight - 0.30).abs() < 1e-6);

    let hygiene_cov = val.coverage.iter().find(|c| c.key == "hygiene").unwrap();
    assert_eq!(hygiene_cov.cases, 1);
    assert!((hygiene_cov.weight - 0.20).abs() < 1e-6);
}

#[test]
fn test_unrepresented_stratum_fails_validation() {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();

    fs::create_dir_all(root.join("cases/c1")).unwrap();

    fs::write(
        root.join("dataset.toml"),
        r#"
[dataset]
id = "empty-stratum"
version = "1.0.0"

[strata.weights]
critical = 0.80
core = 0.20
"#,
    )
    .unwrap();

    fs::write(
        root.join("cases/c1/case.toml"),
        r#"
id = "c1"
title = "Critical Only"
stratum = "critical"
expect = "passes"
[[checks]]
id = "k1"
check = { kind = "artifact_exists", glob = "result.json" }
"#,
    )
    .unwrap();

    let ds = Dataset::load(root).expect("dataset loads");
    let val = ds.validate();
    assert!(!val.ok());
    assert!(val
        .errors
        .iter()
        .any(|e| e.contains("declared but has no cases")));
}
