use crate::domain::Match;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct Store(Connection);

pub struct History {
    pub matches: Vec<(i64, Match)>,
    pub unreadable: Vec<i64>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let mut db = Connection::open(path)?;
        db.busy_timeout(std::time::Duration::from_secs(3))?;
        let tx = db.transaction()?;
        let version: u32 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => tx.execute_batch(
                "CREATE TABLE matches(id INTEGER PRIMARY KEY AUTOINCREMENT, protocol TEXT NOT NULL);
                                   PRAGMA user_version = 1;",
            )?,
            1 | 2 => (),
            _ => return Err("Версия базы данных не поддерживается.".into()),
        }
        if version < 2 {
            tx.execute_batch("CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL); PRAGMA user_version = 2;")?;
        }
        tx.commit()?;
        Ok(Self(db))
    }

    pub fn export_folder(&self) -> Result<Option<String>> {
        Ok(self
            .0
            .query_row(
                "SELECT value FROM app_settings WHERE key='export_folder'",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_export_folder(&self, path: &str) -> Result<()> {
        self.0.execute(
            "INSERT INTO app_settings(key, value) VALUES('export_folder', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [path],
        )?;
        Ok(())
    }

    pub fn list(&self) -> Result<History> {
        let mut query = self
            .0
            .prepare("SELECT id, protocol FROM matches ORDER BY id DESC")?;
        let mut rows = query.query([])?;
        let mut matches = Vec::new();
        let mut unreadable = Vec::new();
        while let Some(row) = rows.next()? {
            let id = row.get(0)?;
            match row
                .get::<_, String>(1)
                .ok()
                .and_then(|raw| serde_json::from_str::<Match>(&raw).ok())
                .filter(|value| value.progress().is_ok())
            {
                Some(value) => matches.push((id, value)),
                None => unreadable.push(id),
            }
        }
        Ok(History {
            matches,
            unreadable,
        })
    }

    pub fn insert(&self, value: &Match) -> Result<i64> {
        value.progress()?;
        self.0.execute(
            "INSERT INTO matches(protocol) VALUES(?)",
            [serde_json::to_string(value)?],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    pub fn update(&self, id: i64, before: &Match, after: &Match) -> Result<()> {
        let affected = self.0.execute(
            "UPDATE matches SET protocol=?1 WHERE id=?2 AND protocol=?3",
            params![
                serde_json::to_string(after)?,
                id,
                serde_json::to_string(before)?
            ],
        )?;
        if affected != 1 {
            return Err("Матч был изменён в другом окне. Откройте его повторно.".into());
        }
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.0.execute("DELETE FROM matches WHERE id=?", [id])?;
        Ok(())
    }
}
