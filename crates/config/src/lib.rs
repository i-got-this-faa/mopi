use camino::{Utf8Path, Utf8PathBuf};
use directories::BaseDirs;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use thiserror::Error;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MopiPaths {
    pub config_dir: Utf8PathBuf,
    pub config_file: Utf8PathBuf,
    pub data_dir: Utf8PathBuf,
    pub cache_dir: Utf8PathBuf,
    pub runtime_dir: Utf8PathBuf,
}

impl MopiPaths {
    pub fn discover() -> Result<Self, ConfigError> {
        let base_dirs = BaseDirs::new().ok_or(ConfigError::MissingBaseDirs)?;
        let home = Utf8PathBuf::from_path_buf(base_dirs.home_dir().to_path_buf())
            .map_err(|_| ConfigError::NonUtf8Path)?;

        let config_root = std::env::var("XDG_CONFIG_HOME")
            .map(Utf8PathBuf::from)
            .unwrap_or_else(|_| home.join(".config"));
        let data_root = std::env::var("XDG_DATA_HOME")
            .map(Utf8PathBuf::from)
            .unwrap_or_else(|_| home.join(".local/share"));
        let cache_root = std::env::var("XDG_CACHE_HOME")
            .map(Utf8PathBuf::from)
            .unwrap_or_else(|_| home.join(".cache"));
        let runtime_root = std::env::var("XDG_RUNTIME_DIR")
            .map(Utf8PathBuf::from)
            .unwrap_or_else(|_| cache_root.join("runtime"));

        let config_dir = config_root.join("mopi");
        let data_dir = data_root.join("mopi");
        let cache_dir = cache_root.join("mopi");
        let runtime_dir = runtime_root.join("mopi");

        Ok(Self {
            config_file: config_dir.join("config.toml"),
            config_dir,
            data_dir,
            cache_dir,
            runtime_dir,
        })
    }

    #[must_use]
    pub fn socket_file(&self) -> Utf8PathBuf {
        self.runtime_dir.join("mopid.sock")
    }

    pub fn ensure_layout(&self) -> Result<(), ConfigError> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.cache_dir,
            &self.runtime_dir,
        ] {
            fs::create_dir_all(dir)?;
        }

        fs::set_permissions(&self.runtime_dir, fs::Permissions::from_mode(0o700))?;
        Ok(())
    }
}

pub fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .try_init();
}

impl AppConfig {
    pub fn load_or_default(paths: &MopiPaths) -> Result<Self, ConfigError> {
        if paths.config_file.exists() {
            let raw = fs::read_to_string(&paths.config_file)?;
            let config = toml::from_str(&raw)?;
            Self::validate(&config)?;
            Ok(config)
        } else {
            let config = Self::default();
            Self::validate(&config)?;
            Ok(config)
        }
    }

    pub fn write_default(paths: &MopiPaths) -> Result<(), ConfigError> {
        paths.ensure_layout()?;
        let config = Self::default_template();
        fs::write(&paths.config_file, config)?;
        Ok(())
    }

    #[must_use]
    pub fn default_template() -> String {
        String::from(
            "# Global mopi configuration\n\
             # Add one or more searchable roots.\n\
             # [[roots]]\n\
             # path = \"/home/you/Documents\"\n\n\
             [policy]\n\
             ignore_hidden = true\n\
             mode = \"blacklist\"\n\
             symlink_mode = \"follow\"\n\
             include = []\n\
             exclude = [\"**/.git/**\"]\n\n\
             [indexing]\n\
             crawl_concurrency = 8\n\
             extraction_concurrency = 4\n\
             embedding_concurrency = 2\n\
             max_in_flight_jobs = 256\n\n\
             [extraction]\n\
             max_file_bytes = 33554432\n\
             max_pdf_pages = 200\n\
             max_extracted_chars = 250000\n\
             timeout_seconds = 15\n\
             enable_plain_text = true\n\
             enable_configs = true\n\
             enable_docx = true\n\
             enable_odt = true\n\
             enable_pdf = true\n\n\
             [embedding]\n\
             model_path = \"\"\n\
             backend = \"auto\"\n\
             query_batch_size = 1\n\
             indexing_batch_size = 32\n\
             strict_startup = false\n\n\
             [ranking]\n\
             content_weight = 1.0\n\
             semantic_weight = 0.8\n\
             filename_weight = 0.45\n\
             path_weight = 0.2\n\
             metadata_weight = 0.35\n\n\
             [daemon]\n\
             socket_override = \"\"\n\
             max_connections = 64\n\n\
             [logging]\n\
             level = \"info\"\n",
        )
    }

    pub fn validate(config: &Self) -> Result<(), ConfigError> {
        validate_roots(&config.roots)?;
        validate_pattern_lists(&config.policy.include, &config.policy.exclude)?;
        validate_pattern_set(&config.policy.include)?;
        validate_pattern_set(&config.policy.exclude)?;
        validate_indexing(&config.indexing)?;
        validate_extraction(&config.extraction)?;
        validate_embedding(&config.embedding)?;
        validate_ranking(&config.ranking)?;
        validate_daemon(&config.daemon)?;
        validate_logging(&config.logging)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RootConfig {
    pub path: Utf8PathBuf,
}

impl Default for RootConfig {
    fn default() -> Self {
        Self {
            path: Utf8PathBuf::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PolicyConfig {
    pub ignore_hidden: bool,
    pub mode: PolicyMode,
    pub symlink_mode: SymlinkMode,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            ignore_hidden: true,
            mode: PolicyMode::Blacklist,
            symlink_mode: SymlinkMode::Follow,
            include: Vec::new(),
            exclude: vec![String::from("**/.git/**")],
        }
    }
}

impl PolicyConfig {
    pub fn include_set(&self) -> Result<GlobSet, ConfigError> {
        compile_glob_set(&self.include)
    }

    pub fn exclude_set(&self) -> Result<GlobSet, ConfigError> {
        compile_glob_set(&self.exclude)
    }

    pub fn matcher(&self) -> Result<PolicyMatcher, ConfigError> {
        Ok(PolicyMatcher {
            ignore_hidden: self.ignore_hidden,
            mode: self.mode.clone(),
            include: self.include_set()?,
            exclude: self.exclude_set()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    Whitelist,
    Blacklist,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymlinkMode {
    Follow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct IndexingConfig {
    pub crawl_concurrency: usize,
    pub extraction_concurrency: usize,
    pub embedding_concurrency: usize,
    pub max_in_flight_jobs: usize,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            crawl_concurrency: 8,
            extraction_concurrency: 4,
            embedding_concurrency: 2,
            max_in_flight_jobs: 256,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ExtractionConfig {
    pub max_file_bytes: u64,
    pub max_pdf_pages: u32,
    pub max_extracted_chars: usize,
    pub timeout_seconds: u64,
    pub enable_plain_text: bool,
    pub enable_configs: bool,
    pub enable_docx: bool,
    pub enable_odt: bool,
    pub enable_pdf: bool,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 32 * 1024 * 1024,
            max_pdf_pages: 200,
            max_extracted_chars: 250_000,
            timeout_seconds: 15,
            enable_plain_text: true,
            enable_configs: true,
            enable_docx: true,
            enable_odt: true,
            enable_pdf: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct EmbeddingConfig {
    pub model_path: String,
    pub backend: String,
    pub query_batch_size: usize,
    pub indexing_batch_size: usize,
    pub strict_startup: bool,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            backend: String::from("auto"),
            query_batch_size: 1,
            indexing_batch_size: 32,
            strict_startup: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RankingConfig {
    pub content_weight: f32,
    pub semantic_weight: f32,
    pub filename_weight: f32,
    pub path_weight: f32,
    pub metadata_weight: f32,
}

impl Default for RankingConfig {
    fn default() -> Self {
        Self {
            content_weight: 1.0,
            semantic_weight: 0.8,
            filename_weight: 0.45,
            path_weight: 0.2,
            metadata_weight: 0.35,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DaemonConfig {
    pub socket_override: String,
    pub max_connections: usize,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_override: String::new(),
            max_connections: 64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: String::from("info"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub roots: Vec<RootConfig>,
    pub policy: PolicyConfig,
    pub indexing: IndexingConfig,
    pub extraction: ExtractionConfig,
    pub embedding: EmbeddingConfig,
    pub ranking: RankingConfig,
    pub daemon: DaemonConfig,
    pub logging: LoggingConfig,
}

impl DaemonConfig {
    #[must_use]
    pub fn socket_path(&self, paths: &MopiPaths) -> Utf8PathBuf {
        if self.socket_override.is_empty() {
            paths.socket_file()
        } else {
            Utf8PathBuf::from(self.socket_override.clone())
        }
    }
}

#[derive(Debug, Clone)]
pub struct PolicyMatcher {
    ignore_hidden: bool,
    mode: PolicyMode,
    include: GlobSet,
    exclude: GlobSet,
}

impl PolicyMatcher {
    #[must_use]
    pub fn allows(&self, path: &Utf8Path) -> bool {
        if self.ignore_hidden && is_hidden_path(path) {
            return false;
        }

        match self.mode {
            PolicyMode::Whitelist => !self.include.is_empty() && self.include.is_match(path),
            PolicyMode::Blacklist => !self.exclude.is_match(path),
        }
    }
}

#[must_use]
pub fn is_hidden_path(path: &Utf8Path) -> bool {
    path.components().any(|segment| {
        let text = segment.as_str();
        text.starts_with('.') && text.len() > 1
    })
}

fn compile_glob_set(patterns: &[String]) -> Result<GlobSet, ConfigError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            Glob::new(pattern).map_err(|source| ConfigError::InvalidGlob {
                pattern: pattern.clone(),
                source,
            })?,
        );
    }

    builder.build().map_err(ConfigError::GlobSetBuild)
}

fn validate_pattern_set(patterns: &[String]) -> Result<(), ConfigError> {
    let _ = compile_glob_set(patterns)?;
    Ok(())
}

fn validate_roots(roots: &[RootConfig]) -> Result<(), ConfigError> {
    for root in roots {
        if !root.path.exists() {
            return Err(ConfigError::MissingRoot(root.path.clone()));
        }

        if !root.path.is_dir() {
            return Err(ConfigError::UnreadableRoot(root.path.clone()));
        }
    }

    Ok(())
}

fn validate_pattern_lists(include: &[String], exclude: &[String]) -> Result<(), ConfigError> {
    let include_set: HashSet<_> = include.iter().cloned().collect();
    let exclude_set: HashSet<_> = exclude.iter().cloned().collect();

    let mut duplicates = include_set.intersection(&exclude_set);
    if let Some(pattern) = duplicates.next() {
        return Err(ConfigError::ConflictingPattern(pattern.clone()));
    }

    Ok(())
}

fn validate_indexing(config: &IndexingConfig) -> Result<(), ConfigError> {
    validate_positive_usize("indexing.crawl_concurrency", config.crawl_concurrency)?;
    validate_positive_usize(
        "indexing.extraction_concurrency",
        config.extraction_concurrency,
    )?;
    validate_positive_usize(
        "indexing.embedding_concurrency",
        config.embedding_concurrency,
    )?;
    validate_positive_usize("indexing.max_in_flight_jobs", config.max_in_flight_jobs)?;
    Ok(())
}

fn validate_extraction(config: &ExtractionConfig) -> Result<(), ConfigError> {
    validate_positive_u64("extraction.max_file_bytes", config.max_file_bytes)?;
    validate_positive_u32("extraction.max_pdf_pages", config.max_pdf_pages)?;
    validate_positive_usize("extraction.max_extracted_chars", config.max_extracted_chars)?;
    validate_positive_u64("extraction.timeout_seconds", config.timeout_seconds)?;
    Ok(())
}

fn validate_embedding(config: &EmbeddingConfig) -> Result<(), ConfigError> {
    validate_positive_usize("embedding.query_batch_size", config.query_batch_size)?;
    validate_positive_usize("embedding.indexing_batch_size", config.indexing_batch_size)?;

    if config.strict_startup && config.model_path.is_empty() {
        return Err(ConfigError::MissingStrictModelPath);
    }

    if config.strict_startup && !config.model_path.is_empty() {
        let path = Utf8PathBuf::from(config.model_path.clone());
        if !path.exists() {
            return Err(ConfigError::MissingModelPath(path));
        }
    }

    Ok(())
}

fn validate_ranking(config: &RankingConfig) -> Result<(), ConfigError> {
    validate_non_negative_f32("ranking.content_weight", config.content_weight)?;
    validate_non_negative_f32("ranking.semantic_weight", config.semantic_weight)?;
    validate_non_negative_f32("ranking.filename_weight", config.filename_weight)?;
    validate_non_negative_f32("ranking.path_weight", config.path_weight)?;
    validate_non_negative_f32("ranking.metadata_weight", config.metadata_weight)?;
    Ok(())
}

fn validate_daemon(config: &DaemonConfig) -> Result<(), ConfigError> {
    if !config.socket_override.is_empty() && !config.socket_override.starts_with('/') {
        return Err(ConfigError::InvalidSocketOverride(
            config.socket_override.clone(),
        ));
    }

    validate_positive_usize("daemon.max_connections", config.max_connections)?;
    Ok(())
}

fn validate_logging(config: &LoggingConfig) -> Result<(), ConfigError> {
    const ALLOWED: &[&str] = &["trace", "debug", "info", "warn", "error"];
    if ALLOWED.contains(&config.level.as_str()) {
        Ok(())
    } else {
        Err(ConfigError::InvalidLogLevel(config.level.clone()))
    }
}

fn validate_positive_usize(field: &'static str, value: usize) -> Result<(), ConfigError> {
    if value == 0 {
        Err(ConfigError::InvalidNumericSetting {
            field,
            reason: "must be greater than zero",
        })
    } else {
        Ok(())
    }
}

fn validate_positive_u64(field: &'static str, value: u64) -> Result<(), ConfigError> {
    if value == 0 {
        Err(ConfigError::InvalidNumericSetting {
            field,
            reason: "must be greater than zero",
        })
    } else {
        Ok(())
    }
}

fn validate_positive_u32(field: &'static str, value: u32) -> Result<(), ConfigError> {
    if value == 0 {
        Err(ConfigError::InvalidNumericSetting {
            field,
            reason: "must be greater than zero",
        })
    } else {
        Ok(())
    }
}

fn validate_non_negative_f32(field: &'static str, value: f32) -> Result<(), ConfigError> {
    if value.is_sign_negative() {
        Err(ConfigError::InvalidNumericSetting {
            field,
            reason: "must not be negative",
        })
    } else {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not resolve base directories for the current user")]
    MissingBaseDirs,
    #[error("encountered a non-UTF-8 path while building XDG paths")]
    NonUtf8Path,
    #[error("failed to read or write config state: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config file: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize config file: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("invalid glob pattern `{pattern}`: {source}")]
    InvalidGlob {
        pattern: String,
        #[source]
        source: globset::Error,
    },
    #[error("failed to build glob matcher: {0}")]
    GlobSetBuild(globset::Error),
    #[error("configured root does not exist: {0}")]
    MissingRoot(Utf8PathBuf),
    #[error("configured root is not a readable directory: {0}")]
    UnreadableRoot(Utf8PathBuf),
    #[error("pattern appears in both include and exclude lists: {0}")]
    ConflictingPattern(String),
    #[error("invalid numeric setting `{field}`: {reason}")]
    InvalidNumericSetting {
        field: &'static str,
        reason: &'static str,
    },
    #[error("strict embedding startup requires a non-empty model path")]
    MissingStrictModelPath,
    #[error("strict embedding startup model path does not exist: {0}")]
    MissingModelPath(Utf8PathBuf),
    #[error("daemon.socket_override must be an absolute path: {0}")]
    InvalidSocketOverride(String),
    #[error("invalid logging level: {0}")]
    InvalidLogLevel(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_policy_matches_product_decisions() {
        let config = AppConfig::default();

        assert!(config.policy.ignore_hidden);
        assert_eq!(config.policy.mode, PolicyMode::Blacklist);
        assert_eq!(config.policy.symlink_mode, SymlinkMode::Follow);
    }

    #[test]
    fn default_template_mentions_policy_and_indexing_sections() {
        let template = AppConfig::default_template();

        assert!(template.contains("[policy]"));
        assert!(template.contains("[indexing]"));
    }

    #[test]
    fn conflicting_patterns_fail_validation() {
        let mut config = AppConfig::default();
        config.policy.include.push(String::from("**/*.rs"));
        config.policy.exclude.push(String::from("**/*.rs"));

        assert!(matches!(
            AppConfig::validate(&config),
            Err(ConfigError::ConflictingPattern(_))
        ));
    }

    #[test]
    fn generated_template_is_parseable_and_valid() {
        let config: AppConfig = toml::from_str(&AppConfig::default_template())
            .expect("default template should parse into AppConfig");

        AppConfig::validate(&config).expect("default template should validate");
    }

    #[test]
    fn missing_root_fails_validation() {
        let mut config = AppConfig::default();
        config.roots.push(RootConfig {
            path: Utf8PathBuf::from("/definitely/missing/mopi-root"),
        });

        assert!(matches!(
            AppConfig::validate(&config),
            Err(ConfigError::MissingRoot(_))
        ));
    }

    #[test]
    fn strict_startup_requires_real_model_path() {
        let mut config = AppConfig::default();
        config.embedding.strict_startup = true;

        assert!(matches!(
            AppConfig::validate(&config),
            Err(ConfigError::MissingStrictModelPath)
        ));
    }

    #[test]
    fn existing_root_passes_validation() {
        let dir = tempdir().expect("temporary directory should be created");
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .expect("temporary path should be UTF-8");
        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path });

        AppConfig::validate(&config).expect("existing root should validate");
    }
}
