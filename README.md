# lss: Local Semantic Search for Agents

`lss` is a blazing fast, secure, and modular local semantic search engine tailored specifically for AI agents (like OpenCode). It exclusively indexes git repositories, providing deep, semantic understanding of codebases.

## Core Architecture

- **lssd**: A high-performance daemon that owns indexes, local model state, and background indexing jobs. It watches and indexes git repositories automatically.
- **lssctl**: An administrative and query CLI for controlling the engine, providing structured JSON outputs ideal for agent toolcalls.
- **skill interface**: Specialized interfaces (MCP/Toolcalls) designed to be plugged directly into agents like OpenCode for immediate context retrieval.

## Deep Learning & Performance

lss is designed for **extreme local performance** without cloud dependencies:

- **Local Embeddings**: Uses `all-MiniLM-L6-v2` via **FastEmbed-RS** and **ONNX Runtime**.
- **Hardware Optimization**: Quantized `INT8` weights and SIMD/AVX-512 utilization for sub-20ms query embedding.
- **Hybrid Retrieval**: Employs **Reciprocal Rank Fusion (RRF)** to combine Tantivy (lexical) and HNSW (vector) search results.
- **Semantic Chunking**: Structure-aware file splitting (AST parsing, code blocks) for higher relevance in codebases.

## Features

- [x] **Agent-First**: Designed to provide toolcalls and skills for LLMs and agents.
- [x] **Git Exclusive**: Deeply understands git repos, ignores ignored files natively.
- [x] **Local-First**: No data leaves your machine. Fully private.
- [x] **Blazing Fast**: Interactive queries in ~35ms (P95).
- [x] **Daemon-Client Model**: Low-overhead IPC over Unix sockets.

## Workspace Layout

- `bin/`: Binary crates (`lssd`, `lssctl`).
- `crates/`: Modular shared libraries (extraction, indexing, embedding, etc.).

## Developer Quickstart

1. **Prerequisites**: Install stable Rust, `rustfmt`, and `clippy`.
2. **Build**: `cargo build --release`
3. **Test**: `cargo test --workspace`
4. **Run**: Start the daemon with `cargo run --bin lssd` and query with `lssctl`.

## OpenCode Integration

The OpenCode plugin lives as a separate sibling project at `../lss-plug-opencode`.
It talks to `lssd` directly over the Unix socket protocol; no native FFI library is
required.

Add the plugin package path to your OpenCode config:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "plugin": [
    "opencode-helicone-session",
    "/absolute/path/to/lss-plug-opencode"
  ]
}
```

The plugin resolves the daemon socket from `LSS_SOCKET`, then
`$XDG_RUNTIME_DIR/lss/lssd.sock`, then `/run/user/<uid>/lss/lssd.sock`, and finally
`~/.cache/runtime/lss/lssd.sock`. Once loaded, OpenCode gets tools for search,
status, roots, ping, refresh, and doctor diagnostics.
