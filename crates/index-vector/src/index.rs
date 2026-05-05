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
        let mut hnsw = Hnsw::new(
            max_nb_connection,
            dimension,
            1_000_000, // max_elements
            16,        // ef_construction
            DistCosine,
        );
        hnsw.set_searching_mode(true);
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

        let mut hnsw = reloader
            .load_hnsw::<f32, DistCosine>()
            .map_err(|e| VectorError::Hnsw(e.to_string()))?;
        hnsw.set_searching_mode(true);

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
        if self.point_count() == 0 {
            for path in [
                dir.join(format!("{}.hnsw.graph", basename)),
                dir.join(format!("{}.hnsw.data", basename)),
                dir.join(format!("{}.deleted", basename)),
            ] {
                if path.exists() {
                    fs::remove_file(path)?;
                }
            }
            return Ok(());
        }

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
        self.hnsw.set_searching_mode(false);
        self.hnsw.parallel_insert(&data);
        self.hnsw.set_searching_mode(true);
        Ok(())
    }

    #[must_use]
    pub fn point_count(&self) -> usize {
        self.hnsw.get_nb_point()
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn upserted_points_are_searchable_and_persisted() {
        let mut index = VectorIndex::new(3, 16);
        index
            .upsert_chunks(&[
                (1, vec![1.0, 0.0, 0.0]),
                (2, vec![0.0, 1.0, 0.0]),
                (3, vec![0.0, 0.0, 1.0]),
            ])
            .expect("upsert should succeed");

        assert_eq!(index.point_count(), 3);

        let hits = index
            .search(&[1.0, 0.0, 0.0], 1)
            .expect("search should succeed");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 1);

        let dir = tempdir().expect("temp dir should be created");
        index.save(dir.path(), "test").expect("save should succeed");

        let graph = dir.path().join("test.hnsw.graph");
        let data = dir.path().join("test.hnsw.data");
        assert!(graph.exists());
        assert!(data.exists());

        let loaded = VectorIndex::load(dir.path(), "test").expect("load should succeed");
        assert_eq!(loaded.point_count(), 3);
        let reloaded_hits = loaded
            .search(&[1.0, 0.0, 0.0], 1)
            .expect("search after reload should succeed");
        assert_eq!(reloaded_hits.len(), 1);
        assert_eq!(reloaded_hits[0].0, 1);
    }

    #[test]
    fn saving_empty_index_cleans_previous_dump() {
        let dir = tempdir().expect("temp dir should be created");
        let mut populated = VectorIndex::new(3, 16);
        populated
            .upsert_chunks(&[(1, vec![1.0, 0.0, 0.0])])
            .expect("upsert should succeed");
        populated
            .save(dir.path(), "test")
            .expect("save should succeed");

        let empty = VectorIndex::new(3, 16);
        empty
            .save(dir.path(), "test")
            .expect("empty save should succeed");

        assert!(!dir.path().join("test.hnsw.graph").exists());
        assert!(!dir.path().join("test.hnsw.data").exists());
        assert!(!dir.path().join("test.deleted").exists());
    }
}
