use mopi_types::{MatchReason, SearchResult};
use std::collections::HashMap;

pub fn combine_and_rank(
    lexical_hits: Vec<SearchResult>,
    semantic_hits: Vec<SearchResult>,
    limit: usize,
) -> Vec<SearchResult> {
    let mut fused = HashMap::new();

    let k = 60.0;

    for (rank, hit) in lexical_hits.into_iter().enumerate() {
        let score = 1.0 / (k + rank as f32);
        let id = hit.document_id;

        let mut hit = hit;
        hit.score = score;
        let entry = fused.entry(id).or_insert(hit);
        entry.score += score;
    }

    for (rank, mut hit) in semantic_hits.into_iter().enumerate() {
        let score = 1.0 / (k + rank as f32);
        let id = hit.document_id;

        if let Some(entry) = fused.get_mut(&id) {
            entry.score += score;
            if !entry.reasons.contains(&MatchReason::Semantic) {
                entry.reasons.push(MatchReason::Semantic);
            }
        } else {
            hit.score = score;
            hit.reasons = vec![MatchReason::Semantic];
            fused.insert(id, hit);
        }
    }

    let mut results: Vec<_> = fused.into_values().collect();
    results.sort_by(|a, b| b.score.total_cmp(&a.score));

    if results.len() > limit {
        results.truncate(limit);
    }

    results
}
