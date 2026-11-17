use std::{
    error::Error,
    fs, io,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::browser::search::SearchEngine;
use rusqlite::{Connection, OptionalExtension, params};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub restore_session: bool,
    pub home_page: String,
    pub search_engine: SearchEngine,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            restore_session: true,
            home_page: crate::browser::start_page::URI.into(),
            search_engine: SearchEngine::default(),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Session {
    pub urls: Vec<String>,
    pub active: usize,
}

pub struct HistoryEntry {
    pub uri: String,
    pub title: String,
    pub visited_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub id: i64,
    pub title: String,
    pub uri: String,
}

pub struct Store {
    connection: Connection,
}

pub fn restorable_uri(uri: &str) -> bool {
    if uri.chars().any(char::is_control) {
        return false;
    }
    if uri == "about:blank" || uri == crate::browser::start_page::URI {
        return true;
    }
    url::Url::parse(uri).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

impl Store {
    pub fn open_default() -> Result<Self> {
        Self::open(
            &gtk4::glib::user_data_dir()
                .join("kairo")
                .join("kairo.sqlite3"),
        )
    }

    pub fn open(path: &Path) -> Result<Self> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        Self::initialize(Connection::open(path)?)
    }

    pub fn memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(mut connection: Connection) -> Result<Self> {
        connection.busy_timeout(Duration::from_millis(250))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 3 {
            return Err(io::Error::other(
                "Este perfil foi criado por uma versão mais recente do Kairo.",
            )
            .into());
        }
        if version == 0 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE preferences (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    restore_session INTEGER NOT NULL CHECK (restore_session IN (0,1)),
                    home_page TEXT NOT NULL
                 );
                 CREATE TABLE session (
                    position INTEGER PRIMARY KEY,
                    uri TEXT NOT NULL,
                    selected INTEGER NOT NULL CHECK (selected IN (0,1))
                 );
                 CREATE TABLE history (
                    id INTEGER PRIMARY KEY,
                    uri TEXT NOT NULL UNIQUE,
                    title TEXT NOT NULL,
                    visited_at INTEGER NOT NULL
                 );
                 CREATE INDEX history_recent ON history(visited_at DESC, id DESC);
                 PRAGMA user_version = 1;",
            )?;
            transaction.commit()?;
        }
        if version < 2 {
            let transaction = connection.transaction()?;
            transaction.execute_batch("ALTER TABLE preferences ADD COLUMN search_engine TEXT NOT NULL DEFAULT 'duckduckgo' CHECK (search_engine IN ('duckduckgo', 'google')); PRAGMA user_version = 2;")?;
            transaction.commit()?;
        }
        if version < 3 {
            let transaction = connection.transaction()?;
            transaction.execute_batch("CREATE TABLE bookmarks (id INTEGER PRIMARY KEY, title TEXT NOT NULL, uri TEXT NOT NULL UNIQUE, position INTEGER NOT NULL); PRAGMA user_version = 3;")?;
            transaction.commit()?;
        }
        Ok(Self { connection })
    }

    pub fn preferences(&self) -> Result<Preferences> {
        let preferences = self
            .connection
            .query_row(
                "SELECT restore_session, home_page, search_engine FROM preferences WHERE id = 1",
                [],
                |row| {
                    Ok(Preferences {
                        restore_session: row.get(0)?,
                        home_page: row.get(1)?,
                        search_engine: SearchEngine::from_id(&row.get::<_, String>(2)?)
                            .ok_or_else(|| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    2,
                                    rusqlite::types::Type::Text,
                                    Box::new(io::Error::other("Buscador inválido no perfil")),
                                )
                            })?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default();
        if !restorable_uri(&preferences.home_page) {
            return Err(io::Error::other("Página inicial inválida no perfil.").into());
        }
        Ok(preferences)
    }

    pub fn save_preferences(&mut self, preferences: &Preferences) -> Result<()> {
        if !restorable_uri(&preferences.home_page) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Use a tela do Kairo, HTTP, HTTPS ou about:blank na página inicial.",
            )
            .into());
        }
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO preferences (id, restore_session, home_page, search_engine) VALUES (1, ?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET restore_session=excluded.restore_session, home_page=excluded.home_page, search_engine=excluded.search_engine",
            params![preferences.restore_session, preferences.home_page, preferences.search_engine.id()],
        )?;
        if !preferences.restore_session {
            transaction.execute("DELETE FROM session", [])?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn session(&self) -> Result<Session> {
        let mut query = self
            .connection
            .prepare("SELECT uri, selected FROM session ORDER BY position")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
        })?;
        let mut session = Session::default();
        for row in rows {
            let (uri, selected) = row?;
            if selected {
                session.active = session.urls.len();
            }
            session.urls.push(if restorable_uri(&uri) {
                uri
            } else {
                "about:blank".into()
            });
        }
        Ok(session)
    }

    pub fn save_session(&mut self, session: &Session) -> Result<()> {
        if session.urls.iter().any(|uri| !restorable_uri(uri))
            || (!session.urls.is_empty() && session.active >= session.urls.len())
        {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Sessão inválida.").into());
        }
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM session", [])?;
        for (position, uri) in session.urls.iter().enumerate() {
            transaction.execute(
                "INSERT INTO session VALUES (?1, ?2, ?3)",
                params![position as i64, uri, position == session.active],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn record_visit(&self, uri: &str, title: &str) -> Result<()> {
        if !restorable_uri(uri)
            || !url::Url::parse(uri).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
        {
            return Ok(());
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
        self.connection.execute(
            "INSERT INTO history (uri, title, visited_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(uri) DO UPDATE SET title=excluded.title, visited_at=excluded.visited_at",
            params![uri, title, now],
        )?;
        Ok(())
    }

    pub fn history(&self) -> Result<Vec<HistoryEntry>> {
        let mut query = self.connection.prepare("SELECT uri, title, visited_at FROM history ORDER BY visited_at DESC, id DESC LIMIT 200")?;
        let rows = query.query_map([], |row| {
            Ok(HistoryEntry {
                uri: row.get(0)?,
                title: row.get(1)?,
                visited_at: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn clear_history(&self) -> Result<()> {
        self.connection.execute("DELETE FROM history", [])?;
        Ok(())
    }

    /// Remove histórico e sessão juntos; favoritos e preferências são preservados.
    pub fn clear_browsing_records(&mut self) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM history", [])?;
        transaction.execute("DELETE FROM session", [])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn bookmarks(&self) -> Result<Vec<Bookmark>> {
        let mut query = self
            .connection
            .prepare("SELECT id, title, uri FROM bookmarks ORDER BY position, id")?;
        let rows = query.query_map([], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                title: row.get(1)?,
                uri: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn save_bookmark(&mut self, id: Option<i64>, title: &str, uri: &str) -> Result<i64> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 200 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Informe um nome com até 200 caracteres.",
            )
            .into());
        }
        let uri = uri.trim();
        if uri.len() > 8192
            || !restorable_uri(uri)
            || !url::Url::parse(uri).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Use um endereço HTTP/HTTPS completo, sem usuário ou senha.",
            )
            .into());
        }
        let uri = url::Url::parse(uri)?.to_string();
        let existing: Option<i64> = self
            .connection
            .query_row("SELECT id FROM bookmarks WHERE uri = ?1", [&uri], |row| {
                row.get(0)
            })
            .optional()?;
        if let (Some(id), Some(existing)) = (id, existing)
            && id != existing
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Esse endereço já está nos favoritos.",
            )
            .into());
        }
        if let Some(id) = id.or(existing) {
            if self.connection.execute(
                "UPDATE bookmarks SET title = ?1, uri = ?2 WHERE id = ?3",
                params![title, uri, id],
            )? == 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Este favorito não existe mais.",
                )
                .into());
            }
            return Ok(id);
        }
        self.connection.execute("INSERT INTO bookmarks (title, uri, position) VALUES (?1, ?2, COALESCE((SELECT MAX(position) + 1 FROM bookmarks), 0))", params![title, uri])?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn remove_bookmark(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn move_bookmark(&mut self, id: i64, direction: isize) -> Result<()> {
        let mut bookmarks = self.bookmarks()?;
        let Some(index) = bookmarks.iter().position(|bookmark| bookmark.id == id) else {
            return Ok(());
        };
        let next = index
            .saturating_add_signed(direction)
            .min(bookmarks.len() - 1);
        if index == next {
            return Ok(());
        }
        bookmarks.swap(index, next);
        let transaction = self.connection.transaction()?;
        for (position, bookmark) in bookmarks.iter().enumerate() {
            transaction.execute(
                "UPDATE bookmarks SET position = ?1 WHERE id = ?2",
                params![position as i64, bookmark.id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clearing_browsing_records_is_atomic_and_preserves_settings_and_bookmarks() {
        let mut store = super::Store::memory().unwrap();
        let preferences = super::Preferences {
            search_engine: crate::browser::search::SearchEngine::Google,
            ..Default::default()
        };
        store.save_preferences(&preferences).unwrap();
        store
            .save_bookmark(None, "Favorito", "https://example.org/saved")
            .unwrap();
        store
            .record_visit("https://example.org/visited", "Visitado")
            .unwrap();
        let session = super::Session {
            urls: vec!["https://example.org/visited".into()],
            active: 0,
        };
        store.save_session(&session).unwrap();
        store.connection.execute_batch("CREATE TRIGGER reject_clear BEFORE DELETE ON session BEGIN SELECT RAISE(ABORT, 'simulated failure'); END;").unwrap();
        assert!(store.clear_browsing_records().is_err());
        assert_eq!(store.history().unwrap().len(), 1);
        assert_eq!(store.session().unwrap(), session);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_clear")
            .unwrap();
        store.clear_browsing_records().unwrap();
        assert!(store.history().unwrap().is_empty());
        assert!(store.session().unwrap().urls.is_empty());
        assert_eq!(store.preferences().unwrap(), preferences);
        assert_eq!(store.bookmarks().unwrap()[0].title, "Favorito");
    }

    #[test]
    fn session_and_preferences_survive_reopening() {
        let directory = std::env::temp_dir().join(format!(
            "kairo-store-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("profile.sqlite3");
        let session = super::Session {
            urls: vec!["https://example.org/one".into(), "about:blank".into()],
            active: 1,
        };
        let preferences = super::Preferences {
            restore_session: true,
            home_page: "https://example.org/home".into(),
            search_engine: crate::browser::search::SearchEngine::Google,
        };
        {
            let mut store = super::Store::open(&path).unwrap();
            store.save_preferences(&preferences).unwrap();
            store.save_session(&session).unwrap();
            store
                .record_visit("https://example.org/one", "A title ' with quotes")
                .unwrap();
        }
        {
            let store = super::Store::open(&path).unwrap();
            assert_eq!(store.preferences().unwrap(), preferences);
            assert_eq!(store.session().unwrap(), session);
            assert_eq!(store.history().unwrap()[0].title, "A title ' with quotes");
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn disabling_restore_clears_saved_session_and_invalid_writes_preserve_it() {
        let mut store = super::Store::memory().unwrap();
        let session = super::Session {
            urls: vec!["https://example.org".into()],
            active: 0,
        };
        store.save_session(&session).unwrap();
        assert!(
            store
                .save_session(&super::Session {
                    urls: vec!["javascript:alert(1)".into()],
                    active: 0
                })
                .is_err()
        );
        assert_eq!(store.session().unwrap(), session);
        store
            .save_preferences(&super::Preferences {
                restore_session: false,
                ..Default::default()
            })
            .unwrap();
        assert!(store.session().unwrap().urls.is_empty());
    }

    #[test]
    fn history_filters_internal_urls_updates_visits_and_can_be_cleared() {
        let store = super::Store::memory().unwrap();
        for uri in [
            "about:blank",
            crate::browser::start_page::URI,
            "file:///etc/passwd",
            "https://user:pass@example.org",
            "data:text/html,hello",
        ] {
            store.record_visit(uri, "Ignored").unwrap();
        }
        assert!(store.history().unwrap().is_empty());
        store.record_visit("https://example.org", "Old").unwrap();
        store.record_visit("https://example.org", "New").unwrap();
        assert_eq!(store.history().unwrap().len(), 1);
        assert_eq!(store.history().unwrap()[0].title, "New");
        store.clear_history().unwrap();
        assert!(store.history().unwrap().is_empty());
    }

    #[test]
    fn newer_database_schema_is_rejected() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection.pragma_update(None, "user_version", 4).unwrap();
        assert!(super::Store::initialize(connection).is_err());
    }

    #[test]
    fn bookmarks_support_editing_duplicates_and_safe_removal() {
        let mut store = super::Store::memory().unwrap();
        let first = store
            .save_bookmark(None, "First", "https://example.org")
            .unwrap();
        assert_eq!(
            store
                .save_bookmark(None, "Renamed", "https://example.org/")
                .unwrap(),
            first
        );
        assert_eq!(store.bookmarks().unwrap().len(), 1);
        let second = store
            .save_bookmark(None, "Second", "https://example.net/")
            .unwrap();
        assert!(
            store
                .save_bookmark(Some(first), "Conflict", "https://example.net/")
                .is_err()
        );
        assert_eq!(store.bookmarks().unwrap()[0].title, "Renamed");
        store
            .save_bookmark(Some(first), "Edited", "https://example.org/new")
            .unwrap();
        assert_eq!(store.bookmarks().unwrap()[0].uri, "https://example.org/new");
        for uri in [
            "javascript:alert(1)",
            "file:///tmp/file",
            "kairo://start/",
            "https://user:pass@example.org",
            "example.org",
        ] {
            assert!(store.save_bookmark(None, "Invalid", uri).is_err());
        }
        assert!(
            store
                .save_bookmark(None, "  ", "https://example.org")
                .is_err()
        );
        store.clear_history().unwrap();
        assert_eq!(store.bookmarks().unwrap().len(), 2);
        store.remove_bookmark(first).unwrap();
        assert_eq!(store.bookmarks().unwrap()[0].id, second);
    }

    #[test]
    fn bookmark_order_and_edits_survive_reopening() {
        let directory = std::env::temp_dir().join(format!(
            "kairo-bookmarks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("profile.sqlite3");
        {
            let mut store = super::Store::open(&path).unwrap();
            let first = store
                .save_bookmark(None, "First", "https://one.example/")
                .unwrap();
            let second = store
                .save_bookmark(None, "Second", "https://two.example/")
                .unwrap();
            let third = store
                .save_bookmark(None, "Third", "https://three.example/")
                .unwrap();
            store.move_bookmark(third, -1).unwrap();
            assert_eq!(
                store
                    .bookmarks()
                    .unwrap()
                    .iter()
                    .map(|bookmark| bookmark.id)
                    .collect::<Vec<_>>(),
                [first, third, second]
            );
            store.remove_bookmark(first).unwrap();
            store.move_bookmark(third, -1).unwrap();
            store.move_bookmark(second, 1).unwrap();
            store
                .save_bookmark(Some(third), "Edited third", "https://three.example/edited")
                .unwrap();
        }
        {
            let store = super::Store::open(&path).unwrap();
            let items = store.bookmarks().unwrap();
            assert_eq!(
                items
                    .iter()
                    .map(|bookmark| bookmark.title.as_str())
                    .collect::<Vec<_>>(),
                ["Edited third", "Second"]
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn schema_two_migrates_without_changing_preferences_or_history() {
        let mut store = super::Store::memory().unwrap();
        let preferences = super::Preferences {
            search_engine: crate::browser::search::SearchEngine::Google,
            ..Default::default()
        };
        store.save_preferences(&preferences).unwrap();
        store
            .record_visit("https://example.org/", "Existing visit")
            .unwrap();
        store
            .connection
            .execute_batch("DROP TABLE bookmarks; PRAGMA user_version = 2;")
            .unwrap();
        let mut migrated = super::Store::initialize(store.connection).unwrap();
        assert_eq!(migrated.preferences().unwrap(), preferences);
        assert_eq!(migrated.history().unwrap()[0].title, "Existing visit");
        assert!(migrated.bookmarks().unwrap().is_empty());
        migrated
            .save_bookmark(None, "New favorite", "https://example.org/")
            .unwrap();
        assert_eq!(migrated.bookmarks().unwrap().len(), 1);
    }

    #[test]
    fn old_profiles_gain_search_preference_without_losing_data() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE preferences (id INTEGER PRIMARY KEY, restore_session INTEGER NOT NULL, home_page TEXT NOT NULL);
            INSERT INTO preferences VALUES (1, 1, 'https://example.org/custom');
            CREATE TABLE session (position INTEGER PRIMARY KEY, uri TEXT NOT NULL, selected INTEGER NOT NULL);
            INSERT INTO session VALUES (0, 'https://example.org/saved', 1);
            CREATE TABLE history (id INTEGER PRIMARY KEY, uri TEXT NOT NULL UNIQUE, title TEXT NOT NULL, visited_at INTEGER NOT NULL);
            INSERT INTO history VALUES (1, 'https://example.org/visited', 'Existing history', 1234);
            PRAGMA user_version = 1;").unwrap();
        let mut store = super::Store::initialize(connection).unwrap();
        let preferences = store.preferences().unwrap();
        assert_eq!(
            preferences.search_engine,
            crate::browser::search::SearchEngine::DuckDuckGo
        );
        assert_eq!(preferences.home_page, "https://example.org/custom");
        assert_eq!(store.session().unwrap().urls, ["https://example.org/saved"]);
        assert_eq!(store.history().unwrap()[0].title, "Existing history");
        store
            .save_preferences(&super::Preferences {
                search_engine: crate::browser::search::SearchEngine::Google,
                ..preferences
            })
            .unwrap();
        assert_eq!(
            store.preferences().unwrap().search_engine,
            crate::browser::search::SearchEngine::Google
        );
    }

    #[test]
    fn session_transaction_rolls_back_if_insertion_fails() {
        let mut store = super::Store::memory().unwrap();
        let original = super::Session {
            urls: vec!["https://example.org/original".into()],
            active: 0,
        };
        store.save_session(&original).unwrap();
        store.connection.execute_batch("CREATE TRIGGER reject_session BEFORE INSERT ON session WHEN NEW.position = 1 BEGIN SELECT RAISE(ABORT, 'simulated failure'); END;").unwrap();
        let replacement = super::Session {
            urls: vec!["https://example.org/new".into(), "about:blank".into()],
            active: 1,
        };
        assert!(store.save_session(&replacement).is_err());
        assert_eq!(store.session().unwrap(), original);
    }

    #[test]
    fn unsafe_profile_urls_are_not_restored_or_accepted_as_home_page() {
        let mut store = super::Store::memory().unwrap();
        store
            .connection
            .execute(
                "INSERT INTO session VALUES (0, ?1, 1)",
                ["javascript:alert(1)"],
            )
            .unwrap();
        assert_eq!(store.session().unwrap().urls, ["about:blank"]);
        for home_page in [
            "file:///etc/passwd",
            "https://example.org/\0",
            "https://example.org/\n",
        ] {
            assert!(
                store
                    .save_preferences(&super::Preferences {
                        home_page: home_page.into(),
                        ..Default::default()
                    })
                    .is_err()
            );
        }
        assert_eq!(store.preferences().unwrap(), super::Preferences::default());
    }
}
