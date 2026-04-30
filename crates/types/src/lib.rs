use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub Uuid);

impl DocumentId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueryId(pub Uuid);

impl QueryId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for QueryId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub raw: String,
    pub terms: String,
    pub limit: usize,
    pub filetype_filters: Vec<String>,
    pub name_filters: Vec<String>,
}

impl SearchQuery {
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            terms: String::new(),
            limit: 20,
            filetype_filters: Vec::new(),
            name_filters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub document_id: DocumentId,
    pub path: Utf8PathBuf,
    pub title: String,
    pub snippet: String,
    pub score: f32,
    pub reasons: Vec<MatchReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchReason {
    Name,
    Path,
    Content,
    Semantic,
    Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonState {
    Starting,
    Ready,
    Indexing,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub state: DaemonState,
    pub indexed_documents: u64,
    pub roots: usize,
}

impl DaemonStatus {
    #[must_use]
    pub fn starting() -> Self {
        Self {
            state: DaemonState::Starting,
            indexed_documents: 0,
            roots: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStats {
    pub protocol_version: u32,
    pub indexed_documents: u64,
    pub configured_roots: usize,
    pub search_requests: u64,
    pub config_reloads: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootSummary {
    pub path: Utf8PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_defaults_to_twenty_results() {
        let query = SearchQuery::new("needle");

        assert_eq!(query.limit, 20);
    }
}
