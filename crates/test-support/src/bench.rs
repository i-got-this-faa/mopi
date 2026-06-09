use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Corpus manifest summary ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusManifestSummary {
    pub profile: String,
    pub seed: u64,
    pub total_files: usize,
    pub indexed_files: usize,
}

// ── Benchmark environment ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEnvironment {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub cpu_count: u32,
    pub memory_mb: Option<u64>,
}

impl BenchmarkEnvironment {
    pub fn capture() -> Self {
        let hostname = Self::capture_hostname();
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        // memory info is platform-specific; leave as None
        BenchmarkEnvironment {
            hostname,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_count,
            memory_mb: None,
        }
    }

    fn capture_hostname() -> String {
        std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout).ok()
                } else {
                    None
                }
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

// ── Benchmark metric ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetric {
    pub name: String,
    pub unit: String,
    pub p50: Option<f64>,
    pub p95: Option<f64>,
    pub p99: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub total: Option<f64>,
    pub count: u64,
}

impl BenchmarkMetric {
    pub fn new(name: impl Into<String>, unit: impl Into<String>) -> Self {
        BenchmarkMetric {
            name: name.into(),
            unit: unit.into(),
            p50: None,
            p95: None,
            p99: None,
            min: None,
            max: None,
            total: None,
            count: 0,
        }
    }
}

// ── Benchmark failure ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkFailure {
    pub name: String,
    pub file: Option<String>,
    pub reason: String,
    pub recoverable: bool,
}

// ── Benchmark report ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub run_id: String,
    pub started_at_unix: i64,
    pub git_revision: Option<String>,
    pub corpus: CorpusManifestSummary,
    pub config: serde_json::Value,
    pub environment: BenchmarkEnvironment,
    pub metrics: Vec<BenchmarkMetric>,
    pub failures: Vec<BenchmarkFailure>,
}

impl BenchmarkReport {
    pub fn new(
        run_id: String,
        corpus: CorpusManifestSummary,
        config: serde_json::Value,
    ) -> Self {
        let started_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let git_revision = Self::detect_git_revision();

        BenchmarkReport {
            run_id,
            started_at_unix,
            git_revision,
            corpus,
            config,
            environment: BenchmarkEnvironment::capture(),
            metrics: Vec::new(),
            failures: Vec::new(),
        }
    }

    fn detect_git_revision() -> Option<String> {
        std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout).ok()
                } else {
                    None
                }
            })
            .map(|s| s.trim().to_string())
    }

    // ── Serialization ──

    pub fn write_json(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(io::Error::other)?;
        fs::write(path, json)
    }

    pub fn write_markdown(&self, path: &Path) -> io::Result<()> {
        let md = self.render_markdown();
        fs::write(path, md)
    }

    pub fn from_json(path: &Path) -> io::Result<Self> {
        let json = fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(io::Error::other)
    }

    // ── Markdown rendering ──

    fn render_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# Benchmark Report\n\n");

        md.push_str(&format!("- **Run ID**: {}\n", self.run_id));
        md.push_str(&format!("- **Started**: {}\n", Self::format_timestamp(self.started_at_unix)));
        if let Some(rev) = &self.git_revision {
            md.push_str(&format!("- **Git Revision**: `{}`\n", rev));
        }
        md.push('\n');

        md.push_str("## Corpus\n\n");
        md.push_str(&format!(
            "- Profile: {}, seed={}, {} files ({} indexed)\n\n",
            self.corpus.profile, self.corpus.seed, self.corpus.total_files, self.corpus.indexed_files
        ));

        md.push_str("## Environment\n\n");
        md.push_str(&format!("- **Hostname**: {}\n", self.environment.hostname));
        md.push_str(&format!("- **OS**: {} ({})\n", self.environment.os, self.environment.arch));
        md.push_str(&format!("- **CPU Count**: {}\n", self.environment.cpu_count));
        if let Some(mem) = self.environment.memory_mb {
            md.push_str(&format!("- **Memory**: {} MB\n", mem));
        }
        md.push('\n');

        if !self.metrics.is_empty() {
            md.push_str("## Metrics\n\n");
            md.push_str("| Name | Unit | p50 | p95 | p99 | Min | Max | Total | Count |\n");
            md.push_str("|------|------|-----|-----|-----|-----|-----|-------|-------|\n");
            for m in &self.metrics {
                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                    m.name,
                    m.unit,
                    format_opt(m.p50),
                    format_opt(m.p95),
                    format_opt(m.p99),
                    format_opt(m.min),
                    format_opt(m.max),
                    format_opt(m.total),
                    m.count,
                ));
            }
            md.push('\n');
        }

        if !self.failures.is_empty() {
            md.push_str("## Failures\n\n");
            for f in &self.failures {
                md.push_str(&format!(
                    "- **{}**{}: {} {}\n",
                    f.name,
                    f.file.as_ref().map(|p| format!(" (`{}`)", p)).unwrap_or_default(),
                    f.reason,
                    if f.recoverable { "(recoverable)" } else { "(fatal)" },
                ));
            }
            md.push('\n');
        }

        md
    }

    fn format_timestamp(unix: i64) -> String {
        // Simple ISO-8601-like format without pulling in chrono
        let secs = if unix >= 0 {
            unix as u64
        } else {
            0u64
        };
        let days = secs / 86400;
        let time_secs = secs % 86400;
        let hours = time_secs / 3600;
        let minutes = (time_secs % 3600) / 60;
        let seconds = time_secs % 60;

        // Approximate year from days since epoch
        let year = 1970 + (days as f64 / 365.25) as u64;
        // This is an approximation; for exact dates we'd need chrono
        // But it's good enough for a report header
        format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, 1, 1, hours, minutes, seconds)
    }
}

fn format_opt(v: Option<f64>) -> String {
    match v {
        Some(n) => format!("{:.3}", n),
        None => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> BenchmarkReport {
        let corpus = CorpusManifestSummary {
            profile: "small".to_string(),
            seed: 42,
            total_files: 200,
            indexed_files: 190,
        };
        let config = serde_json::json!({
            "max_file_bytes": 10485760,
            "enable_ocr": false,
        });
        let mut report = BenchmarkReport::new("test-run".to_string(), corpus, config);
        report.metrics.push(BenchmarkMetric {
            name: "crawl_throughput".to_string(),
            unit: "files/sec".to_string(),
            p50: Some(1500.0),
            p95: Some(2000.0),
            p99: Some(2200.0),
            min: Some(1200.0),
            max: Some(2500.0),
            total: Some(300000.0),
            count: 200,
        });
        report
    }

    #[test]
    fn report_has_run_id() {
        let report = sample_report();
        assert_eq!(report.run_id, "test-run");
    }

    #[test]
    fn report_has_timestamp() {
        let report = sample_report();
        assert!(report.started_at_unix > 0, "timestamp should be set");
    }

    #[test]
    fn report_serialization_roundtrip() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("report.json");
        let report = sample_report();
        report.write_json(&path).expect("write json");
        let loaded = BenchmarkReport::from_json(&path).expect("load json");
        assert_eq!(loaded.run_id, report.run_id);
        assert_eq!(loaded.corpus.profile, report.corpus.profile);
        assert_eq!(loaded.metrics.len(), report.metrics.len());
    }

    #[test]
    fn markdown_output_is_non_empty() {
        let report = sample_report();
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("report.md");
        report.write_markdown(&path).expect("write markdown");
        let content = fs::read_to_string(&path).expect("read markdown");
        assert!(!content.is_empty(), "markdown should be non-empty");
        assert!(content.contains("Benchmark Report"), "should have title");
        assert!(content.contains("test-run"), "should contain run id");
    }

    #[test]
    fn markdown_includes_metrics_table() {
        let report = sample_report();
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("report.md");
        report.write_markdown(&path).expect("write markdown");
        let content = fs::read_to_string(&path).expect("read markdown");
        assert!(content.contains("crawl_throughput"), "should contain metric name");
        assert!(content.contains("1500.000"), "should contain p50 value");
    }

    #[test]
    fn metric_defaults_to_none() {
        let m = BenchmarkMetric::new("test", "ms");
        assert_eq!(m.name, "test");
        assert_eq!(m.unit, "ms");
        assert!(m.p50.is_none());
        assert_eq!(m.count, 0);
    }

    #[test]
    fn report_with_failures_includes_them() {
        let mut report = sample_report();
        report.failures.push(BenchmarkFailure {
            name: "pdf_extraction".to_string(),
            file: Some("doc.pdf".to_string()),
            reason: "page limit exceeded".to_string(),
            recoverable: true,
        });
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("report.json");
        report.write_json(&path).expect("write json");
        let loaded = BenchmarkReport::from_json(&path).expect("load json");
        assert_eq!(loaded.failures.len(), 1);
        assert_eq!(loaded.failures[0].name, "pdf_extraction");
    }

    #[test]
    fn environment_capture_does_not_panic() {
        let env = BenchmarkEnvironment::capture();
        assert!(!env.hostname.is_empty(), "hostname should be captured");
        assert!(!env.os.is_empty(), "os should be non-empty");
        assert!(env.cpu_count > 0, "cpu count should be positive");
    }

    #[test]
    fn markdown_includes_failures_section() {
        let mut report = sample_report();
        report.failures.push(BenchmarkFailure {
            name: "ocr_timeout".to_string(),
            file: None,
            reason: "tesseract not found".to_string(),
            recoverable: true,
        });
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("report.md");
        report.write_markdown(&path).expect("write markdown");
        let content = fs::read_to_string(&path).expect("read markdown");
        assert!(content.contains("ocr_timeout"), "should contain failure");
    }

    #[test]
    fn git_revision_is_optional() {
        // This test just verifies the function doesn't panic
        let _rev = BenchmarkReport::detect_git_revision();
    }
}
