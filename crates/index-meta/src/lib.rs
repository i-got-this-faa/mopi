pub mod db;

pub use db::{FileRecord, MetaError, MetaStore};
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
