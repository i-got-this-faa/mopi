use tantivy::schema::*;

pub struct LexicalSchema {
    pub schema: Schema,
    pub id: Field,
    pub canonical_path: Field,
    pub alias_paths: Field,
    pub filename: Field,
    pub alias_filenames: Field,
    pub content: Field,
    pub extension: Field,
    pub mime: Field,
}

impl LexicalSchema {
    pub fn new() -> Self {
        let mut schema_builder = Schema::builder();

        // ID is stored and indexed as a string for exact lookup
        let id = schema_builder.add_text_field("id", STRING | STORED);

        // Paths and filenames are indexed with a standard analyzer for partial matching
        let canonical_path = schema_builder.add_text_field("canonical_path", TEXT | STORED);
        let alias_paths = schema_builder.add_text_field("alias_paths", TEXT | STORED);
        let filename = schema_builder.add_text_field("filename", TEXT | STORED);
        let alias_filenames = schema_builder.add_text_field("alias_filenames", TEXT | STORED);

        // Content is indexed but not necessarily stored (to save space, we can read from disk)
        // However, Tantivy's snippets work better if it's stored or we use field stores.
        // For now, let's store it to make snippet generation easy, but we might change this.
        let content = schema_builder.add_text_field("content", TEXT | STORED);

        let extension = schema_builder.add_text_field("extension", STRING | STORED);
        let mime = schema_builder.add_text_field("mime", STRING | STORED);

        let schema = schema_builder.build();
        Self {
            schema,
            id,
            canonical_path,
            alias_paths,
            filename,
            alias_filenames,
            content,
            extension,
            mime,
        }
    }
}

impl Default for LexicalSchema {
    fn default() -> Self {
        Self::new()
    }
}
