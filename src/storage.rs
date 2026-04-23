use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::Value;
use std::path::Path;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone)]
pub struct Event {
    pub session_id: String,
    pub event_type: String,
    pub payload: Value,
    pub ts: i64,
}

#[derive(Debug, Clone)]
pub struct FileAccess {
    pub session_id: String,
    pub path: String,
    pub operation: String,
    pub ts: i64,
}

#[derive(Debug, Clone)]
pub struct UserPrompt {
    pub session_id: String,
    pub prompt: String,
    pub ts: i64,
}

pub enum WriteCmd {
    Event(Event),
    FileAccess(FileAccess),
    UserPrompt(UserPrompt),
}

#[derive(Clone)]
pub struct StorageHandle {
    tx: mpsc::Sender<WriteCmd>,
    db_path: String,
}

impl StorageHandle {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db_path = path.as_ref().to_string_lossy().into_owned();
        let conn = open_conn(&db_path)?;
        migrate(&conn)?;

        let (tx, rx) = mpsc::channel::<WriteCmd>(4096);
        tokio::spawn(writer_task(db_path.clone(), rx));

        Ok(Self { tx, db_path })
    }

    pub async fn write_event(&self, ev: Event) {
        let _ = self.tx.send(WriteCmd::Event(ev)).await;
    }

    pub async fn write_file_access(&self, fa: FileAccess) {
        let _ = self.tx.send(WriteCmd::FileAccess(fa)).await;
    }

    pub async fn write_user_prompt(&self, up: UserPrompt) {
        let _ = self.tx.send(WriteCmd::UserPrompt(up)).await;
    }

    pub fn query_user_prompts(&self, session_id: &str, limit: usize) -> Result<Vec<Value>> {
        let conn = open_conn(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT prompt, ts FROM user_prompts WHERE session_id = ?1 ORDER BY ts DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit as i64], |row| {
            Ok(serde_json::json!({
                "prompt": row.get::<_, String>(0)?,
                "ts": row.get::<_, i64>(1)?,
            }))
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }


    pub fn query_sessions(&self, limit: usize) -> Result<Vec<Value>> {
        let conn = open_conn(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, started_at, event_count FROM sessions ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "started_at": row.get::<_, i64>(1)?,
                "event_count": row.get::<_, i64>(2)?,
            }))
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn query_events(&self, session_id: &str, limit: usize) -> Result<Vec<Value>> {
        let conn = open_conn(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, event_type, payload, ts FROM events WHERE session_id = ?1 ORDER BY ts DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit as i64], |row| {
            let payload_str: String = row.get(2)?;
            let payload: Value = serde_json::from_str(&payload_str).unwrap_or(Value::Null);
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "event_type": row.get::<_, String>(1)?,
                "payload": payload,
                "ts": row.get::<_, i64>(3)?,
            }))
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn query_file_accesses(&self, session_id: &str, limit: usize) -> Result<Vec<Value>> {
        let conn = open_conn(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT path, operation, ts FROM file_accesses WHERE session_id = ?1 ORDER BY ts DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit as i64], |row| {
            Ok(serde_json::json!({
                "path": row.get::<_, String>(0)?,
                "operation": row.get::<_, String>(1)?,
                "ts": row.get::<_, i64>(2)?,
            }))
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }
}

fn open_conn(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            started_at INTEGER NOT NULL,
            event_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            payload TEXT NOT NULL,
            ts INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS events_session ON events(session_id, ts);
        CREATE TABLE IF NOT EXISTS file_accesses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            path TEXT NOT NULL,
            operation TEXT NOT NULL,
            ts INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS fa_session ON file_accesses(session_id, ts);
        CREATE TABLE IF NOT EXISTS user_prompts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            prompt TEXT NOT NULL,
            ts INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS up_session ON user_prompts(session_id, ts);",
    )?;
    Ok(())
}

async fn writer_task(db_path: String, mut rx: mpsc::Receiver<WriteCmd>) {
    let mut ticker = interval(Duration::from_millis(100));
    let mut buf: Vec<WriteCmd> = Vec::with_capacity(128);

    loop {
        tokio::select! {
            cmd = rx.recv() => {
                match cmd {
                    Some(c) => buf.push(c),
                    None => {
                        flush(&db_path, &mut buf);
                        return;
                    }
                }
            }
            _ = ticker.tick() => {
                if !buf.is_empty() {
                    flush(&db_path, &mut buf);
                }
            }
        }
    }
}

fn flush(db_path: &str, buf: &mut Vec<WriteCmd>) {
    let conn = match open_conn(db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("storage flush open error: {e}");
            return;
        }
    };

    if let Err(e) = flush_inner(&conn, buf) {
        tracing::error!("storage flush error: {e}");
    }
    buf.clear();
}

fn flush_inner(conn: &Connection, buf: &[WriteCmd]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for cmd in buf {
        match cmd {
            WriteCmd::Event(ev) => {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT OR IGNORE INTO sessions(id, started_at) VALUES(?1, ?2)",
                    params![ev.session_id, ev.ts],
                )?;
                conn.execute(
                    "UPDATE sessions SET event_count = event_count + 1 WHERE id = ?1",
                    params![ev.session_id],
                )?;
                conn.execute(
                    "INSERT INTO events(id, session_id, event_type, payload, ts) VALUES(?1,?2,?3,?4,?5)",
                    params![id, ev.session_id, ev.event_type, ev.payload.to_string(), ev.ts],
                )?;
            }
            WriteCmd::FileAccess(fa) => {
                conn.execute(
                    "INSERT INTO file_accesses(session_id, path, operation, ts) VALUES(?1,?2,?3,?4)",
                    params![fa.session_id, fa.path, fa.operation, fa.ts],
                )?;
            }
            WriteCmd::UserPrompt(up) => {
                conn.execute(
                    "INSERT INTO user_prompts(session_id, prompt, ts) VALUES(?1,?2,?3)",
                    params![up.session_id, up.prompt, up.ts],
                )?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}
