use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{Connection, params};
use rusqlite_migration::{M, Migrations};
use time::OffsetDateTime;
use time::macros::format_description;

/// Must match the number of migrations below. A backup is taken only when an
/// existing file is older than this, and the tests assert the two agree.
const LATEST_SCHEMA: u32 = 6;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("migration: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupt row: {0}")]
    Corrupt(String),
    #[error("database lock poisoned")]
    Poisoned,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Locked(String),
}

/// One connection, serialised. Reads are cheap enough that a pool is not worth
/// the dependency until M1 adds concurrent read commands; the writer is always
/// a single connection because every mutation must be serialisable.
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.exists() {
            backup_before_migration(path)?;
        }
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let mut conn = Connection::open_in_memory()?;
        configure(&conn)?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// The schema version tracked by SQLite itself (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        self.with_conn(|conn| {
            conn.query_row("PRAGMA user_version", [], |row| row.get(0))
                .map_err(StoreError::from)
        })
    }

    pub(crate) fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let guard: MutexGuard<'_, Connection> =
            self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        f(&guard)
    }

    /// One `BEGIN IMMEDIATE`: state change, events and revision move together.
    pub(crate) fn with_write<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let guard: MutexGuard<'_, Connection> =
            self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = guard.unchecked_transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

fn configure(conn: &Connection) -> Result<(), StoreError> {
    // `journal_mode` returns the mode it settled on; WAL is unavailable for
    // in-memory databases, where "memory" is correct.
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if mode != "wal" && mode != "memory" {
        return Err(StoreError::Corrupt(format!(
            "unexpected journal mode {mode}"
        )));
    }
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "cache_size", -16_000i64)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    migrations().to_latest(conn)?;
    Ok(())
}

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../migrations/0001_init.sql")),
        M::up(include_str!("../migrations/0002_m1_documents.sql")),
        M::up(include_str!("../migrations/0003_m2_review.sql")),
        M::up(include_str!("../migrations/0004_m3_concept_gates.sql")),
        M::up(include_str!("../migrations/0005_m4_project_path.sql")),
        M::up(include_str!("../migrations/0006_plan.sql")),
    ])
}

/// `VACUUM INTO` a dated copy before a forward-only migration runs. A failed
/// backup stops startup rather than migrating without a way back.
fn backup_before_migration(path: &Path) -> Result<(), StoreError> {
    let version: u32 = {
        let conn = Connection::open(path)?;
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))?
    };
    if version == 0 || version >= LATEST_SCHEMA {
        return Ok(());
    }

    let dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups");
    std::fs::create_dir_all(&dir)?;
    let stamp = OffsetDateTime::now_utc()
        .format(&format_description!(
            "[year][month][day]T[hour][minute][second].[subsecond digits:3]Z"
        ))
        .map_err(|error| StoreError::Corrupt(format!("backup timestamp: {error}")))?;
    let target = dir.join(format!("antik-v{version}-{stamp}.db"));
    let target = target.to_string_lossy().into_owned();

    let conn = Connection::open(path)?;
    conn.execute("VACUUM INTO ?1", params![target])?;
    drop(conn);

    if let Err(error) = prune_backups(&dir, 3) {
        tracing::warn!(error = %error, "backup retention failed");
    }
    Ok(())
}

fn prune_backups(dir: &Path, keep: usize) -> Result<(), StoreError> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "db")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("antik-"))
        })
        .collect();
    files.sort();
    for path in files.iter().take(files.len().saturating_sub(keep)) {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_a_file_database_and_applies_migrations_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("antik.db");
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), LATEST_SCHEMA);
        drop(db);

        let reopened = Db::open(&path).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), LATEST_SCHEMA);
    }

    #[test]
    fn file_databases_use_wal() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("antik.db")).unwrap();
        let mode: String = db
            .with_conn(|conn| {
                conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))
                    .map_err(StoreError::from)
            })
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn migrates_an_older_file_after_backing_it_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("antik.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(include_str!("../migrations/0001_init.sql"))
                .unwrap();
            conn.execute_batch(include_str!("../migrations/0002_m1_documents.sql"))
                .unwrap();
            conn.pragma_update(None, "user_version", 2).unwrap();
            conn.execute(
                "INSERT INTO projects (id, slug, name, root_path, created_at)
                 VALUES ('p1', 'p1', 'kestrel', '/tmp/kestrel', 0)",
                [],
            )
            .unwrap();
        }

        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), LATEST_SCHEMA);
        let name: String = db
            .with_conn(|conn| {
                conn.query_row("SELECT name FROM projects WHERE id = 'p1'", [], |row| {
                    row.get(0)
                })
                .map_err(StoreError::from)
            })
            .unwrap();
        assert_eq!(name, "kestrel");

        let backups: Vec<_> = std::fs::read_dir(dir.path().join("backups"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(backups.len(), 1, "one backup before the migration");
        assert!(
            backups[0]
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("antik-v2-")
        );
    }

    #[test]
    fn backup_retention_keeps_the_newest_three() {
        let dir = tempfile::tempdir().unwrap();
        for stamp in [
            "20260101T000001.000Z",
            "20260102T000001.000Z",
            "20260103T000001.000Z",
            "20260104T000001.000Z",
            "20260105T000001.000Z",
        ] {
            std::fs::write(dir.path().join(format!("antik-v1-{stamp}.db")), b"backup").unwrap();
        }

        prune_backups(dir.path(), 3).unwrap();

        let mut names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "antik-v1-20260103T000001.000Z.db",
                "antik-v1-20260104T000001.000Z.db",
                "antik-v1-20260105T000001.000Z.db",
            ]
        );
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let db = Db::open_in_memory().unwrap();
        let error = db
            .with_conn(|conn| {
                conn.execute(
                    "INSERT INTO tasks (id, project_id, title, lane, phase, started_at)
                     VALUES ('t1', 'missing', 'orphan', 'learn', 'plan', 0)",
                    [],
                )
                .map_err(StoreError::from)
            })
            .unwrap_err();
        assert!(matches!(error, StoreError::Database(_)));
    }
}
