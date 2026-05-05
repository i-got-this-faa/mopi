#![allow(clippy::collapsible_if)]
use anyhow::{Result, anyhow};
use mopi_chunking::Chunker;
use mopi_config::{AppConfig, MopiPaths};
use mopi_crawl::watch::{MopiWatcher, WatchSignal};
use mopi_crawl::{ChangeEvent, ChangeKind, CrawlFingerprint, CrawlSnapshot, CrawlSnapshotEntry};
use mopi_embed::EmbeddingProvider;
use mopi_embed::provider::FastEmbedProvider;
use mopi_extract::Dispatcher;
use mopi_index_lexical::LexicalStore;
use mopi_index_meta::{FileRecord, MetaStore};
use mopi_index_vector::index::VectorIndex;
use mopi_ipc::{
    PROTOCOL_VERSION, Request, RequestEnvelope, Response, ResponseEnvelope, read_frame, write_frame,
};
use mopi_types::{DaemonState, DaemonStats, DaemonStatus, DoctorCheck, DoctorReport, RootSummary};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{RwLock, mpsc};
use tracing::{error, info, warn};

struct SharedState {
    paths: MopiPaths,
    config: RwLock<AppConfig>,
    status: RwLock<DaemonStatus>,
    snapshot: RwLock<Option<CrawlSnapshot>>,
    search_requests: AtomicU64,
    config_reloads: AtomicU64,
    meta: Mutex<MetaStore>,
    lexical: Mutex<LexicalStore>,
    vector: Mutex<Option<VectorIndex<'static>>>,
    embedder: Mutex<Option<FastEmbedProvider>>,
    dispatcher: Dispatcher,
    change_tx: mpsc::Sender<Vec<ChangeEvent>>,
    watch_control_tx: mpsc::Sender<WatchCommand>,
}

#[derive(Debug, Clone)]
enum WatchCommand {
    Rebuild(AppConfig),
}

#[tokio::main]
async fn main() -> Result<()> {
    mopi_config::init_tracing();

    let paths = MopiPaths::discover()?;
    paths.ensure_layout()?;
    let config = AppConfig::load_or_default(&paths)?;
    let socket_path = config.daemon.socket_path(&paths);

    prepare_socket(socket_path.as_std_path()).await?;
    let listener = UnixListener::bind(socket_path.as_std_path())?;

    let meta_path = paths.data_dir.join("meta.db");
    let lexical_path = paths.data_dir.join("lexical");
    let vector_path = paths.data_dir.join("vector");
    if !vector_path.exists() {
        std::fs::create_dir_all(&vector_path)?;
    }

    let meta = MetaStore::open(meta_path.as_std_path())?;
    let lexical = LexicalStore::open(&lexical_path)?;
    let vector = match VectorIndex::load(vector_path.as_std_path(), "hnsw") {
        Ok(v) => Some(v),
        Err(_) => Some(VectorIndex::new(384, 16)), // AllMiniLML6V2 is 384d
    };

    let (tx, rx) = mpsc::channel(100);
    let (watch_signal_tx, watch_signal_rx) = mpsc::channel(100);
    let (watch_control_tx, watch_control_rx) = mpsc::channel(8);
    let state = Arc::new(SharedState {
        paths: paths.clone(),
        status: RwLock::new(DaemonStatus {
            state: DaemonState::Starting,
            indexed_documents: 0,
            roots: config.roots.len(),
        }),
        config: RwLock::new(config.clone()),
        snapshot: RwLock::new(None),
        search_requests: AtomicU64::new(0),
        config_reloads: AtomicU64::new(0),
        meta: Mutex::new(meta),
        lexical: Mutex::new(lexical),
        vector: Mutex::new(vector),
        embedder: Mutex::new(None),
        dispatcher: Dispatcher::new(),
        change_tx: tx.clone(),
        watch_control_tx: watch_control_tx.clone(),
    });

    let server_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state = Arc::clone(&server_state);
                    tokio::spawn(async move {
                        if let Err(error) = handle_client(stream, state).await {
                            warn!(error = %error, "client session ended with error");
                        }
                    });
                }
                Err(error) => {
                    error!(error = %error, "ipc listener failed");
                    break;
                }
            }
        }
    });

    if let Some(provider) = initialize_embedder(&config)? {
        if let Ok(mut embedder) = state.embedder.lock() {
            *embedder = Some(provider);
        }
        info!("embedding model loaded successfully");
        if let Err(error) = recover_empty_vector_index(&state).await {
            warn!(error = %error, "failed to recover empty vector index");
        }
    }
    state.status.write().await.state = DaemonState::Ready;

    // Start background indexing
    let state_for_indexer = Arc::clone(&state);
    tokio::spawn(async move {
        run_indexer(state_for_indexer, rx).await;
    });

    // Start crawler
    let state_for_crawler = Arc::clone(&state);
    let tx_for_crawler = tx.clone();
    tokio::spawn(async move {
        if let Err(e) = run_crawler(state_for_crawler, tx_for_crawler).await {
            error!(error = %e, "crawler failed");
        }
    });

    // Start watcher
    let state_for_watcher = Arc::clone(&state);
    let watcher_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = run_watcher(
            state_for_watcher,
            watcher_config,
            watch_signal_tx,
            watch_signal_rx,
            watch_control_rx,
        )
        .await
        {
            error!(error = %e, "watcher failed");
        }
    });

    info!(config_root = %paths.config_dir, runtime_dir = %paths.runtime_dir, data_dir = %paths.data_dir, cache_dir = %paths.cache_dir, "starting mopid");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("received shutdown signal");
                break;
            }
        }
    }

    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    Ok(())
}

fn initialize_embedder(config: &AppConfig) -> Result<Option<FastEmbedProvider>> {
    if config.embedding.backend == "none" {
        return Ok(None);
    }

    info!("initializing embedding model before indexing starts...");
    match FastEmbedProvider::new(&config.embedding) {
        Ok(provider) => Ok(Some(provider)),
        Err(error) => {
            if config.embedding.strict_startup {
                Err(anyhow!(
                    "failed to load embedding model with strict startup enabled: {error}"
                ))
            } else {
                warn!(error = %error, "failed to load embedding model, continuing in lexical-only mode");
                Ok(None)
            }
        }
    }
}

async fn handle_client(mut stream: UnixStream, status: SharedStatus) -> Result<()> {
    loop {
        let envelope = match read_frame::<RequestEnvelope>(&mut stream).await {
            Ok(envelope) => envelope,
            Err(error) => {
                if let Some(io_error) = error.downcast_ref::<std::io::Error>()
                    && io_error.kind() == std::io::ErrorKind::UnexpectedEof
                {
                    return Ok(());
                }

                return Err(error);
            }
        };

        let response = dispatch_request(envelope, &status).await;

        write_frame(&mut stream, &ResponseEnvelope::new(response)).await?;
    }
}

type SharedStatus = Arc<SharedState>;

async fn dispatch_request(envelope: RequestEnvelope, state: &SharedStatus) -> Response {
    if envelope.protocol_version != PROTOCOL_VERSION {
        return Response::Error {
            message: format!(
                "protocol mismatch: client={}, daemon={}",
                envelope.protocol_version, PROTOCOL_VERSION
            ),
        };
    }

    match envelope.request {
        Request::Ping => Response::Pong {
            protocol_version: PROTOCOL_VERSION,
        },
        Request::GetStatus => Response::Status(state.status.read().await.clone()),
        Request::GetStats => Response::Stats(DaemonStats {
            protocol_version: PROTOCOL_VERSION,
            indexed_documents: state.status.read().await.indexed_documents,
            configured_roots: state.config.read().await.roots.len(),
            search_requests: state.search_requests.load(Ordering::Relaxed),
            config_reloads: state.config_reloads.load(Ordering::Relaxed),
        }),
        Request::ListRoots => Response::Roots(
            state
                .config
                .read()
                .await
                .roots
                .iter()
                .map(|root| RootSummary {
                    path: root.path.clone(),
                })
                .collect(),
        ),
        Request::ReloadConfig => match AppConfig::load_or_default(&state.paths) {
            Ok(config) => {
                let root_count = config.roots.len();
                *state.config.write().await = config.clone();
                state.status.write().await.roots = root_count;
                state.config_reloads.fetch_add(1, Ordering::Relaxed);
                match state
                    .watch_control_tx
                    .send(WatchCommand::Rebuild(config))
                    .await
                {
                    Ok(()) => Response::Ack {
                        message: String::from("config reloaded"),
                    },
                    Err(error) => Response::Error {
                        message: format!("failed to rebuild watcher after reload: {error}"),
                    },
                }
            }
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Request::RefreshChanged => match refresh_changed(state).await {
            Ok(event_count) => Response::Ack {
                message: format!("queued {} changed file events", event_count),
            },
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Request::Doctor => Response::Doctor(build_doctor_report(state).await),
        Request::Search { query_id, query } => {
            state.search_requests.fetch_add(1, Ordering::Relaxed);
            let lexical_results = match state.lexical.lock() {
                Ok(lexical) => {
                    let mut lex_query = query.clone();
                    lex_query.limit *= 2;
                    match lexical.search(&lex_query) {
                        Ok(results) => {
                            let mut search_results = Vec::new();
                            for result in results {
                                if let Ok(id_uuid) = uuid::Uuid::parse_str(&result.id) {
                                    let doc_id = mopi_types::DocumentId(id_uuid);
                                    search_results.push(mopi_types::SearchResult {
                                        document_id: doc_id,
                                        path: camino::Utf8PathBuf::from(result.path),
                                        title: result.filename,
                                        snippet: result.snippet,
                                        score: result.score,
                                        reasons: vec![mopi_types::MatchReason::Content],
                                    });
                                }
                            }
                            search_results
                        }
                        Err(e) => {
                            warn!("lexical search failed: {}", e);
                            Vec::new()
                        }
                    }
                }
                Err(_) => {
                    warn!("lexical store mutex poisoned");
                    Vec::new()
                }
            };

            let mut semantic_results = Vec::new();
            if let Ok(mut embedder_guard) = state.embedder.lock() {
                if let Some(embedder) = embedder_guard.as_mut() {
                    if let Ok(query_vector) = embedder.embed_query(&query.raw) {
                        if let Ok(vector) = state.vector.lock() {
                            if let Some(v) = vector.as_ref() {
                                if let Ok(neighbors) = v.search(&query_vector, query.limit * 2) {
                                    if let Ok(meta) = state.meta.lock() {
                                        for (chunk_id, score) in neighbors {
                                            if let Ok(Some(file_record)) =
                                                meta.get_file_by_chunk_id(chunk_id as i64)
                                            {
                                                let snippet = meta
                                                    .get_chunk_text(chunk_id as i64)
                                                    .unwrap_or_default()
                                                    .unwrap_or_else(|| String::from("..."));
                                                if let Ok(id_uuid) =
                                                    uuid::Uuid::parse_str(&file_record.id)
                                                {
                                                    semantic_results.push(
                                                        mopi_types::SearchResult {
                                                            document_id: mopi_types::DocumentId(
                                                                id_uuid,
                                                            ),
                                                            path: camino::Utf8PathBuf::from(
                                                                &file_record.canonical_path,
                                                            ),
                                                            title: file_record.file_name.clone(),
                                                            snippet,
                                                            score,
                                                            reasons: vec![
                                                                mopi_types::MatchReason::Semantic,
                                                            ],
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let combined =
                mopi_rank::combine_and_rank(lexical_results, semantic_results, query.limit);

            Response::SearchResults {
                query_id,
                results: combined,
            }
        }
    }
}

async fn run_indexer(state: SharedStatus, mut rx: mpsc::Receiver<Vec<mopi_crawl::ChangeEvent>>) {
    info!("indexer worker started");
    while let Some(events) = rx.recv().await {
        info!("indexer received {} events", events.len());
        for event in events {
            if let Err(e) = process_change_event(&state, event).await {
                error!(error = %e, "failed to process change event");
            }
        }
        info!("committing lexical index...");
        match state.lexical.lock() {
            Ok(mut lexical) => {
                info!("got lexical lock");
                if let Err(e) = lexical.commit() {
                    error!(error = %e, "failed to commit lexical index");
                } else {
                    info!("lexical index committed");
                }
            }
            Err(_) => error!("failed to lock lexical store for commit"),
        }

        info!("locking vector index for save...");
        if let Ok(mut vector) = state.vector.lock() {
            info!("got vector lock");
            if let Some(v) = vector.as_mut() {
                let vector_path = state.paths.data_dir.join("vector");
                info!("saving vector index to {:?}", vector_path);
                if let Err(e) = v.save(vector_path.as_std_path(), "hnsw") {
                    error!(error = %e, "failed to save vector index");
                } else {
                    info!("vector index saved");
                }
            }
        }
    }
}

async fn process_change_event(state: &SharedStatus, event: mopi_crawl::ChangeEvent) -> Result<()> {
    match event.kind {
        ChangeKind::Added | ChangeKind::Modified => {
            if let Some(candidate) = event.candidate {
                info!(path = %candidate.canonical_path, "processing add/modify");
                let config = state.config.read().await.extraction.clone();
                let root_path = state
                    .config
                    .read()
                    .await
                    .roots
                    .get(candidate.root_id)
                    .map(|root| root.path.clone())
                    .ok_or_else(|| anyhow!("invalid crawl root id {}", candidate.root_id))?;
                match state.dispatcher.extract(&candidate.canonical_path, &config) {
                    Ok(output) => {
                        info!(path = %candidate.canonical_path, "extraction successful");
                        let existing_id = state
                            .meta
                            .lock()
                            .map_err(|_| anyhow!("meta store mutex poisoned"))?
                            .get_file_by_canonical_path(candidate.canonical_path.as_str())?;
                        let doc_id = match existing_id.as_deref() {
                            Some(id) => mopi_types::DocumentId(uuid::Uuid::parse_str(id).map_err(
                                |error| anyhow!("invalid stored document id `{id}`: {error}"),
                            )?),
                            None => mopi_types::DocumentId::new(),
                        };
                        let root_id = state
                            .meta
                            .lock()
                            .map_err(|_| anyhow!("meta store mutex poisoned"))?
                            .upsert_root(root_path.as_str())?;

                        if let Some(id) = existing_id.as_deref() {
                            state
                                .lexical
                                .lock()
                                .map_err(|_| anyhow!("lexical store mutex poisoned"))?
                                .delete_document(id)?;

                            // Delete from vector index
                            let chunk_ids = state
                                .meta
                                .lock()
                                .map_err(|_| anyhow!("meta store mutex poisoned"))?
                                .get_chunks_for_file(id)?;
                            if let Ok(mut vector) = state.vector.lock() {
                                if let Some(v) = vector.as_mut() {
                                    let chunk_ids_usize: Vec<usize> =
                                        chunk_ids.into_iter().map(|id| id as usize).collect();
                                    let _ = v.delete_chunks(&chunk_ids_usize);
                                }
                            }
                        }

                        state
                            .meta
                            .lock()
                            .map_err(|_| anyhow!("meta store mutex poisoned"))?
                            .upsert_file(FileRecord {
                                id: &doc_id,
                                root_id,
                                canonical_path: candidate.canonical_path.as_str(),
                                file_name: &candidate.file_name,
                                extension: candidate.extension.as_deref(),
                                size: candidate.file_size,
                                modified_unix_seconds: candidate.modified_unix_seconds,
                            })?;

                        for alias in &candidate.alias_paths {
                            state
                                .meta
                                .lock()
                                .map_err(|_| anyhow!("meta store mutex poisoned"))?
                                .upsert_alias(&doc_id.0.to_string(), alias.as_str())?;
                        }

                        let alias_slices: Vec<&str> =
                            candidate.alias_paths.iter().map(|p| p.as_str()).collect();

                        state
                            .lexical
                            .lock()
                            .map_err(|_| anyhow!("lexical store mutex poisoned"))?
                            .add_document(
                                &doc_id.0.to_string(),
                                candidate.canonical_path.as_str(),
                                &alias_slices,
                                &candidate.file_name,
                                &output.text,
                                candidate.extension.as_deref(),
                                Some(&output.mime),
                            )?;

                        let chunker = Chunker::default();
                        let chunks = chunker.chunk(&output.text);
                        let mut chunk_data = Vec::new();

                        info!(path = %candidate.canonical_path, chunk_count = chunks.len(), "chunking complete");
                        let chunk_texts: Vec<String> =
                            chunks.iter().map(|chunk| chunk.text.clone()).collect();
                        let sqlite_chunk_ids = state
                            .meta
                            .lock()
                            .map_err(|_| anyhow!("meta store mutex poisoned"))?
                            .replace_chunks(&doc_id.0.to_string(), &chunk_texts)?;
                        state
                            .meta
                            .lock()
                            .map_err(|_| anyhow!("meta store mutex poisoned"))?
                            .set_extractor_status(&doc_id.0.to_string(), "done")?;
                        info!(path = %candidate.canonical_path, "chunks stored in metadata db");

                        match state.embedder.lock() {
                            Ok(mut embedder_guard) => {
                                if let Some(embedder) = embedder_guard.as_mut() {
                                    info!(path = %candidate.canonical_path, "generating embeddings...");
                                    let text_slices: Vec<&str> =
                                        chunks.iter().map(|c| c.text.as_str()).collect();
                                    match embedder.embed_chunks(&text_slices) {
                                        Ok(embeddings) => {
                                            if embeddings.len() != sqlite_chunk_ids.len() {
                                                warn!(
                                                    path = %candidate.canonical_path,
                                                    expected = sqlite_chunk_ids.len(),
                                                    actual = embeddings.len(),
                                                    "embedding batch size mismatch"
                                                );
                                            }
                                            info!(path = %candidate.canonical_path, "embeddings generated");
                                            for (chunk_id, embedding) in
                                                sqlite_chunk_ids.iter().zip(embeddings.into_iter())
                                            {
                                                chunk_data.push((*chunk_id as usize, embedding));
                                            }
                                        }
                                        Err(e) => {
                                            warn!(path = %candidate.canonical_path, error = %e, "embedding failed");
                                        }
                                    }
                                } else {
                                    info!(path = %candidate.canonical_path, "embedder unavailable, skipping semantic index");
                                }
                            }
                            Err(_) => {
                                warn!(path = %candidate.canonical_path, "embedder mutex poisoned, skipping semantic index");
                            }
                        }

                        if !chunk_data.is_empty() {
                            if let Ok(mut vector) = state.vector.lock() {
                                if let Some(v) = vector.as_mut() {
                                    info!(path = %candidate.canonical_path, "upserting to vector index...");
                                    if let Err(e) = v.upsert_chunks(&chunk_data) {
                                        warn!(path = %candidate.canonical_path, error = %e, "vector upsert failed");
                                    }
                                    info!(path = %candidate.canonical_path, "vector upsert complete");
                                }
                            }
                        }

                        update_snapshot_for_event(
                            state,
                            ChangeEvent {
                                kind: event.kind,
                                canonical_path: candidate.canonical_path.clone(),
                                candidate: Some(candidate.clone()),
                            },
                        )
                        .await;

                        if event.kind == ChangeKind::Added && existing_id.is_none() {
                            state.status.write().await.indexed_documents += 1;
                        }
                    }
                    Err(e) => {
                        warn!(path = %candidate.canonical_path, error = %e, "extraction failed")
                    }
                }
            }
        }
        ChangeKind::Deleted => {
            let id_opt = state
                .meta
                .lock()
                .map_err(|_| anyhow!("meta store mutex poisoned"))?
                .get_file_by_canonical_path(event.canonical_path.as_str())?;
            if let Some(id_str) = id_opt {
                state
                    .lexical
                    .lock()
                    .map_err(|_| anyhow!("lexical store mutex poisoned"))?
                    .delete_document(&id_str)?;

                {
                    let meta = state
                        .meta
                        .lock()
                        .map_err(|_| anyhow!("meta store mutex poisoned"))?;
                    if let Ok(chunk_ids) = meta.get_chunks_for_file(&id_str) {
                        if let Ok(mut vector) = state.vector.lock() {
                            if let Some(v) = vector.as_mut() {
                                let chunk_ids_usize: Vec<usize> =
                                    chunk_ids.into_iter().map(|id| id as usize).collect();
                                let _ = v.delete_chunks(&chunk_ids_usize);
                            }
                        }
                    }
                    meta.delete_file(&id_str)?;
                }
                update_snapshot_for_event(state, event.clone()).await;
                let mut status = state.status.write().await;
                status.indexed_documents = status.indexed_documents.saturating_sub(1);
            }
        }
        ChangeKind::Unchanged => {}
    }

    Ok(())
}

async fn recover_empty_vector_index(state: &SharedStatus) -> Result<()> {
    let chunk_count = state
        .meta
        .lock()
        .map_err(|_| anyhow!("meta store mutex poisoned"))?
        .count_chunks()?;
    if chunk_count == 0 {
        return Ok(());
    }

    let needs_recovery = match state.vector.lock() {
        Ok(vector) => vector
            .as_ref()
            .is_some_and(|index| index.point_count() == 0),
        Err(_) => false,
    };
    if !needs_recovery {
        return Ok(());
    }

    let batch_size = state
        .config
        .read()
        .await
        .embedding
        .indexing_batch_size
        .max(1);
    info!(
        chunk_count,
        batch_size, "vector index is empty, backfilling from stored chunks"
    );

    let mut last_chunk_id = 0_i64;
    loop {
        let chunk_batch = state
            .meta
            .lock()
            .map_err(|_| anyhow!("meta store mutex poisoned"))?
            .list_chunks_after(last_chunk_id, batch_size)?;
        if chunk_batch.is_empty() {
            break;
        }

        let embeddings = {
            let mut embedder_guard = state
                .embedder
                .lock()
                .map_err(|_| anyhow!("embedder mutex poisoned"))?;
            let embedder = embedder_guard
                .as_mut()
                .ok_or_else(|| anyhow!("embedder not available for vector recovery"))?;
            let text_slices: Vec<&str> =
                chunk_batch.iter().map(|(_, text)| text.as_str()).collect();
            embedder.embed_chunks(&text_slices)?
        };

        if embeddings.len() != chunk_batch.len() {
            return Err(anyhow!(
                "embedding batch size mismatch during vector recovery: expected {}, got {}",
                chunk_batch.len(),
                embeddings.len()
            ));
        }

        let chunk_vectors: Vec<(usize, Vec<f32>)> = chunk_batch
            .iter()
            .zip(embeddings.into_iter())
            .map(|((chunk_id, _), embedding)| (*chunk_id as usize, embedding))
            .collect();

        if let Ok(mut vector) = state.vector.lock() {
            if let Some(index) = vector.as_mut() {
                index.upsert_chunks(&chunk_vectors)?;
            }
        }

        last_chunk_id = chunk_batch
            .last()
            .map(|(chunk_id, _)| *chunk_id)
            .unwrap_or(last_chunk_id);
    }

    let vector_path = state.paths.data_dir.join("vector");
    if let Ok(mut vector) = state.vector.lock() {
        if let Some(index) = vector.as_mut() {
            index.save(vector_path.as_std_path(), "hnsw")?;
            info!(
                point_count = index.point_count(),
                "vector recovery complete"
            );
        }
    }

    Ok(())
}

async fn run_crawler(
    state: SharedStatus,
    tx: mpsc::Sender<Vec<mopi_crawl::ChangeEvent>>,
) -> Result<()> {
    info!("initial crawler started");
    let config = state.config.read().await.clone();

    for root in &config.roots {
        state
            .meta
            .lock()
            .map_err(|_| anyhow!("meta store mutex poisoned"))?
            .upsert_root(root.path.as_str())?;
    }

    let changes = mopi_crawl::discover_changes(&config, None)?;
    *state.snapshot.write().await = Some(changes.snapshot.clone());
    let events: Vec<_> = changes
        .events
        .into_iter()
        .filter(|event| event.kind != ChangeKind::Unchanged)
        .collect();

    let batch_size = state.config.read().await.indexing.max_in_flight_jobs;
    for chunk in events.chunks(batch_size) {
        tx.send(chunk.to_vec()).await?;
    }

    info!("initial crawl finished");
    Ok(())
}

async fn run_watcher(
    state: SharedStatus,
    initial_config: AppConfig,
    watch_signal_tx: mpsc::Sender<WatchSignal>,
    mut watch_signal_rx: mpsc::Receiver<WatchSignal>,
    mut watch_control_rx: mpsc::Receiver<WatchCommand>,
) -> Result<()> {
    let mut current_watcher = Some(MopiWatcher::new(initial_config, watch_signal_tx.clone())?);
    info!("file watcher active");

    loop {
        tokio::select! {
            Some(signal) = watch_signal_rx.recv() => {
                match signal {
                    WatchSignal::RefreshRequested => {
                        if let Err(error) = refresh_changed(&state).await {
                            error!(error = %error, "watch-triggered refresh failed");
                        }
                    }
                }
            }
            Some(command) = watch_control_rx.recv() => {
                match command {
                    WatchCommand::Rebuild(config) => {
                        current_watcher = Some(MopiWatcher::new(config, watch_signal_tx.clone())?);
                        if let Err(error) = refresh_changed(&state).await {
                            error!(error = %error, "post-reload refresh failed");
                        }
                    }
                }
            }
            else => {
                let _ = &current_watcher;
                break;
            }
        }
    }

    Ok(())
}

async fn refresh_changed(state: &SharedStatus) -> Result<usize> {
    let config = state.config.read().await.clone();
    let previous_snapshot = state.snapshot.read().await.clone();
    let changes = mopi_crawl::discover_changes(&config, previous_snapshot.as_ref())?;
    let event_count = changes
        .events
        .iter()
        .filter(|event| event.kind != ChangeKind::Unchanged)
        .count();

    *state.snapshot.write().await = Some(changes.snapshot);

    if event_count > 0 {
        let queueable: Vec<_> = changes
            .events
            .into_iter()
            .filter(|event| event.kind != ChangeKind::Unchanged)
            .collect();
        let batch_size = state.config.read().await.indexing.max_in_flight_jobs;
        for chunk in queueable.chunks(batch_size) {
            state.change_tx.send(chunk.to_vec()).await?;
        }
    }

    Ok(event_count)
}

async fn update_snapshot_for_event(state: &SharedStatus, event: ChangeEvent) {
    let mut snapshot_guard = state.snapshot.write().await;
    let snapshot = snapshot_guard.get_or_insert_with(|| CrawlSnapshot {
        files: Default::default(),
    });

    match event.kind {
        ChangeKind::Added | ChangeKind::Modified => {
            if let Some(candidate) = event.candidate {
                snapshot.files.insert(
                    event.canonical_path,
                    CrawlSnapshotEntry {
                        fingerprint: CrawlFingerprint::from_candidate(&candidate),
                        candidate,
                    },
                );
            }
        }
        ChangeKind::Deleted => {
            snapshot.files.remove(&event.canonical_path);
        }
        ChangeKind::Unchanged => {}
    }
}

async fn build_doctor_report(state: &SharedStatus) -> DoctorReport {
    let config = state.config.read().await.clone();
    let socket_path = config.daemon.socket_path(&state.paths);
    let checks = vec![
        DoctorCheck {
            name: String::from("config_file_parent"),
            ok: state.paths.config_dir.exists(),
            detail: format!("config dir {}", state.paths.config_dir),
        },
        DoctorCheck {
            name: String::from("runtime_dir"),
            ok: state.paths.runtime_dir.exists(),
            detail: format!("runtime dir {}", state.paths.runtime_dir),
        },
        DoctorCheck {
            name: String::from("socket"),
            ok: socket_path.exists(),
            detail: format!("socket {}", socket_path),
        },
        DoctorCheck {
            name: String::from("config_validation"),
            ok: AppConfig::validate(&config).is_ok(),
            detail: String::from("current config reload state"),
        },
    ];

    DoctorReport {
        ok: checks.iter().all(|check| check.ok),
        checks,
    }
}

async fn prepare_socket(socket_path: &std::path::Path) -> Result<()> {
    if !socket_path.exists() {
        return Ok(());
    }

    match UnixStream::connect(socket_path).await {
        Ok(_) => anyhow::bail!(
            "mopid appears to already be running at {}",
            socket_path.display()
        ),
        Err(_) => {
            std::fs::remove_file(socket_path)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    const LOCAL_MODEL_FILES: [&str; 5] = [
        "model.onnx",
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ];

    #[test]
    fn initialize_embedder_returns_none_when_backend_disabled() {
        let mut config = AppConfig::default();
        config.embedding.backend = String::from("none");

        let embedder = initialize_embedder(&config).expect("backend none should not fail");
        assert!(embedder.is_none());
    }

    #[test]
    fn initialize_embedder_falls_back_when_loading_fails_without_strict_startup() {
        let model_dir = make_invalid_local_model_dir();
        let mut config = AppConfig::default();
        config.embedding.model_path = model_dir.path().to_string_lossy().to_string();

        let embedder =
            initialize_embedder(&config).expect("non-strict startup should fall back cleanly");
        assert!(embedder.is_none());
    }

    #[test]
    fn initialize_embedder_errors_when_strict_startup_loading_fails() {
        let model_dir = make_invalid_local_model_dir();
        let mut config = AppConfig::default();
        config.embedding.model_path = model_dir.path().to_string_lossy().to_string();
        config.embedding.strict_startup = true;

        match initialize_embedder(&config) {
            Ok(_) => panic!("strict startup should propagate load errors"),
            Err(error) => assert!(
                error
                    .to_string()
                    .contains("failed to load embedding model with strict startup enabled")
            ),
        }
    }

    fn make_invalid_local_model_dir() -> TempDir {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        for file in LOCAL_MODEL_FILES {
            std::fs::write(dir.path().join(file), b"invalid")
                .expect("placeholder file should be written");
        }
        let unreadable = dir.path().join("model.onnx");
        let mut permissions = std::fs::metadata(&unreadable)
            .expect("model file metadata should load")
            .permissions();
        permissions.set_mode(0o000);
        std::fs::set_permissions(&unreadable, permissions)
            .expect("model file permissions should be updated");
        dir
    }
}
