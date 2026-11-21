//! Tauri commands are intentionally narrow and do not expose OS tools.
//! The CLI and desktop both call the same phibuddy-core engine and SQLite store.

mod evidence_fetch;

use phibuddy_core::{
    agents::{Agent, Mode},
    claim_support::{review_claim, ClaimReview},
    engine::{CouncilDepth, Engine, TurnReferences},
    evidence_passages::{preview_passage, PassageAnchor, PassagePreview, PassageSelection},
    memory_discovery::{rank_suggestions, MemorySuggestion},
    progress::{check_cancel, CancelFlag, TurnProgress},
    store::{EvidenceRecord, EventKind, MemoryRecord, RecordedEvent, SavedMessage, Session, Store},
    Message, ModelProvider,
};
use reqwest::{blocking::Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    env,
    io::{BufRead, BufReader},
    path::PathBuf,
    str::FromStr,
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};
use tauri::{ipc::Channel, Manager, State};

struct Ollama {
    model: String,
    client: Client,
    endpoint: Url,
}

#[derive(Deserialize)]
struct OllamaReply { message: OllamaMessage }

#[derive(Deserialize)]
struct OllamaMessage { content: String }

#[derive(Deserialize)]
struct OllamaStreamChunk {
    message: Option<OllamaMessage>,
    done: Option<bool>,
    error: Option<String>,
}

/// Fail closed for obvious cloud-backed Ollama model tags. This is a guard,
/// not proof that a renamed alias or provider uses only local inference.
fn validate_model_name(model: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if model.trim().is_empty() || model.to_ascii_lowercase().contains("cloud") {
        return Err("PhiBuddy desktop preview requires a local model name; cloud tags are disabled".into());
    }
    Ok(model)
}

impl Ollama {
    fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            model: validate_model_name(
                env::var("PHIBUDDY_MODEL").unwrap_or_else(|_| "qwen3:4b".to_string())
            )?,
            endpoint: Url::parse("http://127.0.0.1:11434/")?,
            client: Client::builder().timeout(Duration::from_secs(180)).build()?,
        })
    }

    fn check(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let reply: serde_json::Value = self.client.get(self.endpoint.join("api/tags")?)
            .timeout(Duration::from_secs(5)).send()?.error_for_status()?.json()?;
        if !reply.get("models").and_then(serde_json::Value::as_array).is_some() {
            return Err("unexpected response from local Ollama /api/tags".into());
        }
        Ok(format!("Ollama connected · {}", self.model))
    }
}

/// Decode NDJSON frames independently of HTTP for deterministic testing.
fn read_stream<R: BufRead>(
    reader: R,
    cancel: &CancelFlag,
    on_delta: &mut dyn FnMut(&str),
) -> phibuddy_core::Result<String> {
    let mut text = String::new();
    let mut finished = false;
    for line in reader.lines() {
        check_cancel(cancel)?;
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let frame: OllamaStreamChunk = serde_json::from_str(&line)?;
        if let Some(error) = frame.error {
            return Err(format!("Ollama stream error: {error}").into());
        }
        if let Some(message) = frame.message {
            if !message.content.is_empty() {
                text.push_str(&message.content);
                on_delta(&message.content);
            }
        }
        if frame.done.unwrap_or(false) {
            finished = true;
            break;
        }
    }
    check_cancel(cancel)?;
    if !finished {
        return Err("Ollama stream ended before a done frame".into());
    }
    if text.trim().is_empty() {
        return Err("Ollama returned an empty response".into());
    }
    Ok(text)
}

impl ModelProvider for Ollama {
    fn complete(&self, messages: &[Message]) -> phibuddy_core::Result<String> {
        let payload = messages.iter().map(|m| json!({
            "role": m.role, "content": m.content
        })).collect::<Vec<_>>();
        let result: OllamaReply = self.client.post(self.endpoint.join("api/chat")?)
            .json(&json!({"model": self.model, "messages": payload, "stream": false}))
            .send()?.error_for_status()?.json()?;
        if result.message.content.trim().is_empty() {
            return Err("Ollama returned an empty response".into());
        }
        Ok(result.message.content)
    }

    fn complete_streaming(
        &self,
        messages: &[Message],
        cancel: &CancelFlag,
        on_delta: &mut dyn FnMut(&str),
    ) -> phibuddy_core::Result<String> {
        check_cancel(cancel)?;
        let payload = messages.iter().map(|m| json!({
            "role": m.role, "content": m.content
        })).collect::<Vec<_>>();
        let response = self.client.post(self.endpoint.join("api/chat")?)
            .json(&json!({"model": self.model, "messages": payload, "stream": true}))
            .send()?.error_for_status()?;
        read_stream(BufReader::new(response), cancel, on_delta)
    }
}

struct DesktopState {
    engine: Arc<Mutex<Engine<Ollama>>>,
    active_cancel: Arc<Mutex<Option<Arc<CancelFlag>>>>,
}

#[derive(Serialize)]
struct SessionView {
    id: String,
    mode: String,
    agent: String,
    created_at: String,
}

impl From<Session> for SessionView {
    fn from(s: Session) -> Self {
        Self {
            id: s.id,
            mode: s.mode.as_str().to_string(),
            agent: s.agent.as_str().to_string(),
            created_at: s.created_at,
        }
    }
}

#[derive(Serialize)]
struct MessageView {
    role: String,
    actor: Option<String>,
    content: String,
}

impl From<SavedMessage> for MessageView {
    fn from(m: SavedMessage) -> Self {
        Self { role: m.role, actor: m.actor, content: m.content }
    }
}

/// Metadata is resolved from the active memory ledger each time;
/// deleted notes appear as missing, rather than retaining an undeletable copy.
#[derive(Serialize)]
struct TurnMemoryRef {
    id: String,
    memory: Option<MemoryRecord>,
}

#[derive(Serialize)]
struct TurnPassageRef {
    anchor: PassageAnchor,
    preview: Option<PassagePreview>,
}

#[derive(Serialize)]
struct ContributionView {
    agent: String,
    phase: String,
    text: String,
}

#[derive(Serialize)]
struct TurnView {
    turn_id: String,
    answer: String,
    contributions: Vec<ContributionView>,
}

fn default_db() -> PathBuf {
    // Opt-in test isolation: do not make a separate source checkout silently
    // share the user's real database when a test path was specified.
    if let Some(path) = env::var_os("PHIBUDDY_DB_PATH") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    if let Some(dir) = env::var_os("LOCALAPPDATA") {
        return PathBuf::from(dir).join("PhiBuddy").join("buddy.sqlite3");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/phibuddy/buddy.sqlite3");
    }
    PathBuf::from("phibuddy.sqlite3")
}

#[tauri::command]
fn get_sessions(state: State<'_, DesktopState>) -> Result<Vec<SessionView>, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    engine.store().list_sessions().map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn create_session(state: State<'_, DesktopState>, mode: String, agent: String) -> Result<SessionView, String> {
    let mode = Mode::from_str(&mode)?;
    let agent = Agent::from_str(&agent)?;
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    engine.store().create_session(mode, agent).map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_messages(state: State<'_, DesktopState>, session_id: String) -> Result<Vec<MessageView>, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    if engine.store().get_session(&session_id).map_err(|e| e.to_string())?.is_none() {
        return Err("unknown session ID".into());
    }
    engine.store().messages(&session_id).map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_events(state: State<'_, DesktopState>, session_id: String) -> Result<Vec<RecordedEvent>, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    if engine.store().get_session(&session_id).map_err(|e| e.to_string())?.is_none() {
        return Err("unknown session ID".into());
    }
    engine.store().events(&session_id).map_err(|e| e.to_string())
}

/// Read explicit user-controlled notes. Search is a simple literal substring.
#[tauri::command]
fn list_memories(
    state: State<'_, DesktopState>,
    query: Option<String>,
) -> Result<Vec<MemoryRecord>, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    match query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(query) => engine.store().search_memories(query, 100).map_err(|e| e.to_string()),
        None => engine.store().list_memories(100).map_err(|e| e.to_string()),
    }
}

/// Suggestions are computed only when explicitly requested. They are never
/// automatically attached to an Ollama request or written into chat history.
#[tauri::command]
fn suggest_memories(
    state: State<'_, DesktopState>,
    query: String,
) -> Result<Vec<MemorySuggestion>, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    let candidates = engine.store().suggestion_candidates().map_err(|e| e.to_string())?;
    rank_suggestions(&query, candidates, 8).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_memory(
    state: State<'_, DesktopState>,
    content: String,
    source_ref: Option<String>,
) -> Result<MemoryRecord, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    engine.store().remember_manual(&content, source_ref.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_memory(state: State<'_, DesktopState>, memory_id: String) -> Result<bool, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    engine.store().forget_memory(&memory_id).map_err(|e| e.to_string())
}

/// The event log records only note IDs. Resolve the labels and content from
/// the current ledger, so a deleted note is not recreated by the inspector.
fn resolve_turn_memories(
    store: &Store,
    session_id: &str,
    turn_id: &str,
) -> Result<Vec<TurnMemoryRef>, String> {
    if store.get_session(session_id).map_err(|e| e.to_string())?.is_none() {
        return Err("unknown session ID".into());
    }
    let events = store.events(session_id).map_err(|e| e.to_string())?;
    events.into_iter().filter(|event| {
        event.turn_id == turn_id && event.kind == EventKind::MemoryAttached
    }).map(|event| {
        let memory = store.get_memory(&event.content).map_err(|e| e.to_string())?;
        Ok(TurnMemoryRef { id: event.content, memory })
    }).collect()
}

#[tauri::command]
fn get_turn_memories(
    state: State<'_, DesktopState>,
    session_id: String,
    turn_id: String,
) -> Result<Vec<TurnMemoryRef>, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    resolve_turn_memories(engine.store(), &session_id, &turn_id)
}

/// This is a deliberate user action, never agent-executed browsing. Network
/// fetch happens outside the SQLite/engine lock, preserving cancellation.
#[tauri::command]
async fn fetch_external_evidence(
    state: State<'_, DesktopState>,
    url: String,
) -> Result<EvidenceRecord, String> {
    let engine = state.engine.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fetched = evidence_fetch::fetch_text(&url)?;
        let engine = engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
        engine.store().record_external_evidence(
            &fetched.url,
            &fetched.host,
            &fetched.content_type,
            &fetched.sha256,
            &fetched.content,
        ).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn list_external_evidence(state: State<'_, DesktopState>) -> Result<Vec<EvidenceRecord>, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    engine.store().list_external_evidence(20).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_external_evidence(
    state: State<'_, DesktopState>,
    evidence_id: String,
) -> Result<bool, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    engine.store().delete_external_evidence(&evidence_id).map_err(|e| e.to_string())
}

/// A preview is a local SQLite read only: no URL fetching or model inference.
#[tauri::command]
fn preview_evidence_passage(
    state: State<'_, DesktopState>,
    evidence_id: String,
    start_line: usize,
    end_line: usize,
) -> Result<PassagePreview, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    preview_passage(engine.store(), &PassageSelection {
        evidence_id, start_line, end_line,
    }).map_err(|e| e.to_string())
}

fn resolve_turn_passages(
    store: &Store,
    session_id: &str,
    turn_id: &str,
) -> Result<Vec<TurnPassageRef>, String> {
    if store.get_session(session_id).map_err(|e| e.to_string())?.is_none() {
        return Err("unknown session ID".into());
    }
    store.events(session_id).map_err(|e| e.to_string())?
        .into_iter()
        .filter(|e| e.turn_id == turn_id && e.kind == EventKind::SourcePassageAttached)
        .map(|e| {
            let anchor: PassageAnchor = serde_json::from_str(&e.content)
                .map_err(|error| format!("malformed source anchor: {error}"))?;
            // A changed or deleted source cannot be represented as the original quote.
            let preview = preview_passage(store, &anchor.selection).ok()
                .filter(|p| p.sha256 == anchor.sha256);
            Ok(TurnPassageRef { anchor, preview })
        }).collect()
}

/// Explicit read-only text check. No network, model call or saved verdict.
#[tauri::command]
fn review_turn_claim(
    state: State<'_, DesktopState>,
    session_id: String,
    turn_id: String,
    claim: String,
) -> Result<ClaimReview, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    review_claim(engine.store(), &session_id, &turn_id, &claim).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_turn_passages(
    state: State<'_, DesktopState>,
    session_id: String,
    turn_id: String,
) -> Result<Vec<TurnPassageRef>, String> {
    let engine = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    resolve_turn_passages(engine.store(), &session_id, &turn_id)
}

#[tauri::command]
async fn check_ollama(state: State<'_, DesktopState>) -> Result<String, String> {
    let engine = state.engine.clone();
    tauri::async_runtime::spawn_blocking(move || {
        engine.lock().map_err(|_| "engine lock poisoned".to_string())?
            .provider().check().map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

/// Independent of the engine lock: this can stop a run while Ollama is generating.
#[tauri::command]
fn cancel_turn(state: State<'_, DesktopState>) -> Result<bool, String> {
    let active = state.active_cancel.lock().map_err(|_| "cancellation lock poisoned".to_string())?;
    if let Some(cancel) = active.as_ref() {
        cancel.store(true, Ordering::Release);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
async fn send_prompt(
    state: State<'_, DesktopState>,
    session_id: String,
    input: String,
    depth: String,
    memory_ids: Vec<String>,
    passage_selections: Vec<PassageSelection>,
    on_progress: Channel<TurnProgress>,
) -> Result<TurnView, String> {
    let depth = CouncilDepth::from_str(&depth)?;
    if input.trim().is_empty() || input.chars().count() > 20_000 {
        return Err("enter a prompt between 1 and 20,000 characters".into());
    }
    let cancel = Arc::new(CancelFlag::new(false));
    {
        let mut active = state.active_cancel.lock().map_err(|_| "cancellation lock poisoned".to_string())?;
        if active.is_some() {
            return Err("another turn is already running".into());
        }
        *active = Some(cancel.clone());
    }
    let active = state.active_cancel.clone();
    let engine = state.engine.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let mut engine = engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
        let session = engine.store().get_session(&session_id)
            .map_err(|e| e.to_string())?.ok_or("unknown session ID")?;
        let disconnected = cancel.clone();
        let turn = engine.run_observed_with_references(
            &session,
            &input,
            depth,
            TurnReferences {
                memory_ids: &memory_ids,
                passages: &passage_selections,
            },
            &cancel,
            &mut |progress| {
                if on_progress.send(progress).is_err() {
                    disconnected.store(true, Ordering::Release);
                }
            },
        ).map_err(|e| e.to_string())?;
        Ok::<TurnView, String>(TurnView {
            turn_id: turn.turn_id,
            answer: turn.answer,
            contributions: turn.contributions.into_iter().map(|c| ContributionView {
                agent: c.agent.as_str().to_string(),
                phase: c.phase.as_str().to_string(),
                text: c.text,
            }).collect(),
        })
    }).await;
    // The slot is released after success, failure, or a worker panic.
    *active.lock().map_err(|_| "cancellation lock poisoned".to_string())? = None;
    task.map_err(|e| e.to_string())?
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let provider = Ollama::new().map_err(|e| -> Box<dyn std::error::Error> { e })?;
            let store = Store::open(default_db()).map_err(|e| -> Box<dyn std::error::Error> { e })?;
            app.manage(DesktopState {
                engine: Arc::new(Mutex::new(Engine::new(provider, store))),
                active_cancel: Arc::new(Mutex::new(None)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sessions, create_session, get_messages, get_events,
            check_ollama, send_prompt, cancel_turn,
            list_memories, save_memory, delete_memory, get_turn_memories,
            suggest_memories,
            fetch_external_evidence, list_external_evidence, delete_external_evidence,
            preview_evidence_passage, get_turn_passages, review_turn_claim
        ])
        .run(tauri::generate_context!())
        .expect("PhiBuddy desktop startup failed");
}

#[cfg(test)]
mod tests {
    use super::{read_stream, resolve_turn_memories, resolve_turn_passages, validate_model_name};
    use phibuddy_core::{
        agents::{Agent, Mode},
        evidence_passages::PassageSelection,
        progress::CancelFlag,
        store::{EventKind, Store},
    };
    use sha2::{Digest, Sha256};
    use std::sync::atomic::Ordering;

    #[test]
    fn local_suggestions_do_not_mutate_sessions_or_attach_notes() {
        let store = Store::in_memory().unwrap();
        store.remember_manual("Rust and SQLite local memories", Some("my notes")).unwrap();
        let before = store.list_memories(10).unwrap().len();
        let candidates = store.suggestion_candidates().unwrap();
        let suggestions = phibuddy_core::memory_discovery::rank_suggestions(
            "Rust memory", candidates, 8
        ).unwrap();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].provenance_status, "unverified_user_note");
        assert!(!suggestions[0].matched_terms.is_empty());
        assert_eq!(store.list_memories(10).unwrap().len(), before);
        assert!(store.list_sessions().unwrap().is_empty());
    }

    #[test]
    fn passage_inspector_displays_exact_quote_until_snapshot_deleted() {
        let store = Store::in_memory().unwrap();
        let session = store.create_session(Mode::Solo, Agent::Buddy).unwrap();
        let content = "first sentence\nsecond sentence\nthird sentence";
        let digest = format!("{:x}", Sha256::digest(content.as_bytes()));
        let saved = store.record_external_evidence(
            "https://www.rfc-editor.org/rfc/rfc9111.txt",
            "www.rfc-editor.org", "text/plain", &digest, content,
        ).unwrap();
        let selection = PassageSelection {
            evidence_id: saved.id.clone(), start_line: 2, end_line: 2,
        };
        let turn_id = Store::new_turn_id();
        let anchor = phibuddy_core::evidence_passages::PassageAnchor {
            selection, sha256: digest,
        };
        store.append_event(
            &session.id, &turn_id, EventKind::SourcePassageAttached,
            None, &serde_json::to_string(&anchor).unwrap(),
        ).unwrap();
        let result = resolve_turn_passages(&store, &session.id, &turn_id).unwrap();
        assert_eq!(result[0].preview.as_ref().unwrap().quote, "second sentence");
        assert!(!store.events(&session.id).unwrap()[0].content.contains("second sentence"));
        assert!(store.delete_external_evidence(&saved.id).unwrap());
        let deleted = resolve_turn_passages(&store, &session.id, &turn_id).unwrap();
        assert!(deleted[0].preview.is_none());
        assert!(resolve_turn_passages(&store, "unknown", &turn_id).is_err());
    }

    #[test]
    fn turn_source_inspector_respects_memory_deletion() {
        let store = Store::in_memory().unwrap();
        let session = store.create_session(Mode::Solo, Agent::Buddy).unwrap();
        let note = store.remember_manual("Private test note", Some("user note")).unwrap();
        let turn_id = Store::new_turn_id();
        store.append_event(
            &session.id, &turn_id, EventKind::MemoryAttached, None, &note.id,
        ).unwrap();
        let attached = resolve_turn_memories(&store, &session.id, &turn_id).unwrap();
        assert_eq!(attached.len(), 1);
        assert_eq!(attached[0].memory.as_ref().unwrap().content, "Private test note");
        assert_eq!(attached[0].id, note.id);
        assert!(store.forget_memory(&note.id).unwrap());
        let deleted = resolve_turn_memories(&store, &session.id, &turn_id).unwrap();
        assert_eq!(deleted.len(), 1);
        assert!(deleted[0].memory.is_none());
        assert!(!store.events(&session.id).unwrap().iter().any(
            |e| e.content.contains("Private test note")
        ));
        assert!(resolve_turn_memories(&store, "another-session", &turn_id).is_err());
    }

    #[test]
    fn streamed_ndjson_preserves_order_and_complete_text() {
        let sample = concat!(
            "{\"message\":{\"content\":\"Hello \"},\"done\":false}\n",
            "{\"message\":{\"content\":\"crew!\"},\"done\":false}\n",
            "{\"done\":true}\n",
        );
        let flag = CancelFlag::new(false);
        let mut pieces = Vec::new();
        let answer = read_stream(sample.as_bytes(), &flag, &mut |delta| {
            pieces.push(delta.to_string());
        })
        .unwrap();
        assert_eq!(answer, "Hello crew!");
        assert_eq!(pieces, vec!["Hello ", "crew!"]);
    }

    #[test]
    fn interrupted_or_malformed_ndjson_never_looks_complete() {
        let flag = CancelFlag::new(false);
        let incomplete = "{\"message\":{\"content\":\"partial\"},\"done\":false}\n";
        assert!(read_stream(incomplete.as_bytes(), &flag, &mut |_| {}).is_err());
        let malformed = "{this is not json}\n";
        assert!(read_stream(malformed.as_bytes(), &flag, &mut |_| {}).is_err());
    }

    #[test]
    fn cancellation_rejects_even_a_completed_stream() {
        let flag = CancelFlag::new(true);
        let valid = "{\"message\":{\"content\":\"ignored\"},\"done\":true}\n";
        assert!(read_stream(valid.as_bytes(), &flag, &mut |_| {}).is_err());
        flag.store(false, Ordering::Release);
        assert_eq!(
            read_stream(valid.as_bytes(), &flag, &mut |_| {}).unwrap(),
            "ignored"
        );
    }

    #[test]
    fn local_model_name_guard() {
        assert_eq!(validate_model_name("qwen3:4b".to_string()).unwrap(), "qwen3:4b");
        assert!(validate_model_name("kimi-k3:cloud".to_string()).is_err());
        assert!(validate_model_name("qwen3-vl:235b-cloud".to_string()).is_err());
        assert!(validate_model_name("".to_string()).is_err());
    }
}
