# Query, Ranking, And Filters

Overall Status: DONE  
Current Owner: UNASSIGNED  
Blocked By: Daemon and IPC, extraction and normalization, storage and indexing, embeddings and vector search  
Last Updated: 2026-04-27

## Objective

Implement the query parser, lexical and semantic retrieval orchestration, soft metadata filters, hybrid score fusion, file-level reranking, and user-visible snippets and match explanations.

## Scope

- free-text query parsing
- soft metadata filters such as `filetype:` and `name:`
- lexical retrieval
- semantic retrieval
- fusion and reranking
- snippets, highlights, and explanation data

## Query Semantics

- [ ] Unqualified terms primarily express content intent.
- [ ] Filename and path signals remain important boosts.
- [ ] `filetype:rs` is a soft preference by default.
- [ ] `name:main` is a soft preference by default.
- [ ] The system can later support strict filters, but strict mode is not the default behavior.
- [ ] Quoted phrases should be preserved for lexical phrase search when possible.

## Initial Filter Surface

- [ ] `filetype:`
- [ ] `name:`
- [ ] `path:`
- [ ] `ext:` as an alias or explicit field if needed

## Retrieval Pipeline Checklist

- [x] Parse the user query into free-text terms and metadata hints.
- [x] Run lexical retrieval against content, filename, alias filename, path, and alias path fields.
- [x] Run semantic retrieval against embedded chunks.
- [x] Union the candidate set.
- [x] Normalize scores across retrieval channels.
- [x] Rerank at the file level using the best chunk and aggregate evidence.
- [x] Generate snippets from the highest-value matching region.
- [x] Return reason flags explaining why each result surfaced.

## Ranking Checklist

- [ ] Boost exact filename matches.
- [ ] Boost basename and prefix filename matches.
- [ ] Boost path matches, but less than filename and content.
- [ ] Treat soft metadata filters as strong boosts rather than hard gates.
- [ ] Aggregate multiple strong chunk hits within the same file.
- [ ] Penalize low-information chunks and boilerplate-heavy matches.
- [ ] Keep ranking weights configurable.

## Snippet And Explanation Checklist

- [ ] Preserve enough extraction structure to produce readable snippets.
- [ ] Prefer snippet windows around exact lexical matches when they exist.
- [ ] Fall back to semantically matched chunk previews when lexical anchors are weak.
- [ ] Include concise reason tags for UI and CLI output.

## Acceptance Criteria

- [ ] Content relevance is visibly primary in mixed-corpus search results.
- [ ] Exact or near-exact name matches are still easy to find.
- [ ] `filetype:` and `name:` materially influence ranking without unexpectedly hiding good content matches.
- [ ] Hybrid search outperforms lexical-only search on representative semantic queries.
- [ ] Snippets are readable and help users decide whether a result is correct.

## Verification

- [ ] Run golden-query tests with expected top-result sets.
- [ ] Compare ranking outcomes for exact filename, content phrase, and semantic intent queries.
- [ ] Validate soft filter behavior on mixed filetype corpora.
- [ ] Validate explanation tags in both CLI and GUI output.

## Notes And Risks

- Overweighting filename can turn this into a launcher rather than a document search engine. The product direction is content-first.
- Underweighting filename can frustrate users typing direct file intents. Tune with real corpora, not gut feel.
