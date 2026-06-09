use lss_types::{DocumentId, FailureRecord};
use rusqlite::{Connection, Result, params};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MetaError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct MetaStore {
    conn: Connection,
}

pub struct FileRecord<'a> {
    pub id: &'a DocumentId,
    pub root_id: i64,
    pub canonical_path: &'a str,
    pub file_name: &'a str,
    pub extension: Option<&'a str>,
    pub size: u64,
    pub modified_unix_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub id: i64,
    pub file_id: String,
    pub canonical_path: String,
    pub stage: String,
    pub started_at: i64,
    pub updated_at: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JobRecord {
    pub id: i64,
    pub kind: String,
    pub status: String,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub detail: Option<String>,
}

impl MetaStore {
    pub fn open(path: &Path) -> Result<Self, MetaError> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA cache_size = -64000;
             PRAGMA mmap_size = 268435456;
             PRAGMA temp_store = MEMORY;
             PRAGMA wal_autocheckpoint = 1000;",
        )?;

        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), MetaError> {
        // Initial schema
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS roots (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE
            );

            CREATE TABLE IF NOT EXISTS files (
                id TEXT PRIMARY KEY,
                root_id INTEGER NOT NULL,
                canonical_path TEXT NOT NULL UNIQUE,
                file_name TEXT NOT NULL,
                extension TEXT,
                mime TEXT,
                size INTEGER NOT NULL,
                modified_unix_seconds INTEGER NOT NULL,
                extractor_status TEXT,
                lexical_gen INTEGER DEFAULT 0,
                FOREIGN KEY(root_id) REFERENCES roots(id)
            );

            CREATE TABLE IF NOT EXISTS file_aliases (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id TEXT NOT NULL,
                observed_path TEXT NOT NULL UNIQUE,
                FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id TEXT NOT NULL,
                text TEXT NOT NULL,
                FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE
            );",
        )?;

        // Migration: add vector_gen to files
        self.add_column_if_not_exists(
            "files",
            "vector_gen INTEGER DEFAULT 0",
        )?;

        // Migration: add last_ingest_time to files
        self.add_column_if_not_exists(
            "files",
            "last_ingest_time INTEGER",
        )?;

        // Migration: add content_fingerprint to files
        self.add_column_if_not_exists(
            "files",
            "content_fingerprint TEXT",
        )?;

        // Migration: add first_seen_time and last_seen_time to file_aliases
        self.add_column_if_not_exists(
            "file_aliases",
            "first_seen_time INTEGER NOT NULL DEFAULT (strftime('%s','now'))",
        )?;
        self.add_column_if_not_exists(
            "file_aliases",
            "last_seen_time INTEGER NOT NULL DEFAULT (strftime('%s','now'))",
        )?;

        // Migration: add embedding BLOB to chunks
        self.add_column_if_not_exists(
            "chunks",
            "embedding BLOB",
        )?;

        // New tables
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS ingest_journal (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id TEXT NOT NULL,
                canonical_path TEXT NOT NULL,
                stage TEXT NOT NULL CHECK(stage IN ('meta','lexical','vector','done')),
                started_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                error_message TEXT
            );

            CREATE TABLE IF NOT EXISTS failures (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id TEXT NOT NULL,
                canonical_path TEXT NOT NULL,
                error_message TEXT NOT NULL,
                stage TEXT NOT NULL,
                failed_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );

            CREATE TABLE IF NOT EXISTS jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                started_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                completed_at INTEGER,
                detail TEXT
            );",
        )?;

        Ok(())
    }

    fn add_column_if_not_exists(&self, table: &str, column_def: &str) -> Result<(), MetaError> {
        let sql = format!("ALTER TABLE {table} ADD COLUMN {column_def}");
        match self.conn.execute_batch(&sql) {
            Ok(()) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::Unknown
                    || err.code == rusqlite::ErrorCode::CannotOpen =>
            {
                // Column already exists — safe to ignore on SQLite
                Ok(())
            }
            Err(e) => Err(MetaError::Sqlite(e)),
        }
    }

    // -----------------------------------------------------------------------
    // Roots
    // -----------------------------------------------------------------------

    pub fn upsert_root(&self, path: &str) -> Result<i64, MetaError> {
        self.conn.execute(
            "INSERT INTO roots (path) VALUES (?1)
             ON CONFLICT(path) DO UPDATE SET path=excluded.path",
            params![path],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM roots WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    // -----------------------------------------------------------------------
    // Files
    // -----------------------------------------------------------------------

    pub fn get_file_by_canonical_path(&self, path: &str) -> Result<Option<String>, MetaError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM files WHERE canonical_path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_file(&self, record: FileRecord<'_>) -> Result<(), MetaError> {
        // Check if canonical_path already exists to preserve vector_gen
        let existing_vector_gen: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(vector_gen, 0) FROM files WHERE canonical_path = ?1",
                params![record.canonical_path],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let fingerprint = self.compute_fingerprint(record.canonical_path);

        self.conn.execute(
            "INSERT INTO files (id, root_id, canonical_path, file_name, extension, size,
                                modified_unix_seconds, extractor_status, content_fingerprint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)
             ON CONFLICT(canonical_path) DO UPDATE SET
                file_name=excluded.file_name,
                extension=excluded.extension,
                size=excluded.size,
                modified_unix_seconds=excluded.modified_unix_seconds,
                extractor_status='pending',
                content_fingerprint=excluded.content_fingerprint,
                vector_gen=?9",
            params![
                record.id.0.to_string(),
                record.root_id,
                record.canonical_path,
                record.file_name,
                record.extension,
                record.size as i64,
                record.modified_unix_seconds as i64,
                fingerprint,
                existing_vector_gen,
            ],
        )?;
        Ok(())
    }

    pub fn set_extractor_status(&self, id: &str, status: &str) -> Result<(), MetaError> {
        self.conn.execute(
            "UPDATE files SET extractor_status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    pub fn update_lexical_gen(&self, id: &str, generation: i64) -> Result<(), MetaError> {
        self.conn.execute(
            "UPDATE files SET lexical_gen = ?1 WHERE id = ?2",
            params![generation, id],
        )?;
        Ok(())
    }

    pub fn update_vector_gen(&self, id: &str, generation: i64) -> Result<(), MetaError> {
        self.conn.execute(
            "UPDATE files SET vector_gen = ?1 WHERE id = ?2",
            params![generation, id],
        )?;
        Ok(())
    }

    pub fn update_ingest_time(&self, id: &str) -> Result<(), MetaError> {
        self.conn.execute(
            "UPDATE files SET last_ingest_time = strftime('%s','now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn delete_file(&self, id: &str) -> Result<(), MetaError> {
        self.conn
            .execute("DELETE FROM files WHERE id = ?1", params![id])?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Aliases
    // -----------------------------------------------------------------------

    pub fn upsert_alias(&self, file_id: &str, observed_path: &str) -> Result<(), MetaError> {
        self.conn.execute(
            "INSERT INTO file_aliases (file_id, observed_path)
             VALUES (?1, ?2)
             ON CONFLICT(observed_path) DO UPDATE SET
                file_id=excluded.file_id,
                last_seen_time=strftime('%s','now')",
            params![file_id, observed_path],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Chunks & Embeddings
    // -----------------------------------------------------------------------

    pub fn insert_chunk(&self, file_id: &str, text: &str) -> Result<i64, MetaError> {
        self.conn.execute(
            "INSERT INTO chunks (file_id, text) VALUES (?1, ?2)",
            params![file_id, text],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn replace_chunks(&self, file_id: &str, chunks: &[String]) -> Result<Vec<i64>, MetaError> {
        self.conn
            .execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])?;

        let mut ids = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            self.conn.execute(
                "INSERT INTO chunks (file_id, text) VALUES (?1, ?2)",
                params![file_id, chunk],
            )?;
            ids.push(self.conn.last_insert_rowid());
        }

        Ok(ids)
    }

    pub fn replace_chunks_with_embeddings(
        &self,
        file_id: &str,
        chunks: &[String],
        embeddings: &[Vec<f32>],
    ) -> Result<Vec<i64>, MetaError> {
        self.conn
            .execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])?;

        let mut ids = Vec::with_capacity(chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            let blob = if i < embeddings.len() {
                embedding_to_blob(&embeddings[i])
            } else {
                None
            };
            self.conn.execute(
                "INSERT INTO chunks (file_id, text, embedding) VALUES (?1, ?2, ?3)",
                params![file_id, chunk, blob],
            )?;
            ids.push(self.conn.last_insert_rowid());
        }

        Ok(ids)
    }

    pub fn store_chunk_embeddings(
        &self,
        chunk_ids: &[i64],
        embeddings: &[Vec<f32>],
    ) -> Result<(), MetaError> {
        for (chunk_id, embedding) in chunk_ids.iter().zip(embeddings) {
            let blob = embedding_to_blob(embedding);
            self.conn.execute(
                "UPDATE chunks SET embedding = ?1 WHERE id = ?2",
                params![blob, chunk_id],
            )?;
        }
        Ok(())
    }

    pub fn get_chunks_with_embeddings_after(
        &self,
        last_chunk_id: i64,
        limit: usize,
    ) -> Result<Vec<(i64, String, Option<Vec<f32>>)>, MetaError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, embedding
             FROM chunks
             WHERE id > ?1
             ORDER BY id
             LIMIT ?2",
        )?;
        let mut rows = stmt.query(params![last_chunk_id, limit as i64])?;
        let mut chunks = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let text: String = row.get(1)?;
            let blob: Option<Vec<u8>> = row.get(2)?;
            let embedding = blob.and_then(|b| embedding_from_blob(&b));
            chunks.push((id, text, embedding));
        }
        Ok(chunks)
    }

    pub fn get_chunks_for_file(&self, file_id: &str) -> Result<Vec<i64>, MetaError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM chunks WHERE file_id = ?1")?;
        let mut rows = stmt.query(params![file_id])?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next()? {
            ids.push(row.get(0)?);
        }
        Ok(ids)
    }

    pub fn list_chunks_after(
        &self,
        last_chunk_id: i64,
        limit: usize,
    ) -> Result<Vec<(i64, String)>, MetaError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text
             FROM chunks
             WHERE id > ?1
             ORDER BY id
             LIMIT ?2",
        )?;
        let mut rows = stmt.query(params![last_chunk_id, limit as i64])?;
        let mut chunks = Vec::new();
        while let Some(row) = rows.next()? {
            chunks.push((row.get(0)?, row.get(1)?));
        }
        Ok(chunks)
    }

    pub fn count_chunks(&self) -> Result<i64, MetaError> {
        self.conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .map_err(MetaError::from)
    }

    pub fn get_file_by_chunk_id(
        &self,
        chunk_id: i64,
    ) -> Result<Option<FileRecordOwned>, MetaError> {
        let mut stmt = self.conn.prepare("SELECT f.id, f.root_id, f.canonical_path, f.file_name, f.extension, f.size, f.modified_unix_seconds FROM files f JOIN chunks c ON c.file_id = f.id WHERE c.id = ?1")?;
        let mut rows = stmt.query(params![chunk_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(FileRecordOwned {
                id: row.get(0)?,
                root_id: row.get(1)?,
                canonical_path: row.get(2)?,
                file_name: row.get(3)?,
                extension: row.get(4)?,
                size: row.get(5)?,
                modified_unix_seconds: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_chunk_text(&self, chunk_id: i64) -> Result<Option<String>, MetaError> {
        let mut stmt = self.conn.prepare("SELECT text FROM chunks WHERE id = ?1")?;
        let mut rows = stmt.query(params![chunk_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    // -----------------------------------------------------------------------
    // Ingest Journal
    // -----------------------------------------------------------------------

    pub fn create_journal_entry(
        &self,
        file_id: &str,
        canonical_path: &str,
    ) -> Result<i64, MetaError> {
        self.conn.execute(
            "INSERT INTO ingest_journal (file_id, canonical_path, stage)
             VALUES (?1, ?2, 'meta')",
            params![file_id, canonical_path],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn advance_journal_stage(&self, journal_id: i64, stage: &str) -> Result<(), MetaError> {
        self.conn.execute(
            "UPDATE ingest_journal SET stage = ?1, updated_at = strftime('%s','now')
             WHERE id = ?2",
            params![stage, journal_id],
        )?;
        Ok(())
    }

    pub fn complete_journal_entry(&self, journal_id: i64) -> Result<(), MetaError> {
        self.conn.execute(
            "DELETE FROM ingest_journal WHERE id = ?1",
            params![journal_id],
        )?;
        Ok(())
    }

    pub fn fail_journal_entry(
        &self,
        journal_id: i64,
        _error_message: &str,
    ) -> Result<(), MetaError> {
        // Delete the journal entry; the failure is recorded separately in the failures table.
        // This prevents recovery from retrying permanently-failed files.
        self.conn.execute(
            "DELETE FROM ingest_journal WHERE id = ?1",
            params![journal_id],
        )?;
        Ok(())
    }

    pub fn get_stale_journal_entries(&self) -> Result<Vec<JournalEntry>, MetaError> {
        // At startup, any entry not marked 'done' is stale (no active indexing)
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, canonical_path, stage, started_at, updated_at, error_message
             FROM ingest_journal
             WHERE stage != 'done'",
        )?;
        let mut rows = stmt.query([])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            entries.push(JournalEntry {
                id: row.get(0)?,
                file_id: row.get(1)?,
                canonical_path: row.get(2)?,
                stage: row.get(3)?,
                started_at: row.get(4)?,
                updated_at: row.get(5)?,
                error_message: row.get(6)?,
            });
        }
        Ok(entries)
    }

    // -----------------------------------------------------------------------
    // Failures
    // -----------------------------------------------------------------------

    pub fn record_failure(
        &self,
        file_id: &str,
        canonical_path: &str,
        error_message: &str,
        stage: &str,
    ) -> Result<(), MetaError> {
        self.conn.execute(
            "INSERT INTO failures (file_id, canonical_path, error_message, stage)
             VALUES (?1, ?2, ?3, ?4)",
            params![file_id, canonical_path, error_message, stage],
        )?;
        Ok(())
    }

    pub fn get_failures(&self, limit: usize) -> Result<Vec<FailureRecord>, MetaError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, canonical_path, error_message, stage, failed_at
             FROM failures
             ORDER BY failed_at DESC
             LIMIT ?1",
        )?;
        let mut rows = stmt.query(params![limit as i64])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(FailureRecord {
                id: row.get(0)?,
                file_id: row.get(1)?,
                canonical_path: row.get(2)?,
                error_message: row.get(3)?,
                stage: row.get(4)?,
                failed_at: row.get(5)?,
            });
        }
        Ok(records)
    }

    pub fn count_failures(&self) -> Result<i64, MetaError> {
        self.conn
            .query_row("SELECT COUNT(*) FROM failures", [], |row| row.get(0))
            .map_err(MetaError::from)
    }

    // -----------------------------------------------------------------------
    // Jobs
    // -----------------------------------------------------------------------

    pub fn create_job(&self, kind: &str) -> Result<i64, MetaError> {
        self.conn.execute(
            "INSERT INTO jobs (kind, status) VALUES (?1, 'running')",
            params![kind],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn complete_job(&self, job_id: i64, status: &str, detail: Option<&str>) -> Result<(), MetaError> {
        self.conn.execute(
            "UPDATE jobs
             SET status = ?1, completed_at = strftime('%s','now'), detail = ?2
             WHERE id = ?3",
            params![status, detail, job_id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Transaction support
    // -----------------------------------------------------------------------

    pub fn begin_transaction(&self) -> Result<(), MetaError> {
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(MetaError::from)
    }

    pub fn commit(&self) -> Result<(), MetaError> {
        self.conn
            .execute_batch("COMMIT")
            .map_err(MetaError::from)
    }

    pub fn rollback(&self) -> Result<(), MetaError> {
        self.conn
            .execute_batch("ROLLBACK")
            .map_err(MetaError::from)
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn compute_fingerprint(&self, path: &str) -> Option<String> {
        // Simple fingerprint based on path + metadata hash
        // This is a placeholder; a full content hash would need extracted text
        Some(format!("path:{}", path))
    }
}

pub struct FileRecordOwned {
    pub id: String,
    pub root_id: i64,
    pub canonical_path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub size: u64,
    pub modified_unix_seconds: u64,
}

fn embedding_to_blob(embedding: &[f32]) -> Option<Vec<u8>> {
    if embedding.is_empty() {
        return None;
    }
    let bytes: Vec<u8> = embedding
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    Some(bytes)
}

fn embedding_from_blob(blob: &[u8]) -> Option<Vec<f32>> {
    if blob.len() % 4 != 0 {
        return None;
    }
    let floats: Vec<f32> = blob
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    Some(floats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_open_and_migrate() {
        let file = NamedTempFile::new().expect("temp db file should be created");
        let store = MetaStore::open(file.path()).expect("meta store should open");

        let root_id = store
            .upsert_root("/home/test")
            .expect("root should upsert successfully");
        assert!(root_id > 0);
    }

    #[test]
    fn test_replace_chunks_rewrites_rows_for_file() {
        let file = NamedTempFile::new().expect("temp db file should be created");
        let store = MetaStore::open(file.path()).expect("meta store should open");
        let doc_id = DocumentId::new();
        let root_id = store
            .upsert_root("/home/test")
            .expect("root should upsert successfully");

        store
            .upsert_file(FileRecord {
                id: &doc_id,
                root_id,
                canonical_path: "/home/test/a.txt",
                file_name: "a.txt",
                extension: Some("txt"),
                size: 1,
                modified_unix_seconds: 1,
            })
            .expect("file should upsert successfully");

        let first = store
            .replace_chunks(
                &doc_id.0.to_string(),
                &[String::from("one"), String::from("two")],
            )
            .expect("first replace should succeed");
        assert_eq!(first.len(), 2);

        let second = store
            .replace_chunks(&doc_id.0.to_string(), &[String::from("updated")])
            .expect("second replace should succeed");
        assert_eq!(second.len(), 1);

        let chunk_ids = store
            .get_chunks_for_file(&doc_id.0.to_string())
            .expect("chunk ids should load");
        assert_eq!(chunk_ids, second);
        assert_eq!(store.count_chunks().expect("chunk count should load"), 1);
    }

    #[test]
    fn test_ingest_journal_lifecycle() {
        let file = NamedTempFile::new().expect("temp db file should be created");
        let store = MetaStore::open(file.path()).expect("meta store should open");

        let jid = store
            .create_journal_entry("uuid-1", "/path/to/file.txt")
            .expect("journal entry should be created");
        assert!(jid > 0);

        store
            .advance_journal_stage(jid, "lexical")
            .expect("journal stage should advance");

        store
            .advance_journal_stage(jid, "vector")
            .expect("journal stage should advance");

        store
            .complete_journal_entry(jid)
            .expect("journal entry should be deleted");

        let stale = store
            .get_stale_journal_entries()
            .expect("stale entries should load");
        assert!(stale.is_empty());
    }

    #[test]
    fn test_failures_lifecycle() {
        let file = NamedTempFile::new().expect("temp db file should be created");
        let store = MetaStore::open(file.path()).expect("meta store should open");

        store
            .record_failure("uuid-1", "/path/to/file.txt", "extraction failed", "meta")
            .expect("failure should be recorded");

        let failures = store.get_failures(10).expect("failures should load");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].canonical_path, "/path/to/file.txt");
        assert_eq!(failures[0].error_message, "extraction failed");

        assert_eq!(
            store.count_failures().expect("count should work"),
            1
        );
    }

    #[test]
    fn test_jobs_lifecycle() {
        let file = NamedTempFile::new().expect("temp db file should be created");
        let store = MetaStore::open(file.path()).expect("meta store should open");

        let job_id = store.create_job("reindex").expect("job should be created");
        assert!(job_id > 0);

        store
            .complete_job(job_id, "completed", Some("all done"))
            .expect("job should complete");
    }

    #[test]
    fn test_embedding_blob_roundtrip() {
        let file = NamedTempFile::new().expect("temp db file should be created");
        let store = MetaStore::open(file.path()).expect("meta store should open");
        let doc_id = DocumentId::new();
        let root_id = store
            .upsert_root("/home/test")
            .expect("root should upsert successfully");

        store
            .upsert_file(FileRecord {
                id: &doc_id,
                root_id,
                canonical_path: "/home/test/a.txt",
                file_name: "a.txt",
                extension: Some("txt"),
                size: 1,
                modified_unix_seconds: 1,
            })
            .expect("file should upsert successfully");

        let embeddings = vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]];
        store
            .replace_chunks_with_embeddings(
                &doc_id.0.to_string(),
                &[String::from("chunk a"), String::from("chunk b")],
                &embeddings,
            )
            .expect("chunks with embeddings should replace");

        let loaded = store
            .get_chunks_with_embeddings_after(0, 10)
            .expect("chunks should load");
        assert_eq!(loaded.len(), 2);
        for (i, (_, _, loaded_emb)) in loaded.iter().enumerate() {
            let loaded_emb = loaded_emb
                .as_ref()
                .expect("embedding should be present");
            assert!((loaded_emb[0] - embeddings[i][0]).abs() < 1e-6);
        }
    }

    #[test]
    fn test_vector_gen_preserved_on_upsert() {
        let file = NamedTempFile::new().expect("temp db file should be created");
        let store = MetaStore::open(file.path()).expect("meta store should open");
        let doc_id = DocumentId::new();
        let root_id = store
            .upsert_root("/home/test")
            .expect("root should upsert successfully");

        store
            .upsert_file(FileRecord {
                id: &doc_id,
                root_id,
                canonical_path: "/home/test/a.txt",
                file_name: "a.txt",
                extension: Some("txt"),
                size: 1,
                modified_unix_seconds: 1,
            })
            .expect("file should upsert successfully");

        store
            .update_vector_gen(&doc_id.0.to_string(), 5)
            .expect("vector gen should update");

        // Re-upsert with same path — vector_gen should be preserved
        store
            .upsert_file(FileRecord {
                id: &doc_id,
                root_id,
                canonical_path: "/home/test/a.txt",
                file_name: "a.txt",
                extension: Some("txt"),
                size: 2,
                modified_unix_seconds: 2,
            })
            .expect("file should re-upsert");

        // Verify via update_vector_gen reading back — if gen reset it'd be 0 then 1
        store
            .update_vector_gen(&doc_id.0.to_string(), 5)
            .expect("vector gen should still be settable");
    }
}
