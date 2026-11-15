use std::fs;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

use crate::library::model::{LibraryItem, LibraryPage, LibraryQuery, MediaKind, MediaRow, SortKey};

const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS media (
  path TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  channel TEXT,
  duration_secs REAL,
  width INTEGER,
  height INTEGER,
  size_bytes INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  thumb_path TEXT,
  indexed_at INTEGER NOT NULL,
  origin TEXT NOT NULL DEFAULT 'scan'
);
CREATE INDEX IF NOT EXISTS media_mtime ON media(mtime);
CREATE INDEX IF NOT EXISTS media_kind ON media(kind);
CREATE TABLE IF NOT EXISTS practice (
  path TEXT PRIMARY KEY,
  mtime INTEGER NOT NULL,
  state_json TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  hidden INTEGER NOT NULL DEFAULT 0
);
";

pub struct Library {
    pub(crate) conn: Mutex<Connection>,
}

impl Library {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        // Existing 1.7.0 development databases may predate the hidden column.
        let has_hidden = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('practice') WHERE name = 'hidden'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_hidden {
            let _ = conn.execute(
                "ALTER TABLE practice ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0",
                [],
            );
        }
        // 1.7.0 databases predate the import origin marker.
        let has_origin = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('media') WHERE name = 'origin'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_origin {
            let _ = conn.execute(
                "ALTER TABLE media ADD COLUMN origin TEXT NOT NULL DEFAULT 'scan'",
                [],
            );
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns the saved practice state when the file has not changed since it
    /// was written. A changed mtime invalidates and removes the row.
    pub fn practice_get(&self, path: &str, mtime: i64) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        let row: Option<(i64, String)> = conn
            .query_row(
                "SELECT mtime, state_json FROM practice WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;

        match row {
            Some((saved_mtime, state)) if saved_mtime == mtime => Ok(Some(state)),
            Some(_) => {
                conn.execute("DELETE FROM practice WHERE path = ?1", params![path])
                    .map_err(|error| error.to_string())?;
                Ok(None)
            }
            None => Ok(None),
        }
    }

    pub fn practice_save(
        &self,
        path: &str,
        mtime: i64,
        state_json: &str,
        updated_at: i64,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT INTO practice(path, mtime, state_json, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
               mtime = excluded.mtime,
               state_json = excluded.state_json,
               updated_at = excluded.updated_at",
            params![path, mtime, state_json, updated_at],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn practice_delete(&self, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute("DELETE FROM practice WHERE path = ?1", params![path])
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn practice_recent(&self, limit: u32) -> Result<Vec<LibraryItem>, String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        let mut statement = conn
            .prepare(
                "SELECT m.path, m.kind, m.title, m.channel, m.duration_secs,
                        m.width, m.height, m.size_bytes, m.mtime, m.thumb_path
                 FROM practice p
                 JOIN media m ON m.path = p.path
                 WHERE p.hidden = 0
                 ORDER BY p.updated_at DESC
                 LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![limit], |row| {
                Ok(LibraryItem {
                    id: row.get::<_, String>(0)?,
                    path: row.get::<_, String>(0)?,
                    kind: kind_column(row, 1)?,
                    title: row.get(2)?,
                    channel: row.get(3)?,
                    duration_secs: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    size_bytes: row.get::<_, i64>(7)? as u64,
                    mtime: row.get(8)?,
                    thumb_path: row.get(9)?,
                })
            })
            .map_err(|error| error.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn practice_prune(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "DELETE FROM practice WHERE path NOT IN (SELECT path FROM media)",
            [],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn practice_hide(&self, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "UPDATE practice SET hidden = 1 WHERE path = ?1",
            params![path],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn practice_unhide(&self, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "UPDATE practice SET hidden = 0 WHERE path = ?1",
            params![path],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn media_get(&self, path: &str) -> Result<Option<LibraryItem>, String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.query_row(
            "SELECT path, kind, title, channel, duration_secs, width, height,
                    size_bytes, mtime, thumb_path
             FROM media WHERE path = ?1",
            params![path],
            row_to_item,
        )
        .optional()
        .map_err(|error| error.to_string())
    }

    /// Paths of indexed files with exactly this size, used as duplicate
    /// candidates before hashing.
    pub fn media_by_size(&self, size: u64) -> Result<Vec<String>, String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        let mut statement = conn
            .prepare("SELECT path FROM media WHERE size_bytes = ?1")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![size as i64], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn media_upsert(&self, row: &MediaRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT INTO media
               (path, kind, title, channel, duration_secs, width, height,
                size_bytes, mtime, thumb_path, indexed_at, origin)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(path) DO UPDATE SET
               kind = excluded.kind,
               title = excluded.title,
               channel = excluded.channel,
               duration_secs = excluded.duration_secs,
               width = excluded.width,
               height = excluded.height,
               size_bytes = excluded.size_bytes,
               mtime = excluded.mtime,
               thumb_path = excluded.thumb_path,
               indexed_at = excluded.indexed_at,
               origin = excluded.origin",
            params![
                row.path,
                row.kind.as_str(),
                row.title,
                row.channel,
                row.duration_secs,
                row.width,
                row.height,
                row.size_bytes as i64,
                row.mtime,
                row.thumb_path,
                row.indexed_at,
                row.origin,
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryItem> {
    Ok(LibraryItem {
        id: row.get::<_, String>(0)?,
        path: row.get::<_, String>(0)?,
        kind: kind_column(row, 1)?,
        title: row.get(2)?,
        channel: row.get(3)?,
        duration_secs: row.get(4)?,
        width: row.get(5)?,
        height: row.get(6)?,
        size_bytes: row.get::<_, i64>(7)? as u64,
        mtime: row.get(8)?,
        thumb_path: row.get(9)?,
    })
}

pub(crate) fn query_page(library: &Library, query: &LibraryQuery) -> Result<LibraryPage, String> {
    let conn = library.conn.lock().map_err(|error| error.to_string())?;
    let limit = query.limit.unwrap_or(200).clamp(1, 500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| format!("%{text}%"));
    let kind = query.kind.map(|kind| kind.as_str());

    let order = match query.sort {
        SortKey::DateAsc => "mtime ASC",
        SortKey::Title => "title COLLATE NOCASE ASC",
        SortKey::Duration => "duration_secs DESC NULLS LAST",
        SortKey::Size => "size_bytes DESC",
        SortKey::Channel => "channel COLLATE NOCASE ASC NULLS LAST",
        SortKey::DateDesc => "mtime DESC",
    };
    let sql = format!(
        "SELECT path, kind, title, channel, duration_secs, width, height, size_bytes, mtime, thumb_path
         FROM media
         WHERE (?1 IS NULL OR title LIKE ?1 OR channel LIKE ?1)
           AND (?2 IS NULL OR kind = ?2)
         ORDER BY {order}
         LIMIT ?3 OFFSET ?4"
    );

    let mut statement = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![search, kind, limit, offset], |row| {
            Ok(LibraryItem {
                id: row.get::<_, String>(0)?,
                path: row.get::<_, String>(0)?,
                kind: kind_column(row, 1)?,
                title: row.get(2)?,
                channel: row.get(3)?,
                duration_secs: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                size_bytes: row.get::<_, i64>(7)? as u64,
                mtime: row.get(8)?,
                thumb_path: row.get(9)?,
            })
        })
        .map_err(|error| error.to_string())?;

    let items: Vec<LibraryItem> = rows.filter_map(Result::ok).collect();
    let total: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM media
             WHERE (?1 IS NULL OR title LIKE ?1 OR channel LIKE ?1)
               AND (?2 IS NULL OR kind = ?2)",
            params![search, kind],
            |row| row.get(0),
        )
        .unwrap_or(items.len() as u32);

    Ok(LibraryPage { items, total })
}

fn kind_column(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<MediaKind> {
    let raw: String = row.get(index)?;
    MediaKind::parse(&raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown media kind: {raw}"),
            )),
        )
    })
}
