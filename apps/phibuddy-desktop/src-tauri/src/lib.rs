//! Tauri commands are intentionally narrow and do not expose OS tools.
//! The CLI and desktop both call the same phibuddy-core engine and SQLite store.

use phibuddy_core::{
    agents::{Agent, Mode},
    engine::{CouncilDepth, Engine},
    progress::{check_cancel, CancelFlag, TurnProgress},
    store::{RecordedEvent, SavedMessage, Session, Store},
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
        let reader = BufReader::new(response);
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
        let turn = engine.run_observed(
            &session,
            &input,
            depth,
            &cancel,
            &mut |progress| { let _ = on_progress.send(progress); },
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
            check_ollama, send_prompt, cancel_turn
        ])
        .run(tauri::generate_context!())
        .expect("PhiBuddy desktop startup failed");
}

#[cfg(test)]
mod tests {
    use super::validate_model_name;

    #[test]
    fn local_model_name_guard() {
        assert_eq!(validate_model_name("qwen3:4b".to_string()).unwrap(), "qwen3:4b");
        assert!(validate_model_name("kimi-k3:cloud".to_string()).is_err());
        assert!(validate_model_name("qwen3-vl:235b-cloud".to_string()).is_err());
        assert!(validate_model_name("".to_string()).is_err());
    }
}
