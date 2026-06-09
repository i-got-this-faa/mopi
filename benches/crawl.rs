#![allow(clippy::unwrap_used)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lss_config::{AppConfig, RootConfig};
use lss_crawl::{discover_changes, discover_files};
use lss_test_support::corpus::{CorpusBuilder, CorpusProfile};

fn bench_crawl_small_corpus(c: &mut Criterion) {
    let root = tempfile::tempdir().unwrap();
    let utf8_root = camino::Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
    let builder = CorpusBuilder::new(CorpusProfile::Small, 42, utf8_root.clone());
    let manifest = builder.generate().unwrap();

    let mut config = AppConfig::default();
    config.roots.push(RootConfig {
        path: manifest.root.clone(),
    });

    c.bench_function("crawl/small/discover", |b| {
        b.iter(|| discover_files(black_box(&config)).unwrap());
    });

    let initial = discover_changes(&config, None).unwrap();

    c.bench_function("crawl/small/changes", |b| {
        b.iter(|| discover_changes(black_box(&config), Some(&initial.snapshot)).unwrap());
    });
}

criterion_group!(benches, bench_crawl_small_corpus);
criterion_main!(benches);
