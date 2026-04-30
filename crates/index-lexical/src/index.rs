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

    pub fn search(
        &self,
        query: &mopi_types::SearchQuery,
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
