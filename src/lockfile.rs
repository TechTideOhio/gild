use crate::dataset::{Dataset, DatasetError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const LOCK_FILE: &str = "gild.lock.json";

#[derive(Debug, thiserror::Error)]
pub enum LockfileError {
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("json error at {path}: {source}")]
    Json {
        path: String,
        source: serde_json::Error,
    },
    #[error("dataset error: {0}")]
    Dataset(#[from] DatasetError),
    #[error("lockfile mismatch for dataset id: expected `{expected}`, found `{found}`")]
    MismatchedId { expected: String, found: String },
    #[error("lockfile mismatch for dataset version: expected `{expected}`, found `{found}`")]
    MismatchedVersion { expected: String, found: String },
    #[error("dataset drift detected: {0}")]
    Drift(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaseLock {
    pub id: String,
    pub stratum: String,
    pub case_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_digest: Option<String>,
    pub rubric_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lockfile {
    pub schema: u32,
    pub dataset_id: String,
    pub dataset_version: String,
    pub dataset_digest: String,
    pub policy_digest: String,
    pub cases: BTreeMap<String, CaseLock>,
}

impl Lockfile {
    pub fn generate(dataset: &Dataset) -> Self {
        let mut cases = BTreeMap::new();
        for case in &dataset.cases {
            cases.insert(
                case.id().to_string(),
                CaseLock {
                    id: case.id().to_string(),
                    stratum: case.stratum().to_string(),
                    case_digest: case.case_digest(),
                    prompt_digest: case.prompt_digest(),
                    rubric_digest: case.rubric_digest(),
                },
            );
        }
        Self {
            schema: crate::SCHEMA,
            dataset_id: dataset.policy.dataset.id.clone(),
            dataset_version: dataset.version().to_string(),
            dataset_digest: dataset.digest(),
            policy_digest: dataset.policy_digest(),
            cases,
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, LockfileError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|source| LockfileError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let lock: Self = serde_json::from_str(&content).map_err(|source| LockfileError::Json {
            path: path.display().to_string(),
            source,
        })?;
        Ok(lock)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), LockfileError> {
        let path = path.as_ref();
        let formatted =
            serde_json::to_string_pretty(self).map_err(|source| LockfileError::Json {
                path: path.display().to_string(),
                source,
            })?;
        fs::write(path, format!("{formatted}\n")).map_err(|source| LockfileError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Ok(())
    }

    pub fn verify(&self, dataset: &Dataset) -> Result<VerificationReport, LockfileError> {
        let mut discrepancies = Vec::new();

        if self.dataset_id != dataset.policy.dataset.id {
            return Err(LockfileError::MismatchedId {
                expected: self.dataset_id.clone(),
                found: dataset.policy.dataset.id.clone(),
            });
        }

        if self.dataset_version != dataset.version() {
            return Err(LockfileError::MismatchedVersion {
                expected: self.dataset_version.clone(),
                found: dataset.version().to_string(),
            });
        }

        let current_policy_digest = dataset.policy_digest();
        if self.policy_digest != current_policy_digest {
            discrepancies.push(format!(
                "policy digest changed: expected {}, found {}",
                self.policy_digest, current_policy_digest
            ));
        }

        let current_dataset_digest = dataset.digest();
        if self.dataset_digest != current_dataset_digest {
            discrepancies.push(format!(
                "overall dataset digest changed: expected {}, found {}",
                self.dataset_digest, current_dataset_digest
            ));
        }

        let mut dataset_case_ids = std::collections::BTreeSet::new();
        for case in &dataset.cases {
            dataset_case_ids.insert(case.id());
            match self.cases.get(case.id()) {
                None => {
                    discrepancies.push(format!(
                        "case `{}` present in dataset but missing from lock",
                        case.id()
                    ));
                }
                Some(locked) => {
                    if locked.stratum != case.stratum() {
                        discrepancies.push(format!(
                            "case `{}` stratum changed: expected `{}`, found `{}`",
                            case.id(),
                            locked.stratum,
                            case.stratum()
                        ));
                    }
                    let current_case_digest = case.case_digest();
                    if locked.case_digest != current_case_digest {
                        discrepancies.push(format!(
                            "case `{}` configuration changed: expected {}, found {}",
                            case.id(),
                            locked.case_digest,
                            current_case_digest
                        ));
                    }
                    let current_prompt_digest = case.prompt_digest();
                    if locked.prompt_digest != current_prompt_digest {
                        discrepancies.push(format!(
                            "case `{}` prompt changed: expected {:?}, found {:?}",
                            case.id(),
                            locked.prompt_digest,
                            current_prompt_digest
                        ));
                    }
                    let current_rubric_digest = case.rubric_digest();
                    if locked.rubric_digest != current_rubric_digest {
                        discrepancies.push(format!(
                            "case `{}` rubric changed: expected {}, found {}",
                            case.id(),
                            locked.rubric_digest,
                            current_rubric_digest
                        ));
                    }
                }
            }
        }

        for locked_id in self.cases.keys() {
            if !dataset_case_ids.contains(locked_id.as_str()) {
                discrepancies.push(format!(
                    "case `{locked_id}` in lock but deleted from dataset"
                ));
            }
        }

        if !discrepancies.is_empty() {
            return Err(LockfileError::Drift(discrepancies.join("; ")));
        }

        Ok(VerificationReport {
            dataset_id: self.dataset_id.clone(),
            dataset_version: self.dataset_version.clone(),
            total_cases: self.cases.len(),
            dataset_digest: self.dataset_digest.clone(),
            verified: true,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub dataset_id: String,
    pub dataset_version: String,
    pub total_cases: usize,
    pub dataset_digest: String,
    pub verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn write_sample_dataset(tmp: &TempDir) {
        let root = tmp.path();
        fs::create_dir_all(root.join("cases/demo")).unwrap();
        fs::write(
            root.join("dataset.toml"),
            r#"
[dataset]
id = "sample"
version = "1.0.0"

[strata]
weights = { core = 1.0 }
"#,
        )
        .unwrap();
        fs::write(
            root.join("cases/demo/case.toml"),
            r#"
id = "demo"
title = "Sample Case"
stratum = "core"
expect = "it works"

[[checks]]
id = "check1"
check = { kind = "artifact_exists", glob = "output.txt" }
"#,
        )
        .unwrap();
    }

    #[test]
    fn generates_and_verifies_lockfile() {
        let tmp = TempDir::new().unwrap();
        write_sample_dataset(&tmp);

        let ds = Dataset::load(tmp.path()).unwrap();
        let lock = Lockfile::generate(&ds);
        let lock_path = tmp.path().join(LOCK_FILE);
        lock.save(&lock_path).unwrap();

        let loaded = Lockfile::load(&lock_path).unwrap();
        let report = loaded.verify(&ds).unwrap();
        assert!(report.verified);
        assert_eq!(report.total_cases, 1);
    }

    #[test]
    fn detects_drift_when_case_mutates() {
        let tmp = TempDir::new().unwrap();
        write_sample_dataset(&tmp);

        let ds = Dataset::load(tmp.path()).unwrap();
        let lock = Lockfile::generate(&ds);

        // Mutate case on disk
        fs::write(
            tmp.path().join("cases/demo/case.toml"),
            r#"
id = "demo"
title = "Mutated Title"
stratum = "core"
expect = "mutated expectation"

[[checks]]
id = "check1"
check = { kind = "artifact_exists", glob = "output.txt" }
"#,
        )
        .unwrap();

        let mutated_ds = Dataset::load(tmp.path()).unwrap();
        let err = lock.verify(&mutated_ds).unwrap_err();
        assert!(matches!(err, LockfileError::Drift(_)));
    }
}
