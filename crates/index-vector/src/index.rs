use hnsw_rs::hnsw::Hnsw;
use hnsw_rs::hnswio::HnswIo;
use hnsw_rs::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VectorError {
    #[error("failed to initialize vector index")]
    InitFailed,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("hnsw_rs error: {0}")]
    Hnsw(String),
}

pub struct VectorIndex<'a> {
    hnsw: Hnsw<'a, f32, DistCosine>,
    deleted_ids: HashSet<usize>,
}

impl<'a> VectorIndex<'a> {
    pub fn new(dimension: usize, max_nb_connection: usize) -> Self {
        let hnsw = Hnsw::new(
            max_nb_connection,
            dimension,
            100, // max_elements
            16,  // ef_construction
            DistCosine,
        );
        Self {
            hnsw,
            deleted_ids: HashSet::new(),
        }
    }

    pub fn load(dir: &Path, basename: &str) -> Result<Self, VectorError> {
        let graph_path = dir.join(format!("{}.hnsw.graph", basename));
        if !graph_path.exists() {
            return Err(VectorError::InitFailed);
        }

        let reloader = Box::new(HnswIo::new(dir, basename));
        let reloader: &'static mut HnswIo = Box::leak(reloader);

        let hnsw = reloader
            .load_hnsw::<f32, DistCosine>()
            .map_err(|e| VectorError::Hnsw(e.to_string()))?;

        let deleted_path = dir.join(format!("{}.deleted", basename));
        let deleted_ids = if deleted_path.exists() {
            let data = fs::read(&deleted_path)?;
            bincode::deserialize(&data)?
        } else {
            HashSet::new()
        };

        Ok(Self { hnsw, deleted_ids })
    }

    pub fn save(&self, dir: &Path, basename: &str) -> Result<(), VectorError> {
        self.hnsw
            .file_dump(dir, basename)
            .map_err(|e| VectorError::Hnsw(e.to_string()))?;

        let deleted_path = dir.join(format!("{}.deleted", basename));
        let data = bincode::serialize(&self.deleted_ids)?;
        fs::write(&deleted_path, data)?;

        Ok(())
    }

    pub fn upsert_chunks(&mut self, chunks: &[(usize, Vec<f32>)]) -> Result<(), VectorError> {
        let mut data = Vec::with_capacity(chunks.len());
        for (id, vec) in chunks {
            self.deleted_ids.remove(id);
            data.push((vec, *id));
        }
        self.hnsw.parallel_insert(&data);
        Ok(())
    }

    pub fn delete_chunks(&mut self, chunk_ids: &[usize]) -> Result<(), VectorError> {
        for id in chunk_ids {
            self.deleted_ids.insert(*id);
        }
        Ok(())
    }

    pub fn search(
        &self,
        query_vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<(usize, f32)>, VectorError> {
        // Request more neighbors in case some are deleted
        let search_k = top_k + self.deleted_ids.len();
        let ef_search = (16 * search_k).max(100);
        let neighbors = self.hnsw.search(query_vector, search_k, ef_search);

        let mut results = Vec::new();
        for n in neighbors {
            if !self.deleted_ids.contains(&n.d_id) {
                results.push((n.d_id, n.distance));
                if results.len() == top_k {
                    break;
                }
            }
        }
        Ok(results)
    }
}
