use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::de::{self, Deserializer};
use serde::Deserialize;

use crate::error::{AiDocsError, Result};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,

    #[serde(default)]
    pub crates: HashMap<String, CrateDoc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocsSource {
    GitHub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Lockfile,
    LatestDocs,
    Hybrid,
}

impl SyncMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lockfile => "lockfile",
            Self::LatestDocs => "latest_docs",
            Self::Hybrid => "hybrid",
        }
    }
}

impl<'de> Deserialize<'de> for SyncMode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "lockfile" => Ok(Self::Lockfile),
            "latest_docs" | "latest-docs" => Ok(Self::LatestDocs),
            "hybrid" => Ok(Self::Hybrid),
            _ => Err(de::Error::custom(format!(
                "settings.sync_mode must be \"lockfile\", \"latest_docs\", or \"hybrid\", got: {value}"
            ))),
        }
    }
}

impl<'de> Deserialize<'de> for DocsSource {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "github" => Ok(Self::GitHub),
            _ => Err(de::Error::custom(format!(
                "settings.docs_source must be \"github\", got: {value}"
            ))),
        }
    }
}

const fn default_docs_source() -> DocsSource {
    DocsSource::GitHub
}

const fn default_sync_mode() -> SyncMode {
    SyncMode::Lockfile
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    #[serde(default = "default_output_dir")]
    pub output_dir: PathBuf,

    #[serde(default = "default_max_file_size_kb")]
    pub max_file_size_kb: usize,

    #[serde(default = "default_true")]
    pub prune: bool,

    #[serde(default = "default_sync_concurrency")]
    pub sync_concurrency: usize,

    #[serde(default = "default_docs_source")]
    pub docs_source: DocsSource,

    #[serde(default = "default_sync_mode")]
    pub sync_mode: SyncMode,

    #[serde(default = "default_latest_ttl_hours")]
    pub latest_ttl_hours: usize,

    #[serde(default = "default_true")]
    pub docsrs_single_page: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CrateDoc {
    /// New format: explicit repository in crate section.
    pub repo: Option<String>,
    /// Optional subpath for monorepos (used for defaults only).
    pub subpath: Option<String>,
    /// Optional explicit file list.
    pub files: Option<Vec<String>>,

    /// Legacy format compatibility.
    pub sources: Option<Vec<Source>>,

    #[serde(default)]
    pub ai_notes: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Source {
    GitHub {
        repo: String,
        #[serde(default)]
        files: Vec<String>,
    },
    DocsRs,
}

impl CrateDoc {
    pub fn github_repo(&self) -> Option<&str> {
        if let Some(repo) = self.repo.as_deref() {
            return Some(repo);
        }

        self.sources.as_ref().and_then(|sources| {
            sources.iter().find_map(|s| match s {
                Source::GitHub { repo, .. } => Some(repo.as_str()),
                Source::DocsRs => None,
            })
        })
    }

    pub fn effective_files(&self) -> Option<Vec<String>> {
        if let Some(files) = &self.files {
            return Some(files.clone());
        }

        self.sources.as_ref().and_then(|sources| {
            sources.iter().find_map(|s| match s {
                Source::GitHub { files, .. } if !files.is_empty() => Some(files.clone()),
                _ => None,
            })
        })
    }

    pub fn config_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();

        // Hash stable fields
        if let Some(repo) = &self.repo {
            hasher.update(b"repo:");
            hasher.update(repo.as_bytes());
        }
        if let Some(subpath) = &self.subpath {
            hasher.update(b"subpath:");
            hasher.update(subpath.as_bytes());
        }
        if let Some(files) = &self.files {
            hasher.update(b"files:");
            for f in files {
                hasher.update(f.as_bytes());
                hasher.update(b",");
            }
        }
        hasher.update(b"notes:");
        hasher.update(self.ai_notes.as_bytes());

        // Legacy sources fallback
        if let Some(sources) = &self.sources {
            hasher.update(b"sources:");
            // Simplified hash for legacy
            let debug_repr = format!("{:?}", sources);
            hasher.update(debug_repr.as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("fdocs")
}

const fn default_max_file_size_kb() -> usize {
    200
}

const fn default_true() -> bool {
    true
}

const fn default_sync_concurrency() -> usize {
    8
}

const fn default_latest_ttl_hours() -> usize {
    24
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            max_file_size_kb: default_max_file_size_kb(),
            prune: default_true(),
            sync_concurrency: default_sync_concurrency(),
            docs_source: default_docs_source(),
            sync_mode: default_sync_mode(),
            latest_ttl_hours: default_latest_ttl_hours(),
            docsrs_single_page: default_true(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(AiDocsError::ConfigNotFound(path.to_path_buf()));
        }

        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.settings.sync_concurrency == 0 {
            return Err(AiDocsError::InvalidConfig(
                "settings.sync_concurrency must be greater than 0".to_string(),
            ));
        }

        if self.settings.sync_concurrency > 50 {
            return Err(AiDocsError::InvalidConfig(
                "settings.sync_concurrency must not exceed 50 to avoid rate limiting".to_string(),
            ));
        }

        if self.settings.max_file_size_kb == 0 {
            return Err(AiDocsError::InvalidConfig(
                "settings.max_file_size_kb must be greater than 0".to_string(),
            ));
        }

        if self.settings.latest_ttl_hours == 0 {
            return Err(AiDocsError::InvalidConfig(
                "settings.latest_ttl_hours must be greater than 0".to_string(),
            ));
        }

        if !self.settings.docsrs_single_page {
            return Err(AiDocsError::InvalidConfig(
                "settings.docsrs_single_page=false is not supported yet; use true".to_string(),
            ));
        }

        if require_github_repo {
            for (crate_name, crate_cfg) in &self.crates {
                if crate_cfg.github_repo().is_none() {
                    return Err(AiDocsError::InvalidConfig(format!(
                        "crate '{crate_name}' must define `repo` or legacy `sources` with GitHub for lockfile mode"
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{Config, SyncMode};

    #[test]
    fn example_config_parses_with_config_load() {
        let path = Path::new("examples/ai-docs.toml");
        let config = Config::load(path).expect("example config must parse");

        assert!(config.crates.contains_key("serde"));
        assert!(config.crates.contains_key("sqlx"));
    }

    #[test]
    fn settings_sync_mode_defaults_to_lockfile() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-default-sync-mode-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
output_dir = "fdocs/rust"

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let config = Config::load(&path).expect("config should parse");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert_eq!(config.settings.sync_mode, SyncMode::Lockfile);
        assert_eq!(config.settings.latest_ttl_hours, 24);
        assert!(config.settings.docsrs_single_page);
    }

    #[test]
    fn settings_sync_mode_accepts_latest_docs_aliases() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-latest-sync-mode-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
sync_mode = "latest-docs"

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let config = Config::load(&path).expect("config should parse");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert_eq!(config.settings.sync_mode, SyncMode::LatestDocs);
    }

    #[test]
    fn settings_sync_mode_accepts_hybrid() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-hybrid-sync-mode-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
sync_mode = "hybrid"

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let config = Config::load(&path).expect("config should parse");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert_eq!(config.settings.sync_mode, SyncMode::Hybrid);
    }

    #[test]
    fn config_with_zero_max_file_size_fails_validation() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ai-fdocs-invalid-max-file-size-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
max_file_size_kb = 0

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("zero max_file_size_kb must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err
            .to_string()
            .contains("settings.max_file_size_kb must be greater than 0"));
    }

    #[test]
    fn config_with_non_integer_max_file_size_fails_parse() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ai-fdocs-invalid-max-file-size-float-{suffix}.toml"
        ));

        fs::write(
            &path,
            r#"[settings]
max_file_size_kb = 1.5

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("non-integer max_file_size_kb must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err.to_string().contains("max_file_size_kb"));
    }

    #[test]
    fn config_with_non_numeric_max_file_size_fails_parse() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ai-fdocs-invalid-max-file-size-bool-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
max_file_size_kb = true

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("non-numeric max_file_size_kb must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err.to_string().contains("max_file_size_kb"));
    }

    #[test]
    fn config_with_zero_sync_concurrency_fails_validation() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ai-fdocs-invalid-sync-concurrency-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
sync_concurrency = 0

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("zero sync_concurrency must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err
            .to_string()
            .contains("settings.sync_concurrency must be greater than 0"));
    }

    #[test]
    fn config_with_invalid_docs_source_fails_parse() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-invalid-docs-source-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
docs_source = "npm_tarball"

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("invalid docs_source must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err
            .to_string()
            .contains("settings.docs_source must be \"github\", got: npm_tarball"));
    }

    #[test]
    fn config_without_docs_source_uses_github_default() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-default-docs-source-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
sync_concurrency = 2

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let cfg = Config::load(&path).expect("config without docs_source should parse");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(matches!(
            cfg.settings.docs_source,
            super::DocsSource::GitHub
        ));
    }
    #[test]
    fn config_without_repo_or_sources_fails_validation_in_lockfile_mode() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-invalid-{suffix}.toml"));

        fs::write(&path, "[crates.serde]\nai_notes = \"x\"\n")
            .expect("must write temporary config");

        let err = Config::load(&path).expect_err("config without repo/sources must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err
            .to_string()
            .contains("must define `repo` or legacy `sources` with GitHub for lockfile mode"));
    }

    #[test]
    fn config_without_repo_or_sources_is_allowed_in_latest_docs_mode() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-latest-no-repo-{suffix}.toml"));

        fs::write(
            &path,
            "[settings]\nsync_mode = \"latest_docs\"\n\n[crates.serde]\nai_notes = \"x\"\n",
        )
        .expect("must write temporary config");

        let cfg = Config::load(&path).expect("latest_docs config without repo should parse");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert_eq!(cfg.settings.sync_mode, SyncMode::LatestDocs);
    }

    #[test]
    fn config_with_zero_latest_ttl_hours_fails_validation() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ai-fdocs-invalid-latest-ttl-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
latest_ttl_hours = 0

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("zero latest_ttl_hours must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err
            .to_string()
            .contains("settings.latest_ttl_hours must be greater than 0"));
    }

    #[test]
    fn config_with_docsrs_single_page_false_fails_validation() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ai-fdocs-invalid-docsrs-single-page-{suffix}.toml"));

        fs::write(
            &path,
            r#"[settings]
docsrs_single_page = false

[crates.serde]
repo = "serde-rs/serde"
"#,
        )
        .expect("must write temporary config");

        let err = Config::load(&path).expect_err("docsrs_single_page=false must fail");
        fs::remove_file(&path).expect("must cleanup temporary config");

        assert!(err
            .to_string()
            .contains("settings.docsrs_single_page=false is not supported yet; use true"));
    }
}
