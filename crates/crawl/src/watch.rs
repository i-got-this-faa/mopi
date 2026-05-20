use crate::CrawlError;
use mopi_config::{AppConfig, PolicyMatcher};
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebouncedEvent, Debouncer, NoCache, new_debouncer};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchSignal {
    RefreshRequested,
}

pub struct MopiWatcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, NoCache>,
}

impl MopiWatcher {
    pub fn new(
        config: AppConfig,
        signal_tx: mpsc::Sender<WatchSignal>,
    ) -> Result<Self, CrawlError> {
        let matcher = config.policy.matcher()?;
        let config = Arc::new(config);
        let matcher = Arc::new(matcher);

        let config_for_closure = Arc::clone(&config);
        let matcher_for_closure = Arc::clone(&matcher);

        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
                if let Ok(events) = result
                    && has_relevant_event(&config_for_closure, &matcher_for_closure, events)
                {
                    let _ = signal_tx.blocking_send(WatchSignal::RefreshRequested);
                }
            },
        )
        .map_err(|e| CrawlError::Metadata {
            path: camino::Utf8PathBuf::from("watcher_init"),
            source: std::io::Error::other(e),
        })?;

        for root in &config.roots {
            debouncer
                .watch(root.path.as_std_path(), RecursiveMode::Recursive)
                .map_err(|e| CrawlError::Metadata {
                    path: root.path.clone(),
                    source: std::io::Error::other(e),
                })?;
        }

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}

fn has_relevant_event(
    config: &AppConfig,
    matcher: &PolicyMatcher,
    events: Vec<DebouncedEvent>,
) -> bool {
    for event in events {
        let is_supported_kind = matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
        );

        if !is_supported_kind {
            continue;
        }

        for path in &event.paths {
            let utf8_path = match camino::Utf8PathBuf::from_path_buf(path.to_path_buf()) {
                Ok(path) => path,
                Err(_) => continue,
            };

            // Find which root this belongs to
            let relative_path = config.roots.iter().find_map(|root| {
                utf8_path
                    .strip_prefix(&root.path)
                    .ok()
            });

            let Some(relative_path) = relative_path else {
                continue;
            };

            if !matcher.allows(relative_path) {
                continue;
            }

            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use mopi_config::RootConfig;
    use notify::{Event, event::CreateKind};
    use tempfile::tempdir;

    #[tokio::test]
    async fn watcher_emits_added_event_on_file_creation() {
        let dir = tempdir().expect("temp dir should exist");
        let root_path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 path");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig {
            path: root_path.clone(),
        });

        let matcher = config.policy.matcher().expect("matcher should build");
        let events = vec![DebouncedEvent {
            event: Event::new(EventKind::Create(CreateKind::File))
                .add_path(root_path.join("test.txt").into_std_path_buf()),
            time: std::time::Instant::now(),
        }];

        assert!(has_relevant_event(&config, &matcher, events));
    }

    #[tokio::test]
    async fn watcher_respects_ignore_hidden() {
        let dir = tempdir().expect("temp dir should exist");
        let root_path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 path");

        let mut config = AppConfig::default();
        config.roots.push(RootConfig {
            path: root_path.clone(),
        });
        config.policy.ignore_hidden = true;

        let hidden_path = root_path.join(".hidden");
        let visible_path = root_path.join("visible.txt");
        let matcher = config.policy.matcher().expect("matcher should build");
        let hidden_only = vec![DebouncedEvent {
            event: Event::new(EventKind::Create(CreateKind::File))
                .add_path(hidden_path.clone().into_std_path_buf()),
            time: std::time::Instant::now(),
        }];
        let visible_only = vec![DebouncedEvent {
            event: Event::new(EventKind::Create(CreateKind::File))
                .add_path(visible_path.clone().into_std_path_buf()),
            time: std::time::Instant::now(),
        }];

        assert!(!has_relevant_event(&config, &matcher, hidden_only));
        assert!(has_relevant_event(&config, &matcher, visible_only));
    }
}
