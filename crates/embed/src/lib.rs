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
    use huggingface_hub::api::sync::ApiBuilder;
    use ndarray::{Array, Array2, Axis, Ix2, Ix3};
    use ort::{
        inputs,
        session::{Session, builder::GraphOptimizationLevel},
        value::Value,
    };
    use serde_json::Value as JsonValue;
    use std::path::{Path, PathBuf};
    use std::time::Instant;
    use tokenizers::{AddedToken, PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};
    use tracing::info;

    const MODEL_NAME: &str = "AllMiniLML6V2";
    const MODEL_CODE: &str = "Qdrant/all-MiniLM-L6-v2-onnx";
    const MODEL_DIMENSION: usize = 384;
    const MODEL_MAX_LENGTH: usize = 512;
    const DEFAULT_BATCH_SIZE: usize = 256;
    const LOCAL_MODEL_FILES: [&str; 5] = [
        "model.onnx",
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ];

    pub struct FastEmbedProvider {
        tokenizer: Tokenizer,
        session: Session,
        need_token_type_ids: bool,
    }

    struct ModelArtifacts {
        model_path: PathBuf,
        tokenizer_file: Vec<u8>,
        config_file: Vec<u8>,
        special_tokens_map_file: Vec<u8>,
        tokenizer_config_file: Vec<u8>,
    }

    impl FastEmbedProvider {
        pub fn new(config: &lss_config::EmbeddingConfig) -> Result<Self, EmbedError> {
            if config.backend == "none" {
                return Err(EmbedError::ModelUnavailable);
            }

            let init_started_at = Instant::now();
            info!(
                backend = %config.backend,
                configured_model_path = if config.model_path.is_empty() { "<default>" } else { config.model_path.as_str() },
                "embedder init started"
            );
            info!(model = MODEL_NAME, "loading embedder model");

            let artifacts = if let Some(model_dir) = local_model_dir_from_config(config) {
                info!(model_dir = %model_dir.display(), "embedder local model bundle detected");
                load_local_artifacts(&model_dir)?
            } else {
                load_remote_artifacts(config)?
            };

            let tokenizer_started_at = Instant::now();
            let tokenizer = load_tokenizer(&artifacts)?;
            info!(
                elapsed_ms = tokenizer_started_at.elapsed().as_millis(),
                "embedder tokenizer initialized"
            );

            let session_started_at = Instant::now();
            info!(
                model_file = %artifacts.model_path.display(),
                optimization = "Level1",
                intra_threads = 1,
                inter_threads = 1,
                execution_mode = "sequential",
                "embedder ORT session creation started"
            );
            let session = Session::builder()
                .map_err(to_embed_error)?
                .with_optimization_level(GraphOptimizationLevel::Level1)
                .map_err(|e| EmbedError::Generation(e.to_string()))?
                .with_intra_threads(1)
                .map_err(|e| EmbedError::Generation(e.to_string()))?
                .with_inter_threads(1)
                .map_err(|e| EmbedError::Generation(e.to_string()))?
                .with_parallel_execution(false)
                .map_err(|e| EmbedError::Generation(e.to_string()))?
                .with_memory_pattern(false)
                .map_err(|e| EmbedError::Generation(e.to_string()))?
                .commit_from_file(&artifacts.model_path)
                .map_err(to_embed_error)?;
            let need_token_type_ids = session
                .inputs()
                .iter()
                .any(|input| input.name() == "token_type_ids");
            info!(
                elapsed_ms = session_started_at.elapsed().as_millis(),
                need_token_type_ids, "embedder ONNX session created"
            );

            info!(
                elapsed_ms = init_started_at.elapsed().as_millis(),
                "embedder init complete"
            );

            Ok(Self {
                tokenizer,
                session,
                need_token_type_ids,
            })
        }
    }

    impl EmbeddingProvider for FastEmbedProvider {
        fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, EmbedError> {
            let mut embeddings = self.embed_internal(&[query], None)?;
            Ok(embeddings.pop().unwrap_or_default())
        }

        fn embed_chunks(&mut self, chunks: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
            self.embed_internal(chunks, None)
        }
    }

    impl FastEmbedProvider {
        fn embed_internal(
            &mut self,
            texts: &[&str],
            batch_size: Option<usize>,
        ) -> Result<Vec<Vec<f32>>, EmbedError> {
            let batch_size = batch_size.unwrap_or(DEFAULT_BATCH_SIZE).max(1);
            let mut embeddings = Vec::with_capacity(texts.len());

            for batch in texts.chunks(batch_size) {
                let encodings = self
                    .tokenizer
                    .encode_batch(batch.to_vec(), true)
                    .map_err(to_embed_error)?;

                let encoding_length = encodings
                    .first()
                    .ok_or_else(|| {
                        EmbedError::Generation(String::from("tokenizer returned empty encodings"))
                    })?
                    .len();
                let batch_len = batch.len();
                let max_size = encoding_length * batch_len;

                let mut ids_array = Vec::with_capacity(max_size);
                let mut mask_array = Vec::with_capacity(max_size);
                let mut type_ids_array = Vec::with_capacity(max_size);

                for encoding in &encodings {
                    ids_array.extend(encoding.get_ids().iter().map(|x| *x as i64));
                    mask_array.extend(encoding.get_attention_mask().iter().map(|x| *x as i64));
                    type_ids_array.extend(encoding.get_type_ids().iter().map(|x| *x as i64));
                }

                let input_ids = Array::from_shape_vec((batch_len, encoding_length), ids_array)
                    .map_err(to_embed_error)?;
                let attention_mask =
                    Array::from_shape_vec((batch_len, encoding_length), mask_array)
                        .map_err(to_embed_error)?;
                let token_type_ids =
                    Array::from_shape_vec((batch_len, encoding_length), type_ids_array)
                        .map_err(to_embed_error)?;

                let mut session_inputs = inputs![
                    "input_ids" => Value::from_array(input_ids).map_err(to_embed_error)?,
                    "attention_mask" => Value::from_array(attention_mask.clone()).map_err(to_embed_error)?,
                ];

                if self.need_token_type_ids {
                    session_inputs.push((
                        "token_type_ids".into(),
                        Value::from_array(token_type_ids)
                            .map_err(to_embed_error)?
                            .into(),
                    ));
                }

                let outputs = self
                    .session
                    .run(session_inputs)
                    .map_err(to_embed_error)?
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value))
                    .collect::<Vec<_>>();

                let pooled = pool_output(&outputs, &attention_mask)?;
                for row in pooled.rows() {
                    let slice = row.as_slice().ok_or_else(|| {
                        EmbedError::Generation(String::from(
                            "failed to convert pooled embedding row to slice",
                        ))
                    })?;
                    embeddings.push(normalize(slice));
                }
            }

            Ok(embeddings)
        }
    }

    fn load_remote_artifacts(
        config: &lss_config::EmbeddingConfig,
    ) -> Result<ModelArtifacts, EmbedError> {
        let cache_dir = cache_dir_from_config(config);
        info!(
            model_code = MODEL_CODE,
            cache_dir = %cache_dir.display(),
            "embedder remote artifact resolution started"
        );

        let artifact_started_at = Instant::now();
        let api = ApiBuilder::new()
            .with_cache_dir(cache_dir)
            .with_progress(true)
            .build()
            .map_err(to_embed_error)?;
        let repo = api.model(MODEL_CODE.to_string());

        let model_path = repo.get("model.onnx").map_err(to_embed_error)?;
        let tokenizer_file = std::fs::read(repo.get("tokenizer.json").map_err(to_embed_error)?)
            .map_err(to_embed_error)?;
        let config_file = std::fs::read(repo.get("config.json").map_err(to_embed_error)?)
            .map_err(to_embed_error)?;
        let special_tokens_map_file = std::fs::read(
            repo.get("special_tokens_map.json")
                .map_err(to_embed_error)?,
        )
        .map_err(to_embed_error)?;
        let tokenizer_config_file =
            std::fs::read(repo.get("tokenizer_config.json").map_err(to_embed_error)?)
                .map_err(to_embed_error)?;

        info!(
            elapsed_ms = artifact_started_at.elapsed().as_millis(),
            model_file = %model_path.display(),
            "embedder artifact resolution complete"
        );

        Ok(ModelArtifacts {
            model_path,
            tokenizer_file,
            config_file,
            special_tokens_map_file,
            tokenizer_config_file,
        })
    }

    fn load_local_artifacts(model_dir: &Path) -> Result<ModelArtifacts, EmbedError> {
        let artifact_started_at = Instant::now();
        let model_path = model_dir.join("model.onnx");
        let tokenizer_file =
            std::fs::read(model_dir.join("tokenizer.json")).map_err(to_embed_error)?;
        let config_file = std::fs::read(model_dir.join("config.json")).map_err(to_embed_error)?;
        let special_tokens_map_file =
            std::fs::read(model_dir.join("special_tokens_map.json")).map_err(to_embed_error)?;
        let tokenizer_config_file =
            std::fs::read(model_dir.join("tokenizer_config.json")).map_err(to_embed_error)?;
        std::fs::metadata(&model_path).map_err(to_embed_error)?;
        info!(
            elapsed_ms = artifact_started_at.elapsed().as_millis(),
            model_dir = %model_dir.display(),
            "embedder artifact resolution complete"
        );

        Ok(ModelArtifacts {
            model_path,
            tokenizer_file,
            config_file,
            special_tokens_map_file,
            tokenizer_config_file,
        })
    }

    fn load_tokenizer(artifacts: &ModelArtifacts) -> Result<Tokenizer, EmbedError> {
        let config: JsonValue =
            serde_json::from_slice(&artifacts.config_file).map_err(to_embed_error)?;
        let special_tokens_map: JsonValue =
            serde_json::from_slice(&artifacts.special_tokens_map_file).map_err(to_embed_error)?;
        let tokenizer_config: JsonValue =
            serde_json::from_slice(&artifacts.tokenizer_config_file).map_err(to_embed_error)?;

        let mut tokenizer =
            Tokenizer::from_bytes(artifacts.tokenizer_file.clone()).map_err(to_embed_error)?;

        let model_max_length = tokenizer_config["model_max_length"]
            .as_f64()
            .unwrap_or(MODEL_MAX_LENGTH as f64) as usize;
        let max_length = MODEL_MAX_LENGTH.min(model_max_length);
        let pad_id = config["pad_token_id"].as_u64().unwrap_or(0) as u32;
        let pad_token = tokenizer_config["pad_token"]
            .as_str()
            .unwrap_or("[PAD]")
            .to_string();

        tokenizer = tokenizer
            .with_padding(Some(PaddingParams {
                strategy: PaddingStrategy::BatchLongest,
                pad_token,
                pad_id,
                ..Default::default()
            }))
            .with_truncation(Some(TruncationParams {
                max_length,
                ..Default::default()
            }))
            .map_err(to_embed_error)?
            .clone()
            .into();

        if let JsonValue::Object(root_object) = special_tokens_map {
            for value in root_object.values() {
                if let Some(content) = value.as_str() {
                    tokenizer.add_special_tokens(&[AddedToken {
                        content: content.into(),
                        special: true,
                        ..Default::default()
                    }]);
                } else if value.is_object() {
                    if let (
                        Some(content),
                        Some(single_word),
                        Some(lstrip),
                        Some(rstrip),
                        Some(normalized),
                    ) = (
                        value["content"].as_str(),
                        value["single_word"].as_bool(),
                        value["lstrip"].as_bool(),
                        value["rstrip"].as_bool(),
                        value["normalized"].as_bool(),
                    ) {
                        tokenizer.add_special_tokens(&[AddedToken {
                            content: content.into(),
                            special: true,
                            single_word,
                            lstrip,
                            rstrip,
                            normalized,
                        }]);
                    }
                }
            }
        }

        Ok(tokenizer)
    }

    fn pool_output(
        outputs: &[(String, Value)],
        attention_mask: &Array2<i64>,
    ) -> Result<Array2<f32>, EmbedError> {
        let output = outputs
            .iter()
            .find(|(name, _)| name == "last_hidden_state")
            .or_else(|| {
                if outputs.len() == 1 {
                    outputs.first()
                } else {
                    None
                }
            })
            .or_else(|| {
                outputs
                    .iter()
                    .find(|(name, _)| name == "sentence_embedding")
            })
            .ok_or_else(|| {
                EmbedError::Generation(format!(
                    "no suitable output found; available outputs: {:?}",
                    outputs.iter().map(|(name, _)| name).collect::<Vec<_>>()
                ))
            })?;

        let array = output
            .1
            .try_extract_array::<f32>()
            .map_err(to_embed_error)?;
        match array.ndim() {
            2 => array
                .into_dimensionality::<Ix2>()
                .map(|view| view.to_owned())
                .map_err(to_embed_error),
            3 => {
                let token_embeddings =
                    array.into_dimensionality::<Ix3>().map_err(to_embed_error)?;
                let attention_mask = attention_mask
                    .view()
                    .insert_axis(Axis(2))
                    .broadcast(token_embeddings.dim())
                    .ok_or_else(|| {
                        EmbedError::Generation(String::from(
                            "could not broadcast attention mask to token embedding shape",
                        ))
                    })?
                    .mapv(|x| x as f32);
                let masked_tensor = &attention_mask * &token_embeddings;
                let sum = masked_tensor.sum_axis(Axis(1));
                let mask_sum = attention_mask
                    .sum_axis(Axis(1))
                    .mapv(|x| if x == 0.0 { 1.0 } else { x });
                Ok(&sum / &mask_sum)
            }
            ndim => Err(EmbedError::Generation(format!(
                "invalid output rank {ndim}; expected 2D or 3D tensor"
            ))),
        }
    }

    fn normalize(v: &[f32]) -> Vec<f32> {
        let norm = (v.iter().map(|val| val * val).sum::<f32>()).sqrt();
        let epsilon = 1e-12;
        v.iter().map(|&val| val / (norm + epsilon)).collect()
    }

    fn local_model_dir_from_config(config: &lss_config::EmbeddingConfig) -> Option<PathBuf> {
        if config.model_path.is_empty() {
            return None;
        }

        local_model_dir_from_path(Path::new(&config.model_path))
    }

    fn local_model_dir_from_path(path: &Path) -> Option<PathBuf> {
        if path.is_dir() && is_local_model_dir(path) {
            return Some(path.to_path_buf());
        }

        if path.is_file() {
            let parent = path.parent()?;
            if is_local_model_dir(parent) {
                return Some(parent.to_path_buf());
            }
        }

        None
    }

    fn is_local_model_dir(path: &Path) -> bool {
        LOCAL_MODEL_FILES
            .iter()
            .all(|file| path.join(file).is_file())
    }

    fn cache_dir_from_config(config: &lss_config::EmbeddingConfig) -> PathBuf {
        if config.model_path.is_empty() {
            PathBuf::from(".fastembed_cache")
        } else {
            PathBuf::from(&config.model_path)
        }
    }

    fn to_embed_error(error: impl std::fmt::Display) -> EmbedError {
        EmbedError::Generation(error.to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::tempdir;

        #[test]
        fn backend_none_disables_provider() {
            let config = lss_config::EmbeddingConfig {
                backend: String::from("none"),
                ..Default::default()
            };

            assert!(matches!(
                FastEmbedProvider::new(&config),
                Err(EmbedError::ModelUnavailable)
            ));
        }

        #[test]
        fn detects_local_model_directory_from_directory_path() {
            let dir = tempdir().expect("temp dir should be created");
            for file in LOCAL_MODEL_FILES {
                std::fs::write(dir.path().join(file), b"placeholder")
                    .expect("placeholder file should be written");
            }

            let detected =
                local_model_dir_from_path(dir.path()).expect("local model dir should be detected");
            assert_eq!(detected, dir.path());
        }

        #[test]
        fn detects_local_model_directory_from_model_file_path() {
            let dir = tempdir().expect("temp dir should be created");
            for file in LOCAL_MODEL_FILES {
                std::fs::write(dir.path().join(file), b"placeholder")
                    .expect("placeholder file should be written");
            }

            let detected = local_model_dir_from_path(&dir.path().join("model.onnx"))
                .expect("model file path should resolve to local model dir");
            assert_eq!(detected, dir.path());
        }

        #[test]
        fn incomplete_local_model_directory_is_ignored() {
            let dir = tempdir().expect("temp dir should be created");
            std::fs::write(dir.path().join("model.onnx"), b"placeholder")
                .expect("placeholder file should be written");

            assert!(local_model_dir_from_path(dir.path()).is_none());
        }

        #[test]
        #[ignore = "downloads the embedding model when not already cached locally"]
        fn loads_model_and_generates_embeddings() {
            let cache_dir = tempdir().expect("temp dir should be created");
            let config = lss_config::EmbeddingConfig {
                model_path: cache_dir.path().to_string_lossy().to_string(),
                ..Default::default()
            };

            let mut provider =
                FastEmbedProvider::new(&config).expect("provider should load the model");
            let query = provider
                .embed_query("hello world")
                .expect("query embedding should succeed");
            assert_eq!(query.len(), MODEL_DIMENSION);

            let chunks = provider
                .embed_chunks(&["hello world", "semantic search"])
                .expect("chunk embeddings should succeed");
            assert_eq!(chunks.len(), 2);
            assert!(
                chunks
                    .iter()
                    .all(|embedding| embedding.len() == MODEL_DIMENSION)
            );
        }
    }
}
