use lss_types::{MatchReason, SearchResult};
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

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use lss_types::DocumentId;

    fn make_result(id: DocumentId) -> SearchResult {
        SearchResult {
            document_id: id,
            path: Utf8PathBuf::from("/test/path"),
            title: String::new(),
            snippet: String::new(),
            score: 0.0,
            reasons: Vec::new(),
        }
    }

    #[test]
    fn empty_inputs_return_empty() {
        let results: Vec<SearchResult> = combine_and_rank(vec![], vec![], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn lexical_only_results() {
        let id1 = DocumentId::new();
        let id2 = DocumentId::new();
        let results = combine_and_rank(vec![make_result(id1), make_result(id2)], vec![], 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].document_id, id1);
        assert_eq!(results[1].document_id, id2);
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn semantic_only_results() {
        let id1 = DocumentId::new();
        let id2 = DocumentId::new();
        let results = combine_and_rank(vec![], vec![make_result(id1), make_result(id2)], 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].document_id, id1);
        assert!(results[0].reasons.contains(&MatchReason::Semantic));
        assert!(results[1].reasons.contains(&MatchReason::Semantic));
    }

    #[test]
    fn overlapping_documents_accumulate_scores() {
        let shared = DocumentId::new();
        let lexical_only = DocumentId::new();
        let semantic_only = DocumentId::new();

        let results = combine_and_rank(
            vec![make_result(shared), make_result(lexical_only)],
            vec![make_result(shared), make_result(semantic_only)],
            10,
        );

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].document_id, shared);
        assert!(results[0].reasons.contains(&MatchReason::Semantic));
    }

    #[test]
    fn limit_truncates_results() {
        let ids: Vec<_> = (0..10).map(|_| DocumentId::new()).collect();
        let lexical: Vec<_> = ids.iter().map(|&id| make_result(id)).collect();
        let semantic: Vec<_> = ids.iter().map(|&id| make_result(id)).collect();

        let results = combine_and_rank(lexical, semantic, 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn rank_order_is_descending_by_score() {
        let ids: Vec<_> = (0..5).map(|_| DocumentId::new()).collect();
        let lexical: Vec<_> = ids.iter().map(|&id| make_result(id)).collect();
        let results = combine_and_rank(lexical, vec![], 10);

        for i in 0..results.len() - 1 {
            assert!(results[i].score >= results[i + 1].score);
        }
    }

    #[test]
    fn semantic_hits_get_semantic_reason() {
        let id = DocumentId::new();
        let results = combine_and_rank(vec![], vec![make_result(id)], 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].reasons, vec![MatchReason::Semantic]);
    }

    #[test]
    fn lexical_hits_do_not_get_semantic_reason() {
        let id = DocumentId::new();
        let results = combine_and_rank(vec![make_result(id)], vec![], 10);
        assert_eq!(results.len(), 1);
        assert!(!results[0].reasons.contains(&MatchReason::Semantic));
    }
}
