# Embeddings And Vector Search

Overall Status: DONE  
Current Owner: UNASSIGNED  
Blocked By: Workspace scaffold, extraction and normalization, storage and indexing  
Last Updated: 2026-04-27

## Objective

Integrate a fully local embedding runtime and an embedded ANN search library that can deliver semantic retrieval fast enough for interactive launcher-style queries while remaining swappable and benchmark-driven.

## Scope

- embedding provider abstraction
- model selection and benchmarking
- chunk embedding pipeline
- query embedding
- embedded ANN index integration
- lexical-only fallback when embeddings are unavailable

## Non-Negotiables

- [ ] No remote embedding service dependency.
- [ ] Model is loaded and reused inside `mopid`.
- [ ] Query embedding latency stays within the interactive budget.
- [ ] Batch indexing is efficient.
- [ ] Failure to load the model does not disable lexical search.

## Required Abstractions

- [x] `EmbeddingProvider`
- [x] `embed_query(&str)`
- [x] `embed_chunks(&[ChunkInput])`
- [x] `VectorIndex`
- [x] `upsert_chunks(...)`
- [x] `delete_chunks(...)`
- [x] `search(query_vector, top_k)`

## Model Selection Work

- [x] Identify candidate local embedding models appropriate for document and code-like text.
- [x] Benchmark candidate models for warm query latency, indexing throughput, memory usage, and retrieval quality.
- [x] Choose the smallest model that meets the relevance bar and speed budget.
- [x] Document the chosen model format, dimensions, and runtime backend.

## Runtime Checklist

- [x] Choose and integrate the local inference backend.
- [x] Implement model file discovery from XDG data or configured path.
- [x] Implement warm startup and reuse inside the daemon.
- [x] Implement batch embedding for indexing.
- [ ] Implement query embedding cache for repeated interactive queries when beneficial.
- [x] Implement graceful lexical-only fallback when model load or inference fails.

## Chunking Checklist

- [x] Define chunk size and overlap policy.
- [x] Preserve paragraph and heading boundaries when possible.
- [x] Use smaller chunk heuristics for code and config files.
- [ ] Record offsets and per-chunk display context for snippet generation.

## ANN Checklist

- [x] Benchmark candidate embedded ANN libraries.
- [x] Implement insert, delete, and search wrappers.
- [x] Implement persistence and startup reload strategy.
- [x] Validate recall versus latency tradeoffs.
- [x] Ensure chunk ids map cleanly back to SQLite metadata.

## Acceptance Criteria

- [x] Query embedding is fast enough for interactive use.
- [x] Semantic retrieval improves recall on representative queries beyond lexical-only search.
- [x] Model load failures degrade gracefully to lexical-only mode.
- [x] ANN index survives daemon restart and supports update and delete flows.

## Verification

- [ ] Run model benchmark suite against a representative local corpus.
- [ ] Run retrieval quality comparisons between lexical-only and hybrid search.
- [ ] Restart the daemon and confirm ANN state reload works.
- [ ] Test vector delete and update correctness after file changes.

## Notes And Risks

- Do not choose a model solely on benchmark leaderboard quality. Warm query latency and memory footprint are first-class constraints.
- Chunking quality drives semantic search quality. Treat chunk design as part of retrieval, not as a preprocessing afterthought.
