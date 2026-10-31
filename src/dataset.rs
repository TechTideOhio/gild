use crate::digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const POLICY_FILE: &str = "dataset.toml";
pub const CASES_DIR: &str = "cases";
pub const CASE_FILE: &str = "case.toml";

#[derive(Debug, thiserror::Error)]
pub enum DatasetError {
    #[error("no policy file at {0}")]
    NoPolicy(String),
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("toml error at {path}: {source}")]
    Toml {
        path: String,
        source: toml::de::Error,
    },
    #[error("no cases found in {0}")]
    NoCases(String),
    #[error("duplicate case id `{id}` ({path})")]
    DuplicateId { id: String, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetMeta {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrataConfig {
    #[serde(default = "default_primary")]
    pub primary: String,
    pub weights: BTreeMap<String, f64>,
    #[serde(default = "one")]
    pub min_cases_per_stratum: usize,
}

fn default_primary() -> String {
    "stratum".to_string()
}
fn one() -> usize {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gates {
    #[serde(default)]
    pub overall_min: f64,
    #[serde(default)]
    pub worst_stratum_min: f64,
    #[serde(default)]
    pub require_all_cases: bool,
    #[serde(default = "yes")]
    pub fail_on_blocker: bool,
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub dataset: DatasetMeta,
    pub strata: StrataConfig,
    #[serde(default)]
    pub gates: Gates,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSpec {
    pub id: String,
    pub check: Check,
    #[serde(default = "unit_weight")]
    pub weight: f64,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default)]
    pub why: Option<String>,
    #[serde(default)]
    pub when: Option<Condition>,
}
fn unit_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub path: String,
    pub equals: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Graded,
    Blocker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Check {
    ArtifactExists {
        glob: String,
    },
    ArtifactAny {
        globs: Vec<String>,
    },
    PathAbsent {
        glob: String,
    },
    RegexPresent {
        source: String,
        pattern: String,
    },
    RegexAbsent {
        source: String,
        pattern: String,
    },
    TextMinBytes {
        source: String,
        min: u64,
    },
    JsonEquals {
        #[serde(default = "default_json_source")]
        source: String,
        path: String,
        value: serde_json::Value,
    },
    JsonOneOf {
        #[serde(default = "default_json_source")]
        source: String,
        path: String,
        values: Vec<serde_json::Value>,
    },
    JsonPresent {
        #[serde(default = "default_json_source")]
        source: String,
        path: String,
    },
    JsonArrayMinLen {
        #[serde(default = "default_json_source")]
        source: String,
        path: String,
        min: usize,
    },
    JsonNumberMin {
        #[serde(default = "default_json_source")]
        source: String,
        path: String,
        min: f64,
    },
    JsonNumberMax {
        #[serde(default = "default_json_source")]
        source: String,
        path: String,
        max: f64,
    },
}
fn default_json_source() -> String {
    "result.json".to_string()
}

impl Check {
    pub fn kind(&self) -> &'static str {
        match self {
            Check::ArtifactExists { .. } => "artifact_exists",
            Check::ArtifactAny { .. } => "artifact_any",
            Check::PathAbsent { .. } => "path_absent",
            Check::RegexPresent { .. } => "regex_present",
            Check::RegexAbsent { .. } => "regex_absent",
            Check::TextMinBytes { .. } => "text_min_bytes",
            Check::JsonEquals { .. } => "json_equals",
            Check::JsonOneOf { .. } => "json_one_of",
            Check::JsonPresent { .. } => "json_present",
            Check::JsonArrayMinLen { .. } => "json_array_min_len",
            Check::JsonNumberMin { .. } => "json_number_min",
            Check::JsonNumberMax { .. } => "json_number_max",
        }
    }
    pub fn lint(&self) -> Option<String> {
        match self {
            Check::RegexPresent { pattern, .. } | Check::RegexAbsent { pattern, .. } => {
                regex::Regex::new(pattern).err().map(|e| e.to_string())
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseFile {
    pub id: String,
    pub title: String,
    /// Stratum key, must exist in `[strata.weights]`. Alias for reval's `stratum`.
    #[serde(alias = "stratum")]
    pub difficulty: String,
    /// What the case is testing.
    #[serde(default)]
    pub intent: String,
    /// Who cares about this case.
    #[serde(default)]
    pub user_segment: String,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub expect: String,
    /// Prompt file, relative to the case directory.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Expected output / golden artifact, relative to the case directory.
    #[serde(default)]
    pub expected: Option<String>,
    pub checks: Vec<CheckSpec>,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub file: CaseFile,
    pub dir: PathBuf,
}

impl Case {
    pub fn id(&self) -> &str {
        &self.file.id
    }
    pub fn stratum(&self) -> &str {
        &self.file.difficulty
    }
    pub fn prompt_text(&self) -> Result<Option<String>, DatasetError> {
        match &self.file.prompt {
            None => Ok(None),
            Some(name) => fs::read_to_string(self.dir.join(name))
                .map(Some)
                .map_err(|source| DatasetError::Io {
                    path: display(&self.dir.join(name)),
                    source,
                }),
        }
    }
    pub fn case_digest(&self) -> String {
        let value = serde_json::to_value(&self.file).unwrap_or(serde_json::Value::Null);
        digest::tagged("case", digest::canonical_json(&value))
    }
    pub fn prompt_digest(&self) -> Option<String> {
        self.prompt_text()
            .ok()
            .flatten()
            .map(|t| digest::tagged("prompt", digest::normalize_text(&t)))
    }
    pub fn rubric_digest(&self) -> String {
        let value = serde_json::to_value(&self.file.checks).unwrap_or(serde_json::Value::Null);
        digest::tagged("rubric", digest::canonical_json(&value))
    }
}

#[derive(Debug, Clone)]
pub struct Dataset {
    pub root: PathBuf,
    pub policy: Policy,
    pub cases: Vec<Case>,
}

impl Dataset {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, DatasetError> {
        let root = root.as_ref().to_path_buf();
        let policy_path = root.join(POLICY_FILE);
        if !policy_path.is_file() {
            return Err(DatasetError::NoPolicy(display(&policy_path)));
        }
        let raw = fs::read_to_string(&policy_path).map_err(|s| DatasetError::Io {
            path: display(&policy_path),
            source: s,
        })?;
        let policy: Policy = toml::from_str(&raw).map_err(|s| DatasetError::Toml {
            path: display(&policy_path),
            source: s,
        })?;
        let cases_dir = root.join(CASES_DIR);
        let mut dirs: Vec<PathBuf> = Vec::new();
        if cases_dir.is_dir() {
            for entry in fs::read_dir(&cases_dir).map_err(|s| DatasetError::Io {
                path: display(&cases_dir),
                source: s,
            })? {
                let path = entry
                    .map_err(|s| DatasetError::Io {
                        path: display(&cases_dir),
                        source: s,
                    })?
                    .path();
                if path.is_dir() && path.join(CASE_FILE).is_file() {
                    dirs.push(path);
                }
            }
        }
        dirs.sort();
        let mut cases = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for dir in dirs {
            let case_path = dir.join(CASE_FILE);
            let raw = fs::read_to_string(&case_path).map_err(|s| DatasetError::Io {
                path: display(&case_path),
                source: s,
            })?;
            let file: CaseFile = toml::from_str(&raw).map_err(|s| DatasetError::Toml {
                path: display(&case_path),
                source: s,
            })?;
            if !seen.insert(file.id.clone()) {
                return Err(DatasetError::DuplicateId {
                    id: file.id.clone(),
                    path: display(&case_path),
                });
            }
            cases.push(Case { file, dir });
        }
        if cases.is_empty() {
            return Err(DatasetError::NoCases(display(&cases_dir)));
        }
        cases.sort_by(|a, b| a.file.id.cmp(&b.file.id));
        Ok(Self {
            root,
            policy,
            cases,
        })
    }

    pub fn version(&self) -> &str {
        &self.policy.dataset.version
    }
    pub fn digest(&self) -> String {
        let mut parts = vec![
            format!("id={}", self.policy.dataset.id),
            format!("version={}", self.policy.dataset.version),
        ];
        for case in &self.cases {
            parts.push(format!(
                "{}|{}|{}",
                case.id(),
                case.case_digest(),
                case.prompt_digest().unwrap_or_default()
            ));
        }
        digest::tagged("dataset", parts.join("\n"))
    }
    pub fn policy_digest(&self) -> String {
        let value = serde_json::to_value(&self.policy).unwrap_or(serde_json::Value::Null);
        digest::tagged("policy", digest::canonical_json(&value))
    }
    pub fn case(&self, id: &str) -> Option<&Case> {
        self.cases.iter().find(|c| c.id() == id)
    }
    pub fn validate(&self) -> Validation {
        let mut out = Validation::default();
        let weights = &self.policy.strata.weights;
        if self.policy.dataset.id.trim().is_empty() {
            out.errors.push("dataset.id is empty".into());
        }
        if self.policy.dataset.version.trim().is_empty() {
            out.errors.push("dataset.version is empty".into());
        }
        let sum: f64 = weights.values().sum();
        if !weights.is_empty() && (sum - 1.0).abs() > 1e-6 {
            out.errors
                .push(format!("strata.weights must sum to 1.0, got {sum:.4}"));
        }
        for (k, w) in weights {
            if !w.is_finite() || *w <= 0.0 {
                out.errors
                    .push(format!("stratum `{k}` has non-positive weight {w}"));
            }
        }
        for case in &self.cases {
            let id = case.id();
            if case.file.title.trim().is_empty() {
                out.errors
                    .push(format!("case `{id}`: title must not be empty"));
            }
            if case.file.expect.trim().is_empty() {
                out.errors
                    .push(format!("case `{id}`: expect must not be empty"));
            }
            if !weights.contains_key(case.stratum()) {
                out.errors.push(format!(
                    "case `{id}`: stratum `{}` not declared",
                    case.stratum()
                ));
            }
            if case.file.checks.is_empty() {
                out.errors
                    .push(format!("case `{id}`: rubric has no checks"));
            }
            let mut ids = std::collections::BTreeSet::new();
            for c in &case.file.checks {
                if c.id.trim().is_empty() {
                    out.errors
                        .push(format!("case `{id}`: a check has empty id"));
                }
                if !ids.insert(c.id.as_str()) {
                    out.errors
                        .push(format!("case `{id}`: duplicate check id `{}`", c.id));
                }
                if !c.weight.is_finite() || c.weight <= 0.0 {
                    out.errors
                        .push(format!("case `{id}`: check `{}` non-positive weight", c.id));
                }
                if let Some(p) = c.check.lint() {
                    out.errors
                        .push(format!("case `{id}`: check `{}` invalid regex: {p}", c.id));
                }
                if c.why.is_none() {
                    out.warnings
                        .push(format!("case `{id}`: check `{}` has no `why`", c.id));
                }
            }
            if let Some(name) = &case.file.prompt {
                if !case.dir.join(name).is_file() {
                    out.errors
                        .push(format!("case `{id}`: prompt file `{name}` missing"));
                }
            }
        }
        for (key, w) in weights {
            let count = self.cases.iter().filter(|c| c.stratum() == key).count();
            out.coverage.push(Coverage {
                key: key.clone(),
                weight: *w,
                cases: count,
            });
            if count == 0 {
                out.errors
                    .push(format!("stratum `{key}` declared but has no cases"));
            } else if count < self.policy.strata.min_cases_per_stratum {
                out.errors.push(format!(
                    "stratum `{key}` has {count} case(s), min {}",
                    self.policy.strata.min_cases_per_stratum
                ));
            } else if count == 1 && *w >= 0.20 {
                out.warnings.push(format!(
                    "stratum `{key}` is {:.0}% of score but has one case",
                    w * 100.0
                ));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Validation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub coverage: Vec<Coverage>,
}
impl Validation {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
    pub fn strict(self) -> Self {
        let mut s = self;
        s.errors.append(&mut s.warnings);
        s
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Coverage {
    pub key: String,
    pub weight: f64,
    pub cases: usize,
}

fn display(p: &Path) -> String {
    p.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn write(tmp: &TempDir, policy: &str, case: &str) {
        let root = tmp.path();
        fs::create_dir_all(root.join("cases/first")).unwrap();
        fs::write(root.join("dataset.toml"), policy).unwrap();
        fs::write(root.join("cases/first/case.toml"), case).unwrap();
        fs::write(root.join("cases/first/prompt.md"), "do the thing\n").unwrap();
    }

    #[test]
    fn validates_a_clean_dataset() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp,
            r#"
[dataset]
id = "demo"
version = "1.0.0"

[strata]
weights = { critical = 1.0 }
"#,
            r#"
id = "first"
title = "first case"
stratum = "critical"
expect = "it works"

[[checks]]
id = "prompt"
check = { kind = "artifact_exists", glob = "prompt.md" }
"#,
        );
        let ds = Dataset::load(tmp.path()).unwrap();
        let v = ds.validate();
        assert!(v.ok(), "{:?}", v.errors);
    }

    #[test]
    fn rejects_unknown_stratum() {
        let tmp = TempDir::new().unwrap();
        write(
            &tmp,
            r#"
[dataset]
id = "demo"
version = "1.0.0"

[strata]
weights = { critical = 1.0 }
"#,
            r#"
id = "first"
title = "first case"
stratum = "trivial"
expect = "it works"

[[checks]]
id = "prompt"
check = { kind = "artifact_exists", glob = "prompt.md" }
"#,
        );
        let ds = Dataset::load(tmp.path()).unwrap();
        let v = ds.validate();
        assert!(!v.ok());
    }
}
