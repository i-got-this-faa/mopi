#![allow(clippy::unwrap_used)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lss_config::AppConfig;
use lss_extract::Dispatcher;
use lss_test_support::corpus::{CorpusBuilder, CorpusProfile};
use std::time::Duration;

fn bench_extract_text_files(c: &mut Criterion) {
    let root = tempfile::tempdir().unwrap();
    let utf8_root = camino::Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
    let builder = CorpusBuilder::new(CorpusProfile::Small, 42, utf8_root);
    let manifest = builder.generate().unwrap();

    let config = AppConfig::default().extraction;
    let dispatcher = Dispatcher::new();

    // Collect text file paths
    let text_paths: Vec<_> = manifest
        .files
        .iter()
        .filter(|f| {
            matches!(f.kind, lss_test_support::corpus::CorpusFileKind::Text)
        })
        .map(|f| f.path.clone())
        .collect();

    c.bench_function("extract/text", |b| {
        b.iter(|| {
            for path in &text_paths {
                dispatcher.extract(black_box(path), &config).unwrap();
            }
        });
    });

    // Collect config file paths
    let config_paths: Vec<_> = manifest
        .files
        .iter()
        .filter(|f| {
            matches!(f.kind, lss_test_support::corpus::CorpusFileKind::Config)
        })
        .map(|f| f.path.clone())
        .collect();

    c.bench_function("extract/config", |b| {
        b.iter(|| {
            for path in &config_paths {
                dispatcher.extract(black_box(path), &config).unwrap();
            }
        });
    });

    // Collect code file paths
    let code_paths: Vec<_> = manifest
        .files
        .iter()
        .filter(|f| {
            matches!(f.kind, lss_test_support::corpus::CorpusFileKind::Code)
        })
        .map(|f| f.path.clone())
        .collect();

    c.bench_function("extract/code", |b| {
        b.iter(|| {
            for path in &code_paths {
                dispatcher.extract(black_box(path), &config).unwrap();
            }
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(1));
    targets = bench_extract_text_files
}
criterion_main!(benches);
