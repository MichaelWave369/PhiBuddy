//! Local SQLite history and append-only deliberation events.
//! v0.2 is additive: existing v0.1 sessions/messages remain readable.
use std::{fs, path::Path, str::FromStr};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    agents::{Agent, Mode},
    Message, Result,
};

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub mode: Mode,
    pub agent: Agent,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct SavedMessage {
    pub role: String,
    pub actor: Option<String>,
    pub content: String,
}

/// User-entered memory. Source references are user-supplied labels, not verified evidence.
#[derive(Clone, Debug, Serialize)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    pub source_kind: String,
    pub source_ref: Option<String>,
    pub created_at: String,
}

/// Read-only candidate with SQLite-calculated age, not factual validity.
#[derive(Clone, Debug)]
pub struct MemoryCandidate {
    pub note: MemoryRecord,
    pub age_days: Option<i64>,
}

/// A stored snapshot of externally fetched text. Retrieval does not prove truth.
#[derive(Clone, Debug, Serialize)]
pub struct EvidenceRecord {
    pub id: String,
    pub source_url: String,
    pub source_host: String,
    pub content_type: String,
    pub content_sha256: String,
    pub retrieved_at: String,
    pub content: String,
    pub status: String,
}

fn read_evidence_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvidenceRecord> {
    Ok(EvidenceRecord {
        id: row.get(0)?,
        source_url: row.get(1)?,
        source_host: row.get(2)?,
        content_type: row.get(3)?,
        content_sha256: row.get(4)?,
        retrieved_at: row.get(5)?,
        content: row.get(6)?,
        status: "retrieved_not_fact_checked".to_string(),
    })
}

fn read_memory_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRecord> {
    Ok(MemoryRecord {
        id: row.get(0)?,
        content: row.get(1)?,
        source_kind: row.get(2)?,
        source_ref: row.get(3)?,
        created_at: row.get(4)?,
    })
}

/// Stable event names. Adding a variant requires a schema-version review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    TurnStarted,
    MemoryAttached,
    AgentDraft,
    AgentCritique,
    Synthesis,
    SoloAnswer,
    WorkshopPlan,
    WorkshopIdeas,
    WorkshopBuild,
    TurnCompleted,
    TurnFailed,
    TurnCancelled,
    StagePartial,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TurnStarted => "turn_started",
            Self::MemoryAttached => "memory_attached",
            Self::AgentDraft => "agent_draft",
            Self::AgentCritique => "agent_critique",
            Self::Synthesis => "synthesis",
            Self::SoloAnswer => "solo_answer",
            Self::WorkshopPlan => "workshop_plan",
            Self::WorkshopIdeas => "workshop_ideas",
            Self::WorkshopBuild => "workshop_build",
            Self::TurnCompleted => "turn_completed",
            Self::TurnFailed => "turn_failed",
            Self::TurnCancelled => "turn_cancelled",
            Self::StagePartial => "stage_partial",
        }
    }
}

impl FromStr for EventKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "turn_started" => Ok(Self::TurnStarted),
            "memory_attached" => Ok(Self::MemoryAttached),
            "agent_draft" => Ok(Self::AgentDraft),
            "agent_critique" => Ok(Self::AgentCritique),
            "synthesis" => Ok(Self::Synthesis),
            "solo_answer" => Ok(Self::SoloAnswer),
            "workshop_plan" => Ok(Self::WorkshopPlan),
            "workshop_ideas" => Ok(Self::WorkshopIdeas),
            "workshop_build" => Ok(Self::WorkshopBuild),
            "turn_completed" => Ok(Self::TurnCompleted),
            "turn_failed" => Ok(Self::TurnFailed),
            "turn_cancelled" => Ok(Self::TurnCancelled),
            "stage_partial" => Ok(Self::StagePartial),
            _ => Err(format!("unknown event kind: {s}")),
        }
    }
}

/// Immutable record of what was actually sent or returned, not a reconstructed summary.
#[derive(Clone, Debug, Serialize)]
pub struct RecordedEvent {
    pub sequence: i64,
    pub schema_version: i64,
    pub session_id: String,
    pub turn_id: String,
    pub kind: EventKind,
    pub actor: Option<String>,
    pub content: String,
    pub created_at: String,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                mode TEXT NOT NULL,
                agent TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'contribution')),
                actor TEXT,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS messages_session_id_idx ON messages(session_id, id);
            CREATE TABLE IF NOT EXISTS deliberation_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                schema_version INTEGER NOT NULL DEFAULT 1 CHECK(schema_version = 1),
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                turn_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                actor TEXT,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS events_session_idx ON deliberation_events(session_id, sequence);
            CREATE INDEX IF NOT EXISTS events_turn_idx ON deliberation_events(turn_id, sequence);
            CREATE TABLE IF NOT EXISTS memory_notes (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL CHECK(length(trim(content)) > 0),
                source_kind TEXT NOT NULL CHECK(source_kind IN ('user_explicit')),
                source_ref TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS memory_notes_created_idx
                ON memory_notes(created_at);
            CREATE TABLE IF NOT EXISTS external_evidence (
                id TEXT PRIMARY KEY,
                source_url TEXT NOT NULL,
                source_host TEXT NOT NULL,
                content_type TEXT NOT NULL,
                content_sha256 TEXT NOT NULL CHECK(length(content_sha256) = 64),
                retrieved_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                content TEXT NOT NULL CHECK(length(content) > 0)
            );
            CREATE INDEX IF NOT EXISTS external_evidence_retrieved_idx
                ON external_evidence(retrieved_at);",
        )?;
        conn.pragma_update(None, "secure_delete", "ON")?;
        Ok(Self { conn })
    }

    pub fn create_session(&self, mode: Mode, agent: Agent) -> Result<Session> {
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO sessions(id, mode, agent) VALUES (?1, ?2, ?3)",
            params![id, mode.as_str(), agent.as_str()],
        )?;
        self.get_session(&id)?
            .ok_or_else(|| "new session was not found".into())
    }

    pub fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let row: Option<(String, String, String, String)> = self
            .conn
            .query_row(
                "SELECT id, mode, agent, created_at FROM sessions WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        row.map(|(id, mode, agent, created_at)| {
            Ok(Session {
                id,
                mode: Mode::from_str(&mode)?,
                agent: Agent::from_str(&agent)?,
                created_at,
            })
        })
        .transpose()
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM sessions ORDER BY created_at DESC, rowid DESC")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| {
                self.get_session(id)?
                    .ok_or_else(|| "listed session disappeared".into())
            })
            .collect()
    }

    pub fn save_message(
        &self,
        session_id: &str,
        role: &str,
        actor: Option<Agent>,
        content: &str,
    ) -> Result<()> {
        if !["user", "assistant", "contribution"].contains(&role) {
            return Err("invalid message role".into());
        }
        self.conn.execute(
            "INSERT INTO messages(session_id, role, actor, content) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, role, actor.map(Agent::as_str), content],
        )?;
        Ok(())
    }

    pub fn messages(&self, session_id: &str) -> Result<Vec<SavedMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT role, actor, content FROM messages WHERE session_id = ?1 ORDER BY id ASC",
        )?;
        let messages = stmt
            .query_map([session_id], |r| {
                Ok(SavedMessage {
                    role: r.get(0)?,
                    actor: r.get(1)?,
                    content: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    /// Only final assistant turns enter future context; intermediate drafts cannot leak as history.
    pub fn conversation(&self, session_id: &str) -> Result<Vec<Message>> {
        Ok(self
            .messages(session_id)?
            .into_iter()
            .filter(|m| m.role == "user" || m.role == "assistant")
            .map(|m| Message::new(m.role, m.content))
            .collect())
    }

    /// Store only a text response that the desktop explicitly retrieved on the
    /// user's instruction. Caller must enforce HTTPS / host policy and hash.
    /// Nothing in this table becomes model context automatically.
    pub fn record_external_evidence(
        &self,
        source_url: &str,
        source_host: &str,
        content_type: &str,
        content_sha256: &str,
        content: &str,
    ) -> Result<EvidenceRecord> {
        if source_url.is_empty()
            || source_url.len() > 2048
            || source_host.is_empty()
            || source_host.len() > 253
            || content_type.is_empty()
            || content_type.len() > 128
            || content.is_empty()
            || content.len() > 262_144
            || content.contains('\0')
            || content_sha256.len() != 64
            || !content_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("external evidence fields failed size or integrity-format checks".into());
        }
        // Reject mismatched fingerprints rather than storing a plausible
        // looking but incorrect hash beside an external document.
        let computed_sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
        if computed_sha256 != content_sha256 {
            return Err("external evidence SHA-256 does not match captured text".into());
        }
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO external_evidence
             (id, source_url, source_host, content_type, content_sha256, content)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                source_url,
                source_host,
                content_type,
                content_sha256,
                content
            ],
        )?;
        self.get_external_evidence(&id)?
            .ok_or_else(|| "saved evidence disappeared".into())
    }

    pub fn get_external_evidence(&self, id: &str) -> Result<Option<EvidenceRecord>> {
        Ok(self.conn.query_row(
            "SELECT id, source_url, source_host, content_type, content_sha256, retrieved_at, content
             FROM external_evidence WHERE id = ?1",
            [id], read_evidence_row,
        ).optional()?)
    }

    pub fn list_external_evidence(&self, limit: usize) -> Result<Vec<EvidenceRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_url, source_host, content_type, content_sha256, retrieved_at, content
             FROM external_evidence ORDER BY retrieved_at DESC, rowid DESC LIMIT ?1",
        )?;
        let entries = stmt
            .query_map([limit.clamp(1, 25) as i64], read_evidence_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    pub fn delete_external_evidence(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM external_evidence WHERE id = ?1", [id])?
            > 0)
    }

    /// Explicit memory entry. Nothing is automatically extracted from model replies.
    pub fn remember_manual(&self, content: &str, source_ref: Option<&str>) -> Result<MemoryRecord> {
        let content = content.trim();
        if content.is_empty() || content.chars().count() > 4096 {
            return Err("memory must be between 1 and 4096 characters".into());
        }
        let source_ref = source_ref.map(str::trim).filter(|value| !value.is_empty());
        if source_ref.is_some_and(|value| value.chars().count() > 1024) {
            return Err("memory source label exceeds 1024 characters".into());
        }
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO memory_notes(id, content, source_kind, source_ref)
             VALUES (?1, ?2, 'user_explicit', ?3)",
            params![id, content, source_ref],
        )?;
        self.get_memory(&id)?
            .ok_or_else(|| "saved memory disappeared".into())
    }

    pub fn get_memory(&self, id: &str) -> Result<Option<MemoryRecord>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, content, source_kind, source_ref, created_at
             FROM memory_notes WHERE id = ?1",
                [id],
                read_memory_row,
            )
            .optional()?)
    }

    pub fn list_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT id, content, source_kind, source_ref, created_at
             FROM memory_notes ORDER BY created_at DESC, rowid DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit], read_memory_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// A literal substring lookup (case-insensitive for SQLite's built-in ASCII rules).
    /// Deliberately no semantic embeddings or automatic prompt insertion.
    pub fn search_memories(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 1024 {
            return Err("memory search requires a query of 1 to 1024 characters".into());
        }
        let limit = limit.clamp(1, 200) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT id, content, source_kind, source_ref, created_at
             FROM memory_notes WHERE instr(lower(content), lower(?1)) > 0
             ORDER BY created_at DESC, rowid DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![query, limit], read_memory_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Read a bounded local candidate pool for manual relevance suggestions.
    /// Does not attach notes to prompts or create a session.
    pub fn suggestion_candidates(&self) -> Result<Vec<MemoryCandidate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, source_kind, source_ref, created_at,
                    CAST(julianday('now') - julianday(created_at) AS INTEGER)
             FROM memory_notes ORDER BY created_at DESC, rowid DESC LIMIT 1000",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let note = read_memory_row(row)?;
                let age_days = row.get::<_, Option<i64>>(5)?.map(|age| age.max(0));
                Ok(MemoryCandidate { note, age_days })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Deletes a record from the active database. Backups and forensic remnants
    /// are outside this API; never claim guaranteed complete erasure.
    pub fn forget_memory(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM memory_notes WHERE id = ?1", [id])?
            > 0)
    }

    pub fn new_turn_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Append immediately after a stage, preserving progress even when a later model call fails.
    pub fn append_event(
        &self,
        session_id: &str,
        turn_id: &str,
        kind: EventKind,
        actor: Option<Agent>,
        content: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO deliberation_events(session_id, turn_id, kind, actor, content)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                turn_id,
                kind.as_str(),
                actor.map(Agent::as_str),
                content
            ],
        )?;
        Ok(())
    }

    /// A completed conversation turn is committed together, never as half a user/assistant pair.
    /// Deliberation events, which are already durable, remain visible independently.
    pub fn finish_turn(
        &mut self,
        session_id: &str,
        turn_id: &str,
        user_input: &str,
        contributions: &[(Agent, String)],
        answer: &str,
        final_agent: Agent,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO messages(session_id, role, content) VALUES (?1, 'user', ?2)",
            params![session_id, user_input],
        )?;
        for (agent, content) in contributions {
            tx.execute(
                "INSERT INTO messages(session_id, role, actor, content)
                        VALUES (?1, 'contribution', ?2, ?3)",
                params![session_id, agent.as_str(), content],
            )?;
        }
        tx.execute(
            "INSERT INTO messages(session_id, role, actor, content)
                    VALUES (?1, 'assistant', ?2, ?3)",
            params![session_id, final_agent.as_str(), answer],
        )?;
        tx.execute(
            "INSERT INTO deliberation_events(session_id, turn_id, kind, actor, content)
                    VALUES (?1, ?2, ?3, ?4, '')",
            params![
                session_id,
                turn_id,
                EventKind::TurnCompleted.as_str(),
                final_agent.as_str()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn events(&self, session_id: &str) -> Result<Vec<RecordedEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT sequence, schema_version, session_id, turn_id, kind, actor, content, created_at
             FROM deliberation_events WHERE session_id = ?1 ORDER BY sequence ASC",
        )?;
        let raw = stmt
            .query_map([session_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        raw.into_iter()
            .map(
                |(
                    sequence,
                    schema_version,
                    session_id,
                    turn_id,
                    kind,
                    actor,
                    content,
                    created_at,
                )| {
                    Ok(RecordedEvent {
                        sequence,
                        schema_version,
                        session_id,
                        turn_id,
                        kind: EventKind::from_str(&kind)?,
                        actor,
                        content,
                        created_at,
                    })
                },
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_and_messages_persist() {
        let store = Store::in_memory().unwrap();
        let session = store.create_session(Mode::Council, Agent::Buddy).unwrap();
        store
            .save_message(&session.id, "user", None, "Hello")
            .unwrap();
        store
            .save_message(&session.id, "contribution", Some(Agent::Homie), "draft")
            .unwrap();
        store
            .save_message(&session.id, "assistant", Some(Agent::Buddy), "reply")
            .unwrap();
        assert_eq!(
            store.get_session(&session.id).unwrap().unwrap().mode,
            Mode::Council
        );
        assert_eq!(store.messages(&session.id).unwrap().len(), 3);
        assert_eq!(store.conversation(&session.id).unwrap().len(), 2);
        assert_eq!(store.list_sessions().unwrap().len(), 1);
    }

    #[test]
    fn event_log_is_ordered_and_serializable() {
        let store = Store::in_memory().unwrap();
        let session = store.create_session(Mode::Solo, Agent::Buddy).unwrap();
        let turn = Store::new_turn_id();
        store
            .append_event(&session.id, &turn, EventKind::TurnStarted, None, "hello")
            .unwrap();
        store
            .append_event(
                &session.id,
                &turn,
                EventKind::SoloAnswer,
                Some(Agent::Buddy),
                "hi",
            )
            .unwrap();
        let events = store.events(&session.id).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[0].sequence < events[1].sequence);
        let json = serde_json::to_string(&events[1]).unwrap();
        assert!(json.contains("solo_answer"));
        assert_eq!(events[0].schema_version, 1);
    }

    #[test]
    fn explicit_memory_is_source_labeled_searchable_and_deletable() {
        let store = Store::in_memory().unwrap();
        let memory = store
            .remember_manual(
                "Buddy should challenge unsupported claims",
                Some("my notes"),
            )
            .unwrap();
        assert_eq!(memory.source_kind, "user_explicit");
        assert_eq!(memory.source_ref.as_deref(), Some("my notes"));
        assert_eq!(store.list_memories(10).unwrap().len(), 1);
        assert_eq!(store.search_memories("UNSUPPORTED", 5).unwrap().len(), 1);
        assert!(store.search_memories("unrelated", 5).unwrap().is_empty());
        assert!(store.forget_memory(&memory.id).unwrap());
        assert!(!store.forget_memory(&memory.id).unwrap());
        assert!(store.list_memories(10).unwrap().is_empty());
    }

    #[test]
    fn manual_memory_survives_restart_and_forget_remains_deleted() {
        let tmp =
            std::env::temp_dir().join(format!("phibuddy-memory-reopen-{}.sqlite3", Uuid::new_v4()));
        let id = {
            let store = Store::open(&tmp).unwrap();
            store
                .remember_manual("A remembered fact with its own source", Some("test note"))
                .unwrap()
                .id
        };
        {
            let store = Store::open(&tmp).unwrap();
            let note = store.get_memory(&id).unwrap().unwrap();
            assert_eq!(note.content, "A remembered fact with its own source");
            assert_eq!(note.source_kind, "user_explicit");
            assert!(store.forget_memory(&id).unwrap());
        }
        {
            let store = Store::open(&tmp).unwrap();
            assert!(store.get_memory(&id).unwrap().is_none());
        }
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(tmp.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(tmp.with_extension("sqlite3-shm"));
    }

    #[test]
    fn invalid_memory_inputs_are_rejected_without_writes() {
        let store = Store::in_memory().unwrap();
        assert!(store.remember_manual("   ", None).is_err());
        assert!(store.remember_manual(&"x".repeat(4097), None).is_err());
        assert!(store
            .remember_manual("valid", Some(&"x".repeat(1025)))
            .is_err());
        assert!(store.search_memories(" ", 10).is_err());
        assert!(store.list_memories(10).unwrap().is_empty());
    }

    #[test]
    fn external_evidence_is_separate_from_manual_memories_and_deletable() {
        let store = Store::in_memory().unwrap();
        let record = store
            .record_external_evidence(
                "https://www.rfc-editor.org/rfc/rfc9111.txt",
                "www.rfc-editor.org",
                "text/plain",
                &format!("{:x}", Sha256::digest(b"RFC example body")),
                "RFC example body",
            )
            .unwrap();
        assert_eq!(record.status, "retrieved_not_fact_checked");
        assert_eq!(record.content, "RFC example body");
        assert_eq!(store.list_external_evidence(10).unwrap().len(), 1);
        assert!(store.list_memories(10).unwrap().is_empty());
        assert!(store.list_sessions().unwrap().is_empty());
        assert!(store.delete_external_evidence(&record.id).unwrap());
        assert!(store.get_external_evidence(&record.id).unwrap().is_none());
        assert!(!store.delete_external_evidence(&record.id).unwrap());
    }

    #[test]
    fn evidence_fingerprint_mismatch_is_rejected_and_snapshot_survives_reopen() {
        let path =
            std::env::temp_dir().join(format!("phibuddy-evidence-{}.sqlite3", Uuid::new_v4()));
        let id = {
            let store = Store::open(&path).unwrap();
            assert!(store
                .record_external_evidence(
                    "https://www.rfc-editor.org/rfc/rfc9111.txt",
                    "www.rfc-editor.org",
                    "text/plain",
                    &"0".repeat(64),
                    "A verified byte string",
                )
                .is_err());
            store
                .record_external_evidence(
                    "https://www.rfc-editor.org/rfc/rfc9111.txt",
                    "www.rfc-editor.org",
                    "text/plain",
                    &format!("{:x}", Sha256::digest(b"Example source text")),
                    "Example source text",
                )
                .unwrap()
                .id
        };
        {
            let store = Store::open(&path).unwrap();
            let snapshot = store.get_external_evidence(&id).unwrap().unwrap();
            assert_eq!(snapshot.content, "Example source text");
            assert!(store.delete_external_evidence(&id).unwrap());
        }
        {
            let store = Store::open(&path).unwrap();
            assert!(store.get_external_evidence(&id).unwrap().is_none());
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    }

    #[test]
    fn external_evidence_input_limits_and_schema_addition() {
        let store = Store::in_memory().unwrap();
        assert!(store
            .record_external_evidence("url", "host", "text/plain", "invalid", "text")
            .is_err());
        assert!(store
            .record_external_evidence("url", "host", "text/plain", &"0".repeat(64), "")
            .is_err());
        assert!(store
            .record_external_evidence(
                "url",
                "host",
                "text/plain",
                &"0".repeat(64),
                &"x".repeat(262_145),
            )
            .is_err());
        assert!(store.list_external_evidence(10).unwrap().is_empty());
    }

    #[test]
    fn old_schema_migrates_without_losing_messages() {
        let tmp =
            std::env::temp_dir().join(format!("phibuddy-migration-{}.sqlite3", Uuid::new_v4()));
        {
            let legacy = Connection::open(&tmp).unwrap();
            legacy.execute_batch("CREATE TABLE sessions (id TEXT PRIMARY KEY, mode TEXT NOT NULL, agent TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
                CREATE TABLE messages (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE, role TEXT NOT NULL, actor TEXT, content TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
                INSERT INTO sessions(id, mode, agent) VALUES ('old', 'solo', 'buddy');
                INSERT INTO messages(session_id, role, content) VALUES ('old', 'user', 'old hello');").unwrap();
        }
        {
            let store = Store::open(&tmp).unwrap();
            assert_eq!(store.messages("old").unwrap()[0].content, "old hello");
            store
                .append_event("old", "t", EventKind::TurnStarted, None, "new")
                .unwrap();
            assert_eq!(store.events("old").unwrap().len(), 1);
        }
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(tmp.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(tmp.with_extension("sqlite3-shm"));
    }
}
