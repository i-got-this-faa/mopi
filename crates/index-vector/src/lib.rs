use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod index;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorHit {
    pub chunk_id: Uuid,
    pub score: f32,
}
