pub mod bench;
pub mod bench_env;
pub mod corpus;
pub mod fixtures;

use camino::Utf8PathBuf;
use std::fs;
use tempfile::{TempDir, tempdir};

pub fn temp_workspace() -> TempDir {
    tempdir().expect("temporary workspace should be created")
}

pub struct TestFile {
    pub path: Utf8PathBuf,
    pub content: String,
}

pub fn temp_root_with_files(files: &[(&str, &str)]) -> TempDir {
    let dir = temp_workspace();
    for (relative_path, content) in files {
        let full_path = dir.path().join(relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("parent dirs should be created");
        }
        fs::write(&full_path, content).expect("test file should be written");
    }
    dir
}

pub fn sample_text_file(name: &str) -> TestFile {
    TestFile {
        path: Utf8PathBuf::from(name),
        content: "The quick brown fox jumps over the lazy dog. \
                  This is a sample text file for testing purposes. \
                  It contains multiple sentences and enough content \
                  to exercise chunking and search functionality."
            .to_string(),
    }
}

pub fn sample_config_toml() -> TestFile {
    TestFile {
        path: Utf8PathBuf::from("config.toml"),
        content: r#"[database]
host = "localhost"
port = 5432
name = "test_db"

[server]
bind = "0.0.0.0"
port = 8080
workers = 4
"#
        .to_string(),
    }
}

pub fn sample_json_config() -> TestFile {
    TestFile {
        path: Utf8PathBuf::from("settings.json"),
        content: r#"{
    "theme": "dark",
    "font_size": 14,
    "auto_save": true,
    "plugins": ["syntax", "git", "linter"]
}
"#
        .to_string(),
    }
}

pub fn sample_code_file() -> TestFile {
    TestFile {
        path: Utf8PathBuf::from("main.rs"),
        content: r#"use std::collections::HashMap;

fn main() {
    let mut map = HashMap::new();
    map.insert("hello", "world");
    println!("{:?}", map);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
"#
        .to_string(),
    }
}

pub fn sample_markdown_file() -> TestFile {
    TestFile {
        path: Utf8PathBuf::from("README.md"),
        content: r#"# Project Title

This is a sample markdown file for testing.

## Features

- Feature one: semantic search
- Feature two: lexical search
- Feature three: hybrid ranking

## Usage

Run `lssctl query <terms>` to search.
"#
        .to_string(),
    }
}

pub fn sample_corpus() -> TempDir {
    temp_root_with_files(&[
        ("src/main.rs", sample_code_file().content.as_str()),
        ("docs/README.md", sample_markdown_file().content.as_str()),
        ("config/settings.json", sample_json_config().content.as_str()),
        ("config/app.toml", sample_config_toml().content.as_str()),
        ("notes/thoughts.txt", sample_text_file("thoughts.txt").content.as_str()),
        (".hidden/secret.txt", "this should be ignored by default"),
        ("data/report.csv", "name,value\nfoo,1\nbar,2\n"),
    ])
}

pub fn symlink_loop_fixture() -> TempDir {
    let dir = temp_workspace();
    let root = dir.path().join("root");
    fs::create_dir_all(&root).expect("root dir should be created");

    let a = root.join("a");
    let b = root.join("b");
    fs::create_dir_all(&a).expect("dir a should be created");
    fs::create_dir_all(&b).expect("dir b should be created");

    fs::write(a.join("file.txt"), "content in a").expect("file should be written");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&b, a.join("link_to_b")).expect("symlink should be created");
        std::os::unix::fs::symlink(&a, b.join("link_to_a")).expect("symlink should be created");
    }

    dir
}
