use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("embedding model is not configured")]
    ModelUnavailable,
    #[error("embedding failed: {0}")]
    Generation(String),
}

pub trait EmbeddingProvider: Send + Sync {
    fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, EmbedError>;
    fn embed_chunks(&mut self, chunks: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError>;
}

pub mod provider {
    use super::{EmbedError, EmbeddingProvider};
    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

    pub struct FastEmbedProvider {
        model: TextEmbedding,
    }

    impl FastEmbedProvider {
        pub fn new(config: &mopi_config::EmbeddingConfig) -> Result<Self, EmbedError> {
            if config.backend == "none" {
                return Err(EmbedError::ModelUnavailable);
            }
            let mut options = InitOptions::new(EmbeddingModel::AllMiniLML6V2);
            options.show_download_progress = true;
            if !config.model_path.is_empty() {
                options.cache_dir = std::path::PathBuf::from(&config.model_path);
            }
            let model = TextEmbedding::try_new(options)
                .map_err(|e| EmbedError::Generation(e.to_string()))?;
            Ok(Self { model })
        }
    }

    impl EmbeddingProvider for FastEmbedProvider {
        fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, EmbedError> {
            let mut embeddings = self
                .model
                .embed(vec![query], None)
                .map_err(|e| EmbedError::Generation(e.to_string()))?;
            Ok(embeddings.pop().unwrap_or_default())
        }

        fn embed_chunks(&mut self, chunks: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
            self.model
                .embed(chunks, None)
                .map_err(|e| EmbedError::Generation(e.to_string()))
        }
    }
}
