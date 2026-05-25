pub mod db;

pub use db::{FileRecord, FileRecordOwned, JobRecord, JournalEntry, MetaError, MetaStore};
pub use lss_types::FailureRecord;
use lss_types::DocumentId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexGeneration {
    pub id: DocumentId,
}

impl Default for IndexGeneration {
    fn default() -> Self {
        Self {
            id: DocumentId::new(),
        }
    }
}
