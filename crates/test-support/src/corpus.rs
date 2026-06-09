use camino::Utf8PathBuf;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

// ── Constants ──

const FILES_PER_BATCH: usize = 100;
const DEFAULT_CORPUS_KIND: CorpusFileKind = CorpusFileKind::Text;
const TEXT_INDEX_RATIO: f64 = 0.95;

const WORD_POOL: &[&str] = &[
    "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog", "hello", "world",
    "foo", "bar", "baz", "qux", "apple", "banana", "cherry", "date", "elderberry", "fig",
    "grape", "honey", "iris", "jasmine", "kiwi", "lemon", "mango", "nectarine", "orange",
    "pear", "quince", "raspberry", "strawberry", "tangerine", "ugli", "vanilla", "walnut",
    "xigua", "yam", "zucchini", "alpha", "beta", "gamma", "delta", "epsilon", "zeta",
    "eta", "theta", "iota", "kappa", "lambda", "mu", "nu", "xi", "omicron", "pi",
    "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega",
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    "red", "blue", "green", "yellow", "purple", "cyan", "magenta", "white", "black",
    "circle", "square", "triangle", "cube", "sphere", "cone", "cylinder", "pyramid",
    "river", "lake", "ocean", "mountain", "valley", "forest", "desert", "plains",
    "spring", "summer", "autumn", "winter", "morning", "evening", "night", "dawn",
    "light", "dark", "sound", "silence", "motion", "still", "warm", "cool", "soft",
    "hard", "smooth", "rough", "bright", "dim", "swift", "slow", "broad", "narrow",
    "north", "south", "east", "west", "left", "right", "front", "back", "center",
    "inside", "outside", "above", "below", "near", "far", "open", "closed", "full",
    "empty", "new", "old", "first", "last", "next", "previous", "current", "final",
    "valid", "invalid", "true", "false", "high", "low", "fast", "slow", "hot", "cold",
    "input", "output", "source", "target", "value", "key", "data", "info", "config",
    "setup", "init", "start", "stop", "pause", "resume", "create", "delete", "update",
    "read", "write", "open", "close", "begin", "end", "enter", "exit", "push", "pull",
    "send", "receive", "load", "save", "import", "export", "encode", "decode", "parse",
    "format", "merge", "split", "join", "sort", "filter", "map", "reduce", "invert",
    "connect", "disconnect", "attach", "detach", "mount", "unmount", "lock", "unlock",
    "enable", "disable", "allow", "deny", "grant", "revoke", "accept", "reject",
    "pass", "fail", "success", "error", "warn", "info", "debug", "trace", "fatal",
    "system", "process", "thread", "task", "job", "queue", "stack", "heap", "pool",
    "node", "leaf", "root", "branch", "path", "edge", "graph", "tree", "ring", "mesh",
];

const TARGET_TERMS: &[&str] = &[
    "quantum", "nebula", "horizon", "cascade", "vector", "prism", "axiom", "vertex",
    "pulse", "echo", "zenith", "nova", "orbit", "fusion", "helix", "photon", "plasma",
    "quasar", "radon", "solar", "tensor", "ultra", "vortex", "wave", "xenon", "yield",
    "zephyr", "astral", "binary", "cosmic", "digital", "eclipse", "flux", "gravity",
    "hybrid", "inertia", "joule", "kinetic", "lunar", "matrix", "neural", "optical",
];

// ── Corpus profile ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorpusProfile {
    Small,
    Medium,
    Large,
    Stress(usize),
}

impl CorpusProfile {
    pub fn file_count(&self) -> usize {
        match self {
            CorpusProfile::Small => 200,
            CorpusProfile::Medium => 10_000,
            CorpusProfile::Large => 100_000,
            CorpusProfile::Stress(n) => *n,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            CorpusProfile::Small => "small",
            CorpusProfile::Medium => "medium",
            CorpusProfile::Large => "large",
            CorpusProfile::Stress(_) => "stress",
        }
    }
}

// ── File kind ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorpusFileKind {
    Text,
    Config,
    Code,
}

impl CorpusFileKind {
    fn distribution_weights() -> &'static [(CorpusFileKind, f64)] {
        &[
            (CorpusFileKind::Text, 0.40),
            (CorpusFileKind::Config, 0.30),
            (CorpusFileKind::Code, 0.30),
        ]
    }

    fn extensions(&self) -> &'static [&'static str] {
        match self {
            CorpusFileKind::Text => &["txt"],
            CorpusFileKind::Config => &["toml", "json", "yaml"],
            CorpusFileKind::Code => &["rs", "py", "js", "go"],
        }
    }
}

// ── Corpus file entry ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusFile {
    pub path: Utf8PathBuf,
    pub kind: CorpusFileKind,
    pub bytes: u64,
    pub should_index: bool,
    pub expected_terms: Vec<String>,
    pub expected_failure: Option<String>,
}

// ── Expected counts ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusExpectedCounts {
    pub total_files: usize,
    pub indexed_files: usize,
    pub text_files: usize,
    pub config_files: usize,
    pub code_files: usize,
}

// ── Corpus manifest ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusManifest {
    pub profile: String,
    pub seed: u64,
    pub root: Utf8PathBuf,
    pub files: Vec<CorpusFile>,
    pub expected_counts: CorpusExpectedCounts,
}

impl CorpusManifest {
    pub fn write_to_file(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(io::Error::other)?;
        fs::write(path, json)
    }

    pub fn from_file(path: &Path) -> io::Result<Self> {
        let json = fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

// ── Builder ──

pub struct CorpusBuilder {
    profile: CorpusProfile,
    seed: u64,
    root: Utf8PathBuf,
}

impl CorpusBuilder {
    pub fn new(profile: CorpusProfile, seed: u64, root: Utf8PathBuf) -> Self {
        CorpusBuilder { profile, seed, root }
    }

    pub fn generate(&self) -> io::Result<CorpusManifest> {
        let mut rng = StdRng::seed_from_u64(self.seed);
        let count = self.profile.file_count();
        let mut files = Vec::with_capacity(count);

        let text_dir = self.root.join("text");
        let config_dir = self.root.join("config");
        let code_dir = self.root.join("code");

        let kind_weights = CorpusFileKind::distribution_weights();
        let mut cumulative = Vec::with_capacity(kind_weights.len());
        let mut cum = 0.0_f64;
        for (k, w) in kind_weights {
            cum += w;
            cumulative.push((k, cum));
        }

        let mut text_count: usize = 0;
        let mut config_count: usize = 0;
        let mut code_count: usize = 0;
        let mut indexed_count: usize = 0;

        for _ in 0..count {
            let kind = *pick_kind(&mut rng, &cumulative);
            let exts = kind.extensions();
            let ext = exts[rng.random_range(0..exts.len())];
            let should_index = rng.random_bool(TEXT_INDEX_RATIO);
            let num_terms = rng.random_range(1..=3);
            let terms: Vec<String> = (0..num_terms)
                .map(|_| TARGET_TERMS[rng.random_range(0..TARGET_TERMS.len())].to_string())
                .collect();

            let (dir, prefix, kind_counter) = match &kind {
                CorpusFileKind::Text => (&text_dir, "text", &mut text_count),
                CorpusFileKind::Config => (&config_dir, "config", &mut config_count),
                CorpusFileKind::Code => (&code_dir, "code", &mut code_count),
            };

            let batch = *kind_counter / FILES_PER_BATCH;
            let batch_dir = dir.join(format!("{:04}", batch));
            fs::create_dir_all(&batch_dir)?;

            let filename = format!("{}_{:06}.{}", prefix, kind_counter, ext);
            let path = batch_dir.join(&filename);
            let content = generate_content(&mut rng, kind, &terms);

            fs::write(&path, content.as_bytes())?;

            let utf8_path = Utf8PathBuf::from_path_buf(path.into()).map_err(|p| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("non-utf8 path: {}", p.display()),
                )
            })?;

            *kind_counter += 1;
            if should_index {
                indexed_count += 1;
            }

            files.push(CorpusFile {
                path: utf8_path,
                kind,
                bytes: content.len() as u64,
                should_index,
                expected_terms: terms,
                expected_failure: None,
            });
        }

        Ok(CorpusManifest {
            profile: self.profile.name().to_string(),
            seed: self.seed,
            root: self.root.clone(),
            expected_counts: CorpusExpectedCounts {
                total_files: files.len(),
                indexed_files: indexed_count,
                text_files: files.iter().filter(|f| f.kind == CorpusFileKind::Text).count(),
                config_files: files.iter().filter(|f| f.kind == CorpusFileKind::Config).count(),
                code_files: files.iter().filter(|f| f.kind == CorpusFileKind::Code).count(),
            },
            files,
        })
    }
}

// ── Helper: pick kind from cumulative weights ──
// cumulative sums to 1.0 and rng.random() returns [0, 1),
// so this always matches in practice.

fn pick_kind<'a>(
    rng: &mut StdRng,
    cumulative: &[(&'a CorpusFileKind, f64)],
) -> &'a CorpusFileKind {
    let roll: f64 = rng.random();
    for (kind, upper) in cumulative {
        if roll < *upper {
            return kind;
        }
    }
    // rng.random() < 1.0 always, and cumulative ends at 1.0,
    // so the loop always returns. This fallback handles an empty slice.
    cumulative
        .last()
        .map(|(k, _)| *k)
        .unwrap_or(&DEFAULT_CORPUS_KIND)
}

// ── Content generation ──

fn generate_content(rng: &mut StdRng, kind: CorpusFileKind, terms: &[String]) -> String {
    match kind {
        CorpusFileKind::Text => generate_text_content(rng, terms),
        CorpusFileKind::Config => generate_config_content(rng, terms),
        CorpusFileKind::Code => generate_code_content(rng, terms),
    }
}

fn generate_text_content(rng: &mut StdRng, terms: &[String]) -> String {
    let mut content = String::new();

    let title_term = terms.first().map(|s| s.as_str()).unwrap_or("document");
    content.push_str(&format!("Title: {}\n\n", capitalize_first(title_term)));

    // Explicitly mention each term to guarantee presence in output
    if !terms.is_empty() {
        let mention = terms.join(", ");
        content.push_str(&format!("This document discusses {} and related topics. ", mention));
    }

    let num_paragraphs = rng.random_range(3..=5);
    for _ in 0..num_paragraphs {
        let num_sentences = rng.random_range(3..=7);
        for s in 0..num_sentences {
            let num_words = rng.random_range(5..=15);
            let mut sentence_words: Vec<String> = (0..num_words)
                .map(|_| {
                    if rng.random_bool(0.1) && !terms.is_empty() {
                        terms[rng.random_range(0..terms.len())].clone()
                    } else {
                        WORD_POOL[rng.random_range(0..WORD_POOL.len())].to_string()
                    }
                })
                .collect();

            if let Some(first) = sentence_words.first_mut() {
                *first = capitalize_first(first);
            }
            let sentence = sentence_words.join(" ");
            content.push_str(&sentence);
            if s < num_sentences - 1 {
                content.push_str(". ");
            } else {
                content.push('.');
            }
        }
        content.push('\n');
        content.push('\n');
    }

    content
}

fn generate_config_content(rng: &mut StdRng, terms: &[String]) -> String {
    let ext_idx = rng.random_range(0..3);
    match ext_idx {
        0 => generate_toml_config(rng, terms),
        1 => generate_json_config(rng, terms),
        _ => generate_yaml_config(rng, terms),
    }
}

fn generate_toml_config(rng: &mut StdRng, terms: &[String]) -> String {
    let term = terms.first().map(|s| s.as_str()).unwrap_or("setting");
    let mut content = String::new();

    content.push_str(&format!("[{}]\n", term));
    content.push_str(&format!("enabled = {}\n", if rng.random_bool(0.5) { "true" } else { "false" }));
    content.push_str(&format!("threshold = {}\n", rng.random_range(1..=100)));
    content.push_str(&format!("name = \"{}\"\n", term));
    content.push('\n');

    if let Some(t) = terms.get(1) {
        content.push_str(&format!("# {} tuning parameters\n", t));
        content.push_str(&format!("{}_mode = \"auto\"\n", t));
        content.push('\n');
    }

    if let Some(t) = terms.get(2) {
        content.push_str(&format!("[{}]\n", t));
        content.push_str("enabled = true\n");
        content.push('\n');
    }

    content.push_str("[database]\n");
    content.push_str("host = \"localhost\"\n");
    content.push_str(&format!("port = {}\n", rng.random_range(1024..=65535)));
    content.push_str("pool_size = 10\n");
    content.push('\n');

    content.push_str("[logging]\n");
    content.push_str("level = \"info\"\n");
    content.push_str("file = \"/var/log/app.log\"\n");

    content
}

fn generate_json_config(rng: &mut StdRng, terms: &[String]) -> String {
    let term = terms.first().map(|s| s.as_str()).unwrap_or("setting");
    let mut map = serde_json::Map::new();

    map.insert(
        term.to_string(),
        serde_json::json!({
            "enabled": rng.random_bool(0.5),
            "threshold": rng.random_range(1..=100),
            "name": term,
        }),
    );

    for t in terms.iter().skip(1) {
        map.insert(
            format!("{}_config", t),
            serde_json::json!({
                "enabled": true,
                "mode": "auto",
            }),
        );
    }

    map.insert(
        "database".to_string(),
        serde_json::json!({
            "host": "localhost",
            "port": rng.random_range(1024..=65535),
            "pool_size": 10,
        }),
    );
    map.insert(
        "logging".to_string(),
        serde_json::json!({
            "level": "info",
            "file": "/var/log/app.log",
        }),
    );

    serde_json::to_string_pretty(&map).unwrap_or_else(|_| "{}".to_string())
}

fn generate_yaml_config(rng: &mut StdRng, terms: &[String]) -> String {
    let term = terms.first().map(|s| s.as_str()).unwrap_or("setting");
    let mut content = String::new();

    content.push_str(&format!("{}:\n", term));
    content.push_str(&format!("  enabled: {}\n", if rng.random_bool(0.5) { "true" } else { "false" }));
    content.push_str(&format!("  threshold: {}\n", rng.random_range(1..=100)));
    content.push_str(&format!("  name: \"{}\"\n", term));
    content.push('\n');

    for t in terms.iter().skip(1) {
        content.push_str(&format!("{}_config:\n", t));
        content.push_str("  enabled: true\n");
        content.push_str("  mode: \"auto\"\n");
        content.push('\n');
    }

    content.push_str("database:\n");
    content.push_str("  host: localhost\n");
    content.push_str(&format!("  port: {}\n", rng.random_range(1024..=65535)));
    content.push_str("  pool_size: 10\n");
    content.push('\n');

    content.push_str("logging:\n");
    content.push_str("  level: info\n");
    content.push_str("  file: \"/var/log/app.log\"\n");

    content
}

fn generate_code_content(rng: &mut StdRng, terms: &[String]) -> String {
    let lang_idx = rng.random_range(0..4);
    match lang_idx {
        0 => generate_rust_code(rng, terms),
        1 => generate_python_code(rng, terms),
        2 => generate_javascript_code(rng, terms),
        _ => generate_go_code(rng, terms),
    }
}

fn generate_rust_code(rng: &mut StdRng, terms: &[String]) -> String {
    let term = terms.first().map(|s| s.as_str()).unwrap_or("compute");
    let mut code = String::new();

    code.push_str("/// Perform a calculation.\n");
    code.push_str(&format!("pub fn compute_{}(input: i32) -> i32 {{\n", term));
    code.push_str(&format!("    // Using {} algorithm\n", term));
    code.push_str("    let factor = 2;\n");
    code.push_str("    let result = input * factor;\n");
    if rng.random_bool(0.5) {
        code.push_str("    tracing::info!(\"computed {}\", result);\n");
    }
    code.push_str("    result\n");
    code.push_str("}\n");

    for t in terms.iter().skip(1) {
        code.push('\n');
        code.push_str(&format!("/// Helper for {} processing.\n", t));
        code.push_str(&format!("pub fn process_{}(data: &[u8]) -> Vec<u8> {{\n", t));
        code.push_str("    data.to_vec()\n");
        code.push_str("}\n");
    }

    code.push('\n');
    code.push_str("#[cfg(test)]\n");
    code.push_str("mod tests {\n");
    code.push_str("    use super::*;\n");
    code.push('\n');
    code.push_str("    #[test]\n");
    code.push_str(&format!("    fn test_{}_compute() {{\n", term));
    code.push_str(&format!("        assert_eq!(compute_{}(21), 42);\n", term));
    code.push_str("    }\n");
    code.push_str("}\n");

    code
}

fn generate_python_code(rng: &mut StdRng, terms: &[String]) -> String {
    let term = terms.first().map(|s| s.as_str()).unwrap_or("compute");
    let mut code = String::new();

    code.push_str("#!/usr/bin/env python3\n");
    code.push_str("import sys\n");
    code.push('\n');
    code.push_str(&format!("def compute_{}(input_val: int) -> int:\n", term));
    code.push_str("    \"\"\"Perform a calculation.\"\"\"\n");
    code.push_str(&format!("    # Using {} algorithm\n", term));
    code.push_str("    factor = 2\n");
    code.push_str("    result = input_val * factor\n");
    if rng.random_bool(0.5) {
        code.push_str("    print(f\"computed {result}\")\n");
    }
    code.push_str("    return result\n");

    for t in terms.iter().skip(1) {
        code.push('\n');
        code.push_str(&format!("def process_{}(data: list) -> list:\n", t));
        code.push_str(&format!("    \"\"\"Process {} data.\"\"\"\n", t));
        code.push_str("    return data.copy()\n");
    }

    code.push('\n');
    code.push_str("if __name__ == \"__main__\":\n");
    code.push_str(&format!("    result = compute_{}(21)\n", term));
    code.push_str("    print(result)\n");

    code
}

fn generate_javascript_code(rng: &mut StdRng, terms: &[String]) -> String {
    let term = terms.first().map(|s| s.as_str()).unwrap_or("compute");
    let mut code = String::new();

    code.push_str("'use strict';\n");
    code.push('\n');
    code.push_str(&format!("/** Perform a {} calculation */\n", term));
    code.push_str(&format!("function compute{}(input) {{\n", capitalize_first(term)));
    code.push_str(&format!("    // Using {} algorithm\n", term));
    code.push_str("    const factor = 2;\n");
    code.push_str("    const result = input * factor;\n");
    if rng.random_bool(0.5) {
        code.push_str("    console.log(`computed ${result}`);\n");
    }
    code.push_str("    return result;\n");
    code.push_str("}\n");

    for t in terms.iter().skip(1) {
        code.push('\n');
        code.push_str(&format!("/** Process {} data. */\n", t));
        code.push_str(&format!("function process{}(data) {{\n", capitalize_first(t)));
        code.push_str("    return data.slice();\n");
        code.push_str("}\n");
    }

    code.push('\n');
    code.push_str("module.exports = { compute");
    code.push_str(capitalize_first(term).as_str());
    for t in terms.iter().skip(1) {
        code.push_str(", process");
        code.push_str(capitalize_first(t).as_str());
    }
    code.push_str(" };\n");

    code
}

fn generate_go_code(rng: &mut StdRng, terms: &[String]) -> String {
    let term = terms.first().map(|s| s.as_str()).unwrap_or("compute");
    let mut code = String::new();

    code.push_str("package main\n");
    code.push('\n');
    code.push_str("import \"fmt\"\n");
    code.push('\n');
    code.push_str(&format!(
        "// Compute{} performs a calculation.\n",
        capitalize_first(term)
    ));
    code.push_str(&format!(
        "func Compute{}(input int) int {{\n",
        capitalize_first(term)
    ));
    code.push_str(&format!("\t// Using {} algorithm\n", term));
    code.push_str("\tfactor := 2\n");
    code.push_str("\tresult := input * factor\n");
    if rng.random_bool(0.5) {
        code.push_str("\tfmt.Printf(\"computed %d\\n\", result)\n");
    }
    code.push_str("\treturn result\n");
    code.push_str("}\n");

    for t in terms.iter().skip(1) {
        code.push('\n');
        code.push_str(&format!(
            "// Process{} handles {} data.\n",
            capitalize_first(t),
            t
        ));
        code.push_str(&format!(
            "func Process{}(data []byte) []byte {{\n",
            capitalize_first(t)
        ));
        code.push_str("\treturn append([]byte{}, data...)\n");
        code.push_str("}\n");
    }

    code.push('\n');
    code.push_str("func main() {\n");
    code.push_str(&format!("\tCompute{}(21)\n", capitalize_first(term)));
    code.push_str("}\n");

    code
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    fn small_builder() -> (CorpusBuilder, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .expect("temp path is utf-8");
        let builder = CorpusBuilder::new(CorpusProfile::Small, 42, root.clone());
        (builder, dir)
    }

    #[test]
    fn manifest_has_correct_profile() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation should succeed");
        assert_eq!(manifest.profile, "small");
    }

    #[test]
    fn manifest_has_correct_file_count() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation should succeed");
        assert_eq!(manifest.files.len(), CorpusProfile::Small.file_count());
        assert_eq!(
            manifest.expected_counts.total_files,
            CorpusProfile::Small.file_count()
        );
    }

    #[test]
    fn all_files_exist_on_disk() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation should succeed");
        for file in &manifest.files {
            let path = Path::new(file.path.as_str());
            assert!(path.exists(), "file should exist: {}", file.path);
            assert!(path.is_file(), "should be a file: {}", file.path);
        }
    }

    #[test]
    fn content_contains_expected_terms() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation should succeed");
        for file in &manifest.files {
            if file.should_index {
                let content = fs::read_to_string(Path::new(file.path.as_str()))
                    .expect("readable file");
                for term in &file.expected_terms {
                    assert!(
                        content.contains(term.as_str()),
                        "file {} should contain term '{}'",
                        file.path,
                        term
                    );
                }
            }
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .expect("temp path is utf-8");
        let builder_a = CorpusBuilder::new(CorpusProfile::Small, 42, root.join("a"));
        let builder_b = CorpusBuilder::new(CorpusProfile::Small, 42, root.join("b"));

        let manifest_a = builder_a.generate().expect("generation a");
        let manifest_b = builder_b.generate().expect("generation b");

        assert_eq!(manifest_a.files.len(), manifest_b.files.len());
        for (fa, fb) in manifest_a.files.iter().zip(manifest_b.files.iter()) {
            assert_eq!(
                fa.bytes, fb.bytes,
                "same bytes for kind {:?} at {}",
                fa.kind, fa.path
            );
            assert_eq!(
                fa.expected_terms, fb.expected_terms,
                "same terms for kind {:?} at {}",
                fa.kind, fa.path
            );
            assert_eq!(
                fa.should_index, fb.should_index,
                "same should_index for kind {:?} at {}",
                fa.kind, fa.path
            );
        }
    }

    #[test]
    fn different_seeds_produce_different_content() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root_a = Utf8PathBuf::from_path_buf(dir.path().join("a")).expect("path utf-8");
        let root_b = Utf8PathBuf::from_path_buf(dir.path().join("b")).expect("path utf-8");
        let builder_a = CorpusBuilder::new(CorpusProfile::Small, 1, root_a);
        let builder_b = CorpusBuilder::new(CorpusProfile::Small, 999, root_b);

        let manifest_a = builder_a.generate().expect("generation a");
        let manifest_b = builder_b.generate().expect("generation b");

        let all_same = manifest_a
            .files
            .iter()
            .zip(manifest_b.files.iter())
            .all(|(fa, fb)| fa.bytes == fb.bytes);
        assert!(
            !all_same,
            "different seeds should produce different file sizes"
        );
    }

    #[test]
    fn profile_names_and_counts() {
        assert_eq!(CorpusProfile::Small.name(), "small");
        assert_eq!(CorpusProfile::Small.file_count(), 200);
        assert_eq!(CorpusProfile::Medium.name(), "medium");
        assert_eq!(CorpusProfile::Medium.file_count(), 10_000);
        assert_eq!(CorpusProfile::Large.name(), "large");
        assert_eq!(CorpusProfile::Large.file_count(), 100_000);
        assert_eq!(CorpusProfile::Stress(500).name(), "stress");
        assert_eq!(CorpusProfile::Stress(500).file_count(), 500);
    }

    #[test]
    fn manifest_serialization_roundtrip() {
        let (builder, dir) = small_builder();
        let manifest = builder.generate().expect("generation");

        let file_path = dir.path().join("manifest.json");
        manifest.write_to_file(&file_path).expect("write manifest");
        assert!(file_path.exists(), "manifest file should exist");

        let loaded = CorpusManifest::from_file(&file_path).expect("load manifest");
        assert_eq!(manifest.profile, loaded.profile);
        assert_eq!(manifest.seed, loaded.seed);
        assert_eq!(manifest.files.len(), loaded.files.len());
    }

    #[test]
    fn expected_counts_match_actual() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation");

        let text_files: usize = manifest
            .files
            .iter()
            .filter(|f| matches!(f.kind, CorpusFileKind::Text))
            .count();
        let config_files: usize = manifest
            .files
            .iter()
            .filter(|f| matches!(f.kind, CorpusFileKind::Config))
            .count();
        let code_files: usize = manifest
            .files
            .iter()
            .filter(|f| matches!(f.kind, CorpusFileKind::Code))
            .count();
        let indexed_files: usize =
            manifest.files.iter().filter(|f| f.should_index).count();

        assert_eq!(manifest.expected_counts.text_files, text_files);
        assert_eq!(manifest.expected_counts.config_files, config_files);
        assert_eq!(manifest.expected_counts.code_files, code_files);
        assert_eq!(manifest.expected_counts.indexed_files, indexed_files);
    }

    #[test]
    fn all_text_files_are_txt() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation");
        for file in manifest
            .files
            .iter()
            .filter(|f| matches!(f.kind, CorpusFileKind::Text))
        {
            assert!(
                file.path.as_str().ends_with(".txt"),
                "text file should end with .txt: {}",
                file.path
            );
        }
    }

    #[test]
    fn config_files_have_known_extensions() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation");
        for file in manifest
            .files
            .iter()
            .filter(|f| matches!(f.kind, CorpusFileKind::Config))
        {
            let has_valid_ext = file.path.as_str().ends_with(".toml")
                || file.path.as_str().ends_with(".json")
                || file.path.as_str().ends_with(".yaml");
            assert!(
                has_valid_ext,
                "config file has known extension: {}",
                file.path
            );
        }
    }

    #[test]
    fn code_files_have_known_extensions() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation");
        for file in manifest
            .files
            .iter()
            .filter(|f| matches!(f.kind, CorpusFileKind::Code))
        {
            let has_valid_ext = file.path.as_str().ends_with(".rs")
                || file.path.as_str().ends_with(".py")
                || file.path.as_str().ends_with(".js")
                || file.path.as_str().ends_with(".go");
            assert!(
                has_valid_ext,
                "code file has known extension: {}",
                file.path
            );
        }
    }

    #[test]
    fn files_are_in_nested_batches() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation");
        let batch_dirs: std::collections::HashSet<&str> = manifest
            .files
            .iter()
            .map(|f| {
                let parent = Path::new(f.path.as_str())
                    .parent()
                    .expect("file has parent dir");
                parent
                    .file_name()
                    .expect("dir has name")
                    .to_str()
                    .expect("dir name is utf-8")
            })
            .collect();
        assert!(
            batch_dirs.contains("0000"),
            "batch 0000 should exist"
        );
    }

    #[test]
    fn stress_profile_respects_custom_count() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .expect("temp path is utf-8");
        let builder = CorpusBuilder::new(CorpusProfile::Stress(50), 42, root);
        let manifest = builder.generate().expect("generation");
        assert_eq!(manifest.files.len(), 50);
    }

    #[test]
    fn medium_profile_generates_correct_count() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .expect("temp path is utf-8");
        let builder = CorpusBuilder::new(CorpusProfile::Medium, 42, root);
        let manifest = builder.generate().expect("generation");
        assert_eq!(manifest.files.len(), 10_000);
    }

    #[test]
    fn generate_text_content_produces_valid_output() {
        let mut rng = StdRng::seed_from_u64(42);
        let terms = vec!["quantum".to_string(), "nebula".to_string()];
        let content = generate_text_content(&mut rng, &terms);
        assert!(!content.is_empty(), "text content should not be empty");
        assert!(
            content.contains("quantum") || content.contains("nebula"),
            "text should contain at least one target term"
        );
        assert!(
            content.starts_with("Title:"),
            "text should start with Title:"
        );
    }

    #[test]
    fn generate_config_content_produces_valid_output() {
        let mut rng = StdRng::seed_from_u64(42);
        let terms = vec!["quantum".to_string()];
        let content = generate_config_content(&mut rng, &terms);
        assert!(!content.is_empty(), "config content should not be empty");
        assert!(
            content.contains("quantum"),
            "config should contain target term"
        );
    }

    #[test]
    fn generate_code_content_produces_valid_output() {
        let mut rng = StdRng::seed_from_u64(42);
        let terms = vec!["quantum".to_string()];
        let content = generate_code_content(&mut rng, &terms);
        assert!(!content.is_empty(), "code content should not be empty");
        assert!(
            content.contains("quantum"),
            "code should contain target term"
        );
    }

    #[test]
    fn expected_terms_are_distinctive() {
        for term in TARGET_TERMS {
            assert!(
                !WORD_POOL.contains(term),
                "target term '{}' should not be in word pool",
                term
            );
        }
    }

    #[test]
    fn manifest_expected_counts_have_no_overflow() {
        let (builder, _dir) = small_builder();
        let manifest = builder.generate().expect("generation");
        let counted = manifest.expected_counts.text_files
            + manifest.expected_counts.config_files
            + manifest.expected_counts.code_files;
        assert_eq!(
            counted,
            manifest.files.len(),
            "sum of kind-specific counts should equal total"
        );
    }
}
