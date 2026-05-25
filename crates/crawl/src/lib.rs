pub mod watch;

use camino::Utf8PathBuf;
use lss_config::{AppConfig, ConfigError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::time::UNIX_EPOCH;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlCandidate {
    pub root_id: usize,
    pub observed_path: Utf8PathBuf,
    pub canonical_path: Utf8PathBuf,
    pub alias_paths: Vec<Utf8PathBuf>,
    pub file_name: String,
    pub extension: Option<String>,
    pub file_size: u64,
    pub modified_unix_seconds: u64,
    pub hidden: bool,
    pub is_alias: bool,
}

#[derive(Debug, Error)]
pub enum CrawlError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("encountered a non-UTF-8 path during crawl: {0}")]
    NonUtf8Path(std::path::PathBuf),
    #[error("failed to read file metadata for {path}: {source}")]
    Metadata {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to canonicalize {path}: {source}")]
    Canonicalize {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlOutput {
    pub candidates: Vec<CrawlCandidate>,
    pub skipped_duplicates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlSnapshot {
    pub files: BTreeMap<Utf8PathBuf, CrawlSnapshotEntry>,
}

impl CrawlSnapshot {
    #[must_use]
    pub fn from_output(output: &CrawlOutput) -> Self {
        let files = output
            .candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.canonical_path.clone(),
                    CrawlSnapshotEntry {
                        candidate: candidate.clone(),
                        fingerprint: CrawlFingerprint::from_candidate(candidate),
                    },
                )
            })
            .collect();

        Self { files }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlSnapshotEntry {
    pub candidate: CrawlCandidate,
    pub fingerprint: CrawlFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlFingerprint {
    pub file_size: u64,
    pub modified_unix_seconds: u64,
}

impl CrawlFingerprint {
    #[must_use]
    pub fn from_candidate(candidate: &CrawlCandidate) -> Self {
        Self {
            file_size: candidate.file_size,
            modified_unix_seconds: candidate.modified_unix_seconds,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    pub snapshot: CrawlSnapshot,
    pub events: Vec<ChangeEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub kind: ChangeKind,
    pub canonical_path: Utf8PathBuf,
    pub candidate: Option<CrawlCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

pub fn discover_changes(
    config: &AppConfig,
    previous: Option<&CrawlSnapshot>,
) -> Result<ChangeSet, CrawlError> {
    let output = discover_files(config)?;
    let snapshot = CrawlSnapshot::from_output(&output);
    let events = diff_snapshots(previous, &snapshot);

    Ok(ChangeSet { snapshot, events })
}

#[must_use]
pub fn changed_events(events: &[ChangeEvent]) -> Vec<&ChangeEvent> {
    events
        .iter()
        .filter(|event| event.kind != ChangeKind::Unchanged)
        .collect()
}

#[must_use]
pub fn diff_snapshots(
    previous: Option<&CrawlSnapshot>,
    current: &CrawlSnapshot,
) -> Vec<ChangeEvent> {
    let mut events = Vec::new();

    match previous {
        None => {
            for (canonical_path, entry) in &current.files {
                events.push(ChangeEvent {
                    kind: ChangeKind::Added,
                    canonical_path: canonical_path.clone(),
                    candidate: Some(entry.candidate.clone()),
                });
            }
        }
        Some(previous) => {
            for (canonical_path, entry) in &current.files {
                match previous.files.get(canonical_path) {
                    None => events.push(ChangeEvent {
                        kind: ChangeKind::Added,
                        canonical_path: canonical_path.clone(),
                        candidate: Some(entry.candidate.clone()),
                    }),
                    Some(previous_entry) if previous_entry.fingerprint != entry.fingerprint => {
                        events.push(ChangeEvent {
                            kind: ChangeKind::Modified,
                            canonical_path: canonical_path.clone(),
                            candidate: Some(entry.candidate.clone()),
                        });
                    }
                    Some(_) => events.push(ChangeEvent {
                        kind: ChangeKind::Unchanged,
                        canonical_path: canonical_path.clone(),
                        candidate: Some(entry.candidate.clone()),
                    }),
                }
            }

            for canonical_path in previous.files.keys() {
                if !current.files.contains_key(canonical_path) {
                    events.push(ChangeEvent {
                        kind: ChangeKind::Deleted,
                        canonical_path: canonical_path.clone(),
                        candidate: None,
                    });
                }
            }
        }
    }

    events.sort_by(|left, right| left.canonical_path.cmp(&right.canonical_path));
    events
}

pub fn discover_files(config: &AppConfig) -> Result<CrawlOutput, CrawlError> {
    let matcher = config.policy.matcher()?;
    let mut canonical_to_candidate = std::collections::BTreeMap::new();
    let mut skipped_duplicates = 0;

    for (root_id, root) in config.roots.iter().enumerate() {
        for entry in WalkDir::new(root.path.as_std_path())
            .follow_links(true)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let observed_path = utf8(entry.path())?;
            let relative_path = observed_path
                .strip_prefix(&root.path)
                .unwrap_or(observed_path.as_path());

            if !matcher.allows(relative_path) {
                continue;
            }

            match resolve_candidate(root_id, &root.path, &observed_path) {
                Ok(mut candidate) => {
                    let is_alias = candidate.canonical_path != candidate.observed_path;
                    if let Some(existing) =
                        canonical_to_candidate.get_mut(&candidate.canonical_path)
                    {
                        let existing: &mut CrawlCandidate = existing;
                        if is_alias {
                            existing.alias_paths.push(candidate.observed_path.clone());
                        } else {
                            existing.observed_path = candidate.observed_path.clone();
                        }
                        skipped_duplicates += 1;
                    } else {
                        if is_alias {
                            candidate.alias_paths.push(candidate.observed_path.clone());
                        }
                        canonical_to_candidate.insert(candidate.canonical_path.clone(), candidate);
                    }
                }
                Err(e) => {
                    // Log error but continue walking
                    tracing::warn!("Failed to resolve candidate for {}: {}", observed_path, e);
                }
            }
        }
    }

    let candidates = canonical_to_candidate.into_values().collect();

    Ok(CrawlOutput {
        candidates,
        skipped_duplicates,
    })
}

pub fn resolve_candidate(
    root_id: usize,
    root_path: &Utf8PathBuf,
    observed_path: &Utf8PathBuf,
) -> Result<CrawlCandidate, CrawlError> {
    let canonical_path = fs::canonicalize(observed_path.as_std_path()).map_err(|source| {
        CrawlError::Canonicalize {
            path: observed_path.clone(),
            source,
        }
    })?;
    let canonical_path = utf8(canonical_path)?;
    let metadata =
        fs::metadata(canonical_path.as_std_path()).map_err(|source| CrawlError::Metadata {
            path: canonical_path.clone(),
            source,
        })?;

    let relative_path = observed_path
        .strip_prefix(root_path)
        .unwrap_or(observed_path.as_path());

    let modified_unix_seconds = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs());

    let is_alias = canonical_path != *observed_path;

    Ok(CrawlCandidate {
        root_id,
        file_name: observed_path
            .file_name()
            .map_or_else(String::new, ToOwned::to_owned),
        extension: observed_path.extension().map(ToOwned::to_owned),
        file_size: metadata.len(),
        modified_unix_seconds,
            hidden: lss_config::is_hidden_path(relative_path),
        is_alias,
        observed_path: observed_path.clone(),
        canonical_path,
        alias_paths: Vec::new(),
    })
}

pub fn utf8(path: impl AsRef<std::path::Path>) -> Result<Utf8PathBuf, CrawlError> {
    Utf8PathBuf::from_path_buf(path.as_ref().to_path_buf()).map_err(CrawlError::NonUtf8Path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use lss_config::{PolicyMode, RootConfig};
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    fn write_file(path: &Utf8Path, contents: &str) {
        fs::write(path, contents).expect("test file should be written");
    }

    #[test]
    fn ignores_hidden_paths_by_default() {
        let dir = tempdir().expect("temp dir should exist");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 temp path");
        let hidden_dir = root.join(".private");
        fs::create_dir_all(&hidden_dir).expect("hidden dir should exist");
        write_file(&hidden_dir.join("secret.txt"), "secret");
        write_file(&root.join("visible.txt"), "visible");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path: root });

        let output = discover_files(&config).expect("crawl should succeed");

        assert_eq!(output.candidates.len(), 1);
        assert_eq!(output.candidates[0].file_name, "visible.txt");
    }

    #[test]
    fn whitelist_mode_only_keeps_allowed_paths() {
        let dir = tempdir().expect("temp dir should exist");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 temp path");
        write_file(&root.join("keep.rs"), "fn main() {}\n");
        write_file(&root.join("drop.txt"), "drop\n");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path: root });
        config.policy.mode = PolicyMode::Whitelist;
        config.policy.include = vec![String::from("**/*.rs")];
        config.policy.exclude.clear();

        let output = discover_files(&config).expect("crawl should succeed");

        assert_eq!(output.candidates.len(), 1);
        assert_eq!(output.candidates[0].file_name, "keep.rs");
    }

    #[test]
    fn blacklist_mode_excludes_forbidden_paths() {
        let dir = tempdir().expect("temp dir should exist");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 temp path");
        write_file(&root.join("keep.rs"), "fn main() {}\n");
        write_file(&root.join("drop.log"), "ignore\n");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path: root });
        config.policy.exclude.push(String::from("**/*.log"));

        let output = discover_files(&config).expect("crawl should succeed");

        assert_eq!(output.candidates.len(), 1);
        assert_eq!(output.candidates[0].file_name, "keep.rs");
    }

    #[test]
    fn deduplicates_alias_paths_by_canonical_target() {
        let dir = tempdir().expect("temp dir should exist");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 temp path");
        let canonical = root.join("canonical.txt");
        let alias = root.join("alias.txt");
        write_file(&canonical, "same\n");
        symlink(canonical.as_std_path(), alias.as_std_path()).expect("symlink should be created");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path: root });

        let output = discover_files(&config).expect("crawl should succeed");

        assert_eq!(output.candidates.len(), 1);
        assert_eq!(output.skipped_duplicates, 1);
        assert_eq!(output.candidates[0].alias_paths.len(), 1);
    }

    #[test]
    fn follows_symlinked_directories_without_looping_forever() {
        let dir = tempdir().expect("temp dir should exist");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 temp path");
        let docs = root.join("docs");
        fs::create_dir_all(&docs).expect("docs dir should exist");
        write_file(&docs.join("a.txt"), "a\n");
        symlink(root.as_std_path(), docs.join("loop").as_std_path())
            .expect("loop symlink should exist");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path: root });

        let output = discover_files(&config).expect("crawl should succeed");

        assert_eq!(output.candidates.len(), 1);
    }

    #[test]
    fn initial_watch_tick_marks_all_files_as_added() {
        let dir = tempdir().expect("temp dir should exist");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 temp path");
        write_file(&root.join("first.txt"), "first\n");
        write_file(&root.join("second.txt"), "second\n");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path: root });

        let changes = discover_changes(&config, None).expect("initial watch tick should succeed");

        assert_eq!(changes.events.len(), 2);
        assert!(
            changes
                .events
                .iter()
                .all(|event| event.kind == ChangeKind::Added)
        );
    }

    #[test]
    fn watch_tick_only_emits_changed_files() {
        let dir = tempdir().expect("temp dir should exist");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 temp path");
        let keep = root.join("keep.txt");
        let change = root.join("change.txt");
        write_file(&keep, "keep\n");
        write_file(&change, "old\n");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path: root.clone() });

        let initial = discover_changes(&config, None).expect("initial crawl should succeed");
        std::thread::sleep(std::time::Duration::from_secs(1));
        write_file(&change, "new\n");

        let next = discover_changes(&config, Some(&initial.snapshot))
            .expect("second crawl should succeed");
        let changed = changed_events(&next.events);

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].kind, ChangeKind::Modified);
        assert_eq!(
            changed[0]
                .candidate
                .as_ref()
                .map(|candidate| candidate.file_name.as_str()),
            Some("change.txt")
        );
    }

    #[test]
    fn watch_tick_reports_added_and_deleted_files() {
        let dir = tempdir().expect("temp dir should exist");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 temp path");
        let remove = root.join("remove.txt");
        write_file(&remove, "remove\n");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig { path: root.clone() });

        let initial = discover_changes(&config, None).expect("initial crawl should succeed");
        fs::remove_file(&remove).expect("file should be removed");
        write_file(&root.join("add.txt"), "add\n");

        let next = discover_changes(&config, Some(&initial.snapshot))
            .expect("second crawl should succeed");
        let changed = changed_events(&next.events);

        assert_eq!(changed.len(), 2);
        assert!(changed.iter().any(|event| event.kind == ChangeKind::Added));
        assert!(
            changed
                .iter()
                .any(|event| event.kind == ChangeKind::Deleted)
        );
    }
}
