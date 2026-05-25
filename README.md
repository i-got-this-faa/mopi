# lss: Local Semantic Search

`lss` is a blazing fast, secure, and modular local semantic search engine for your files. It combines traditional lexical search with modern deep learning embeddings to provide "search-as-you-type" performance for semantic intent.

## Core Architecture

- **lssd**: A high-performance daemon that owns indexes, local model state, and background indexing jobs.
- **lssctl**: An administrative and query CLI for controlling the engine.
- **lssi**: A Wayland-friendly, launcher-style GUI for interactive search.

## Deep Learning & Performance

lss is designed for **extreme local performance** without cloud dependencies:

- **Local Embeddings**: Uses `all-MiniLM-L6-v2` via **FastEmbed-RS** and **ONNX Runtime**.
- **Hardware Optimization**: Quantized `INT8` weights and SIMD/AVX-512 utilization for sub-20ms query embedding.
- **Hybrid Retrieval**: Employs **Reciprocal Rank Fusion (RRF)** to combine Tantivy (lexical) and HNSW (vector) search results.
- **Semantic Chunking**: Structure-aware file splitting (paragraphs, code blocks) for higher relevance compared to fixed-window methods.

## Features

- [x] **Local-First**: No data leaves your machine. Fully private.
- [x] **Blazing Fast**: Interactive queries in ~35ms (P95).
- [x] **Rich Document Support**: Plain text, Config (JSON/TOML/YAML), Office (DOCX/ODT), and PDF.
- [x] **Intelligent Crawling**: Whitelist/Blacklist policies, symlink loop protection, and incremental refresh.
- [x] **Daemon-Client Model**: Low-overhead IPC over Unix sockets.

## Workspace Layout

- `bin/`: Binary crates (`lssd`, `lssctl`, `lssi`).
- `crates/`: Modular shared libraries (extraction, indexing, embedding, etc.).
- `plans/`: Detailed execution and architecture roadmaps.
- `fixtures/`: Test data for various file formats.

## Developer Quickstart

1. **Prerequisites**: Install stable Rust, `rustfmt`, and `clippy`.
2. **Build**: `cargo build --release`
3. **Test**: `cargo test --workspace`
4. **Run**: Start the daemon with `cargo run --bin lssd` and query with `lssctl`.

See `plans/00-master-tracker.md` for the current roadmap and development status.
