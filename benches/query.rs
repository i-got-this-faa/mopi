#![allow(clippy::unwrap_used)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lss_index_lexical::LexicalStore;
use lss_index_vector::index::VectorIndex;
use lss_rank::combine_and_rank;
use lss_test_support::corpus::{CorpusBuilder, CorpusProfile};
use lss_types::{DocumentId, MatchReason, SearchQuery, SearchResult};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Duration;

fn bench_lexical_query(c: &mut Criterion) {
    let root = tempfile::tempdir().unwrap();
    let utf8_root = camino::Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
    let builder = CorpusBuilder::new(CorpusProfile::Small, 42, utf8_root);
    let manifest = builder.generate().unwrap();

    let index_dir = tempfile::tempdir().unwrap();
    let index_path = camino::Utf8PathBuf::from_path_buf(index_dir.path().to_path_buf()).unwrap();
    let mut store = LexicalStore::open(&index_path).unwrap();

    for file in &manifest.files {
        let content = std::fs::read_to_string(file.path.as_std_path()).unwrap();
        let filename = file.path.file_name().unwrap_or("unknown");
        let ext = file.path.extension().unwrap_or("");
        store
            .add_document(
                file.path.as_str(),
                file.path.as_str(),
                &[],
                filename,
                &content,
                Some(ext),
                None,
            )
            .unwrap();
    }
    store.commit().unwrap();

    let mut group = c.benchmark_group("query/lexical");
    for term in &["quantum", "nebula", "cascade"] {
        let query = SearchQuery::new(term.to_string());
        group.bench_function(*term, |b| {
            b.iter(|| store.search(black_box(&query)).unwrap());
        });
    }
    group.finish();
}

fn bench_vector_search(c: &mut Criterion) {
    const DIMENSION: usize = 384;
    const NUM_VECTORS: usize = 10_000;

    let mut vector_index = VectorIndex::new(DIMENSION, 16);
    let mut rng = StdRng::seed_from_u64(42);

    let chunks: Vec<_> = (0..NUM_VECTORS)
        .map(|i| {
            let vec: Vec<f32> = (0..DIMENSION).map(|_| rng.random()).collect();
            (i, vec)
        })
        .collect();
    vector_index.upsert_chunks(&chunks).unwrap();

    let query_vec: Vec<f32> = (0..DIMENSION).map(|_| rng.random()).collect();

    let mut group = c.benchmark_group("query/vector");
    group.bench_function("hnsw_10k", |b| {
        b.iter(|| vector_index.search(black_box(&query_vec), 20).unwrap());
    });
    group.finish();
}

fn bench_rank_rrf(c: &mut Criterion) {
    const LIMIT: usize = 20;
    const LEXICAL_COUNT: usize = 50;
    const SEMANTIC_COUNT: usize = 30;

    let lexical_hits: Vec<SearchResult> = (0..LEXICAL_COUNT)
        .map(|i| SearchResult {
            document_id: DocumentId::new(),
            path: camino::Utf8PathBuf::from(format!("/doc/{i}.txt")),
            title: format!("doc {i}"),
            snippet: "some content".to_string(),
            score: 1.0 / (60.0 + i as f32),
            reasons: vec![MatchReason::Content],
        })
        .collect();

    let semantic_hits: Vec<SearchResult> = (0..SEMANTIC_COUNT)
        .map(|i| SearchResult {
            document_id: DocumentId::new(),
            path: camino::Utf8PathBuf::from(format!("/doc/semantic_{i}.txt")),
            title: format!("semantic {i}"),
            snippet: "some content".to_string(),
            score: 1.0 / (60.0 + i as f32),
            reasons: vec![MatchReason::Semantic],
        })
        .collect();

    c.bench_function("query/rank/rrf_50_30", |b| {
        b.iter(|| {
            combine_and_rank(
                black_box(lexical_hits.clone()),
                black_box(semantic_hits.clone()),
                LIMIT,
            )
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(1));
    targets = bench_lexical_query, bench_vector_search, bench_rank_rrf
}
criterion_main!(benches);
