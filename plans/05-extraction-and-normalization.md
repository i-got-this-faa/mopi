# Extraction And Normalization

Overall Status: DONE  
Current Owner: OpenCode  
Blocked By: Workspace scaffold, config and policy, crawl and discovery  
Last Updated: 2026-04-27

## Objective

Extract searchable text and cheap metadata from supported file formats quickly, safely, and consistently enough to feed lexical indexing, chunking, and semantic search.

## Scope

- [x] extractor trait and dispatcher
- [x] content-type sniffing
- [x] plain text extraction
- [x] config-file extraction
- [x] `docx` extraction
- [x] `odt` extraction
- [x] fast `pdf` extraction
- [x] normalization and warnings

## Required Formats

- [x] plain text
- [x] common config formats such as `toml`, `yaml`, `yml`, `json`, `jsonc`, `ini`, `env`, `xml`, `md`
- [x] code and source files treated as text when decodable
- [x] `docx`
- [x] `odt`
- [x] `pdf`

## Extraction Output Contract

Every extractor should produce a common structure with:

- [x] canonical path reference
- [x] observed path reference when needed
- [x] detected mime or format
- [x] extracted text
- [x] cheap metadata such as title, page count, author, language hint, or section info when available at low cost
- [x] warnings for truncation, malformed input, or partial extraction
- [x] stats such as bytes read and extraction duration

## Safety And Speed Requirements

- [x] Enforce byte, page, and runtime limits per extraction job.
- [x] Refuse obviously unsupported binary data early.
- [x] Never crash the daemon on malformed input.
- [x] Prefer speed over layout-perfect text recovery for PDFs.
- [x] No OCR in v1.
- [x] Log partial extraction warnings without treating them as fatal when enough text was recovered.

## Plain Text And Config Checklist

- [x] Detect decodable text safely.
- [x] Preserve original ordering and meaningful newlines.
- [x] Normalize line endings.
- [x] Strip NULs and control garbage that harms indexing.
- [x] Keep whitespace normalization conservative so phrase search is not destroyed.

## Office Document Checklist

- [x] Implement `docx` extraction from zipped XML content.
- [x] Implement `odt` extraction from zipped XML content.
- [x] Preserve paragraph boundaries where possible.
- [x] Capture cheap metadata when available without expensive extra passes.

## PDF Checklist

- [x] Choose a fast text extraction path.
- [x] Bound page count and extraction duration.
- [x] Capture per-page boundaries if cheap enough for snippet quality.
- [x] Avoid heavy layout reconstruction in v1.
- [x] Record when extraction quality is partial or degraded.

## Normalization Checklist

- [x] Normalize newlines.
- [x] Preserve Unicode text where valid.
- [x] Remove invalid replacement-heavy garbage when a file is clearly not usable text.
- [x] Keep normalized text suitable for both lexical indexing and chunking.
- [x] Preserve enough structure to generate user-friendly snippets later.

## Acceptance Criteria

- [x] All mandatory formats can be extracted through the common pipeline.
- [x] Unsupported or malformed files fail gracefully with recorded warnings or errors.
- [x] Text output quality is good enough for content search and chunking.
- [x] Extraction remains fast enough to keep indexing throughput within the project goals.

## Verification

- [x] Run fixture-based tests for every supported format.
- [x] Run malformed-file tests for every complex extractor.
- [x] Measure extraction latency distributions across a mixed corpus.
- [x] Confirm that extracted text supports exact phrase search on representative files.

## Notes And Risks

- PDF extraction quality is a classic trap. Stay speed-first and only expand scope if benchmarks show acceptable headroom.
- Do not over-normalize. Destroying structure for the sake of cleaner tokens will hurt snippet quality and exact matching.
