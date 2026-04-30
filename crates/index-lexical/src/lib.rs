pub mod index;
pub mod schema;

pub use index::{LexicalError, LexicalSearchResult, LexicalStore};
pub use schema::LexicalSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LexicalDocument {
    pub id: String,
    pub title: String,
    pub path: String,
    pub content: String,
}
