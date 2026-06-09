use crate::schema::LexicalSchema;
use camino::Utf8Path;
use tantivy::SnippetGenerator;
use tantivy::directory::error::OpenDirectoryError;
use tantivy::query::{BooleanQuery, Occur, QueryParser, QueryParserError};
use tantivy::schema::Value;
use tantivy::{Index, IndexWriter, ReloadPolicy, TantivyDocument};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LexicalError {
    #[error("tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("query parser error: {0}")]
    QueryParser(#[from] QueryParserError),
    #[error("open directory error: {0}")]
    OpenDirectory(#[from] OpenDirectoryError),
}

pub struct LexicalStore {
    index: Index,
    schema: LexicalSchema,
    writer: IndexWriter,
}

impl LexicalStore {
    pub fn open(path: &Utf8Path) -> Result<Self, LexicalError> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }

        let schema = LexicalSchema::new();
        let index = Index::open_or_create(
            tantivy::directory::MmapDirectory::open(path)?,
            schema.schema.clone(),
        )?;

        // Use a 50MB heap for indexing. We can make this configurable later.
        let writer = index.writer(50_000_000)?;

        Ok(Self {
            index,
            schema,
            writer,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_document(
        &self,
        id: &str,
        canonical_path: &str,
        alias_paths: &[&str],
        filename: &str,
        text: &str,
        extension: Option<&str>,
        mime: Option<&str>,
    ) -> Result<(), LexicalError> {
        let mut doc = TantivyDocument::default();
        doc.add_text(self.schema.id, id);
        doc.add_text(self.schema.canonical_path, canonical_path);
        for alias in alias_paths {
            doc.add_text(self.schema.alias_paths, *alias);
            if let Some(name) = Utf8Path::new(alias).file_name() {
                doc.add_text(self.schema.alias_filenames, name);
            }
        }
        doc.add_text(self.schema.filename, filename);
        doc.add_text(self.schema.content, text);

        if let Some(ext) = extension {
            doc.add_text(self.schema.extension, ext);
        }
        if let Some(m) = mime {
            doc.add_text(self.schema.mime, m);
        }

        self.writer.add_document(doc)?;
        Ok(())
    }

    pub fn delete_document(&mut self, id: &str) -> Result<(), LexicalError> {
        let term = tantivy::Term::from_field_text(self.schema.id, id);
        self.writer.delete_term(term);
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), LexicalError> {
        self.writer.commit()?;
        Ok(())
    }

    /// Full-text search returning matching document content for grep-style
    /// post-filtering. Returns `(id, canonical_path, content)` tuples.
    pub fn search_content(
        &self,
        pattern: &str,
        limit: usize,
    ) -> Result<Vec<(String, String, String)>, LexicalError> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.schema.filename, self.schema.content],
        );

        let query = query_parser.parse_query(pattern)?;
        let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let id = retrieved_doc
                .get_first(self.schema.id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let path = retrieved_doc
                .get_first(self.schema.canonical_path)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let content = retrieved_doc
                .get_first(self.schema.content)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            results.push((id, path, content));
        }

        Ok(results)
    }

    pub fn search(
        &self,
        query: &lss_types::SearchQuery,
    ) -> Result<Vec<LexicalSearchResult>, LexicalError> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.schema.filename,
                self.schema.alias_filenames,
                self.schema.content,
            ],
        );

        let mut subqueries = Vec::new();

        if !query.terms.is_empty() {
            let parsed = query_parser.parse_query(&query.terms)?;
            subqueries.push((Occur::Should, parsed));
        }

        for ext in &query.filetype_filters {
            if let Ok(boosted) = query_parser.parse_query(&format!("extension:{}^{}", ext, 5.0)) {
                subqueries.push((Occur::Should, boosted));
            }
        }

        for name in &query.name_filters {
            if let Ok(boosted) = query_parser.parse_query(&format!("filename:{}^{}", name, 5.0)) {
                subqueries.push((Occur::Should, boosted));
            }
        }

        let final_query = if subqueries.is_empty() && !query.raw.is_empty() {
            // Fallback to raw if terms and filters were somehow empty but raw isn't
            query_parser.parse_query(&query.raw)?
        } else if subqueries.is_empty() {
            return Ok(Vec::new());
        } else {
            Box::new(BooleanQuery::new(subqueries)) as Box<dyn tantivy::query::Query>
        };

        let top_docs = searcher.search(
            &final_query,
            &tantivy::collector::TopDocs::with_limit(query.limit),
        )?;

        let snippet_generator =
            SnippetGenerator::create(&searcher, &*final_query, self.schema.content)?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let id = retrieved_doc
                .get_first(self.schema.id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let path = retrieved_doc
                .get_first(self.schema.canonical_path)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let filename = retrieved_doc
                .get_first(self.schema.filename)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let snippet = snippet_generator.snippet_from_doc(&retrieved_doc).to_html();

            results.push(LexicalSearchResult {
                id,
                path,
                filename,
                snippet,
                score,
            });
        }

        Ok(results)
    }
}

pub struct LexicalSearchResult {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub snippet: String,
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use lss_types::SearchQuery;
    use tempfile::tempdir;

    fn open_test_store() -> (tempfile::TempDir, LexicalStore) {
        let dir = tempdir().expect("temp dir should exist");
        let store = LexicalStore::open(dir.path().try_into().expect("utf8 path")).expect("store should open");
        (dir, store)
    }

    #[test]
    fn add_and_search_by_content() {
        let (_dir, mut store) = open_test_store();
        store
            .add_document("doc1", "/test/file.txt", &[], "file.txt", "hello world from lss", Some("txt"), Some("text/plain"))
            .expect("add should succeed");
        store.commit().expect("commit should succeed");

        let query = SearchQuery {
            raw: "lss".into(),
            terms: "lss".into(),
            limit: 10,
            filetype_filters: vec![],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn add_and_search_by_filename() {
        let (_dir, mut store) = open_test_store();
        store
            .add_document("doc1", "/test/main.rs", &[], "main.rs", "fn main() {}", Some("rs"), Some("text/rust"))
            .expect("add should succeed");
        store.commit().expect("commit should succeed");

        let query = SearchQuery {
            raw: "main".into(),
            terms: "main".into(),
            limit: 10,
            filetype_filters: vec![],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert!(!results.is_empty());
        assert_eq!(results[0].filename, "main.rs");
    }

    #[test]
    fn delete_document_removes_from_search() {
        let (_dir, mut store) = open_test_store();
        store
            .add_document("doc1", "/test/file.txt", &[], "file.txt", "searchable content", Some("txt"), None)
            .expect("add should succeed");
        store.commit().expect("commit should succeed");

        store.delete_document("doc1").expect("delete should succeed");
        store.commit().expect("commit should succeed");

        let query = SearchQuery {
            raw: "searchable".into(),
            terms: "searchable".into(),
            limit: 10,
            filetype_filters: vec![],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert!(results.is_empty());
    }

    #[test]
    fn filetype_filter_boosts_results() {
        let (_dir, mut store) = open_test_store();
        store
            .add_document("doc1", "/test/a.rs", &[], "a.rs", "hello world", Some("rs"), None)
            .expect("add should succeed");
        store
            .add_document("doc2", "/test/b.txt", &[], "b.txt", "hello world", Some("txt"), None)
            .expect("add should succeed");
        store.commit().expect("commit should succeed");

        let query = SearchQuery {
            raw: "hello".into(),
            terms: "hello".into(),
            limit: 10,
            filetype_filters: vec!["rs".into()],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn name_filter_boosts_results() {
        let (_dir, mut store) = open_test_store();
        store
            .add_document("doc1", "/test/main.rs", &[], "main.rs", "content a", Some("rs"), None)
            .expect("add should succeed");
        store
            .add_document("doc2", "/test/util.rs", &[], "util.rs", "content a", Some("rs"), None)
            .expect("add should succeed");
        store.commit().expect("commit should succeed");

        let query = SearchQuery {
            raw: "content".into(),
            terms: "content".into(),
            limit: 10,
            filetype_filters: vec![],
            name_filters: vec!["main".into()],
        };
        let results = store.search(&query).expect("search should succeed");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn empty_query_returns_no_results() {
        let (_dir, store) = open_test_store();
        let query = SearchQuery {
            raw: String::new(),
            terms: String::new(),
            limit: 10,
            filetype_filters: vec![],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert!(results.is_empty());
    }

    #[test]
    fn search_on_empty_index_returns_no_results() {
        let (_dir, store) = open_test_store();
        let query = SearchQuery {
            raw: "anything".into(),
            terms: "anything".into(),
            limit: 10,
            filetype_filters: vec![],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert!(results.is_empty());
    }

    #[test]
    fn snippet_contains_matched_terms() {
        let (_dir, mut store) = open_test_store();
        store
            .add_document("doc1", "/test/file.txt", &[], "file.txt", "the quick brown fox jumps over the lazy dog", Some("txt"), None)
            .expect("add should succeed");
        store.commit().expect("commit should succeed");

        let query = SearchQuery {
            raw: "fox".into(),
            terms: "fox".into(),
            limit: 10,
            filetype_filters: vec![],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.contains("fox"));
    }

    #[test]
    fn alias_paths_are_indexed() {
        let (_dir, mut store) = open_test_store();
        store
            .add_document("doc1", "/real/file.txt", &["/link/file.txt"], "file.txt", "content", Some("txt"), None)
            .expect("add should succeed");
        store.commit().expect("commit should succeed");

        let query = SearchQuery {
            raw: "content".into(),
            terms: "content".into(),
            limit: 10,
            filetype_filters: vec![],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn limit_truncates_results() {
        let (_dir, mut store) = open_test_store();
        for i in 0..5 {
            store
                .add_document(
                    &format!("doc{i}"),
                    &format!("/test/file{i}.txt"),
                    &[],
                    &format!("file{i}.txt"),
                    "searchable content here",
                    Some("txt"),
                    None,
                )
                .expect("add should succeed");
        }
        store.commit().expect("commit should succeed");

        let query = SearchQuery {
            raw: "searchable".into(),
            terms: "searchable".into(),
            limit: 2,
            filetype_filters: vec![],
            name_filters: vec![],
        };
        let results = store.search(&query).expect("search should succeed");
        assert_eq!(results.len(), 2);
    }
}
