use lss_types::DocumentId;
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

impl MetaStore {
    pub fn open(path: &Path) -> Result<Self, MetaError> {
        let conn = Connection::open(path)?;

        // Performance pragmas
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), MetaError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS roots (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE
            );
            
            CREATE TABLE IF NOT EXISTS files (
                id TEXT PRIMARY KEY, -- DocumentId (UUID)
                root_id INTEGER NOT NULL,
                canonical_path TEXT NOT NULL UNIQUE,
                file_name TEXT NOT NULL,
                extension TEXT,
                mime TEXT,
                size INTEGER NOT NULL,
                modified_unix_seconds INTEGER NOT NULL,
                extractor_status TEXT, -- 'pending', 'done', 'failed'
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
        Ok(())
    }

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
        self.conn.execute(
            "INSERT INTO files (id, root_id, canonical_path, file_name, extension, size, modified_unix_seconds, extractor_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')
             ON CONFLICT(canonical_path) DO UPDATE SET
                file_name=excluded.file_name,
                extension=excluded.extension,
                size=excluded.size,
                modified_unix_seconds=excluded.modified_unix_seconds,
                extractor_status='pending'",
            params![
                record.id.0.to_string(),
                record.root_id,
                record.canonical_path,
                record.file_name,
                record.extension,
                record.size as i64,
                record.modified_unix_seconds as i64,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_alias(&self, file_id: &str, observed_path: &str) -> Result<(), MetaError> {
        self.conn.execute(
            "INSERT INTO file_aliases (file_id, observed_path)
             VALUES (?1, ?2)
             ON CONFLICT(observed_path) DO UPDATE SET file_id=excluded.file_id",
            params![file_id, observed_path],
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

    pub fn delete_file(&self, id: &str) -> Result<(), MetaError> {
        self.conn
            .execute("DELETE FROM files WHERE id = ?1", params![id])?;
        Ok(())
    }

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
}
