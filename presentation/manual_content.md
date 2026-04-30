# mopi: Presentation Content & Visual Plan

This document provides the text and visual strategies for each slide to assist in manual assembly.

---

## 1. Introduction & Problem Statement
**Text:**
- **The Latency Trap:** Cloud-based semantic search introduces 500ms+ network overhead, killing the "search-as-you-type" experience.
- **The Privacy Gap:** Users shouldn't have to send private documents to the cloud to get intelligent search results.
- **The Intelligence Ceiling:** Lexical search (keywords only) fails on synonyms and conceptual intent.

**Visual Plan:**
- A "Cloud vs. Local" comparison graphic. Cloud icon with a 500ms timer vs. Local PC icon with a 35ms lightning bolt and a lock.

---

## 2. Project Objectives
**Text:**
- **Fully Local Inference:** No API keys, no data leaks, 100% private.
- **Interactive Latency:** Target <50ms P95 latency for a "launcher-like" feel.
- **Universal Support:** Native support for Text, Code, PDF, DOCX, and ODT.
- **Modular Stability:** Built with high-performance Rust crates for reliability.

**Visual Plan:**
- Icons representing: Shield (Private), Stopwatch (Fast), File Stack (Diverse), and Gears (Modular).

---

## 3. Proposed Solution & Innovation
**Text:**
- **Hybrid Retrieval Engine:** Merges the precision of Tantivy with the recall of Vector Search.
- **Reciprocal Rank Fusion (RRF):** Mathematically merges scores to ensure the best results rise to the top.
- **Semantic Chunking:** Structure-aware splitting (headings/paragraphs) for better context preservation.

**Visual Plan:**
- A Venn Diagram. "Exact Keywords" on the left, "Semantic Meaning" on the right, and "mopi Hybrid Engine" in the center overlap.

---

## 4. Methodology (Technology Stack)
**Text:**
- **Language:** Rust (Performance & Memory Safety).
- **DL Runtime:** ONNX Runtime via **FastEmbed-RS**.
- **Search Core:** Tantivy (Lexical) + HNSW/ANN (Vector).
- **Metadata/IPC:** SQLite and Unix Domain Sockets.

**Visual Plan:**
- A "Logos" slide featuring the Rust crab, ONNX logo, SQLite logo, and Linux Tux.

---

## 5. System Architecture / Workflow
**Text:**
- **Crawl:** Discover files and detect changes.
- **Extract:** Normalize text from complex formats (PDF/Office).
- **Embed:** Generate vectors using local quantized models.
- **Index:** Sync metadata, lexical docs, and vectors.
- **Query:** Stream results instantly to the user.

**Visual Plan:**
- A clean horizontal flowchart showing the pipeline: CRAWL -> EXTRACT -> EMBED -> INDEX -> QUERY.

---

## 6. Implementation
**Text:**
- **Model:** `all-MiniLM-L6-v2` (384-dim) for optimal speed/accuracy.
- **Optimization:** INT8 quantization for 4x memory savings.
- **Concurrency:** Non-blocking worker pools for background indexing.

**Visual Plan:**
- A code snippet of a Rust `struct` or a diagram showing "Worker Pools" handling different file formats in parallel.

---

## 7. Results / Outcomes
**Text:**
- **Performance:** Sub-35ms total pipeline latency (P95).
- **Intelligence:** Higher recall than traditional tools on conceptual queries.
- **User Experience:** Enabled the "Kiwi" search-as-you-type GUI.

**Visual Plan:**
- A bar chart comparing Cloud Latency (Tall Bar) vs. mopi Latency (Tiny Bar).

---

## 8. Conclusion
**Text:**
- Privacy and Speed are no longer a trade-off.
- Rust and local ONNX models enable a new class of secure, intelligent desktop tools.
- **Future:** Cross-Encoder reranking and hardware acceleration (Metal/CUDA/Vulkan).

**Visual Plan:**
- A "Roadmap" arrow pointing toward "Hardware Acceleration" and "Cross-Encoders".

---

## 10. References
**Text:**
- **FastEmbed-RS:** qdrant.tech
- **Tantivy:** quickwit.io
- **ONNX Runtime:** onnxruntime.ai
- **Project Repo:** github.com/i-got-this-faa/mopi

**Visual Plan:**
- "Thank You" text in large font with the team name "OpenCode".
