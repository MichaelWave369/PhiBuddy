//! PhiBuddy's local-first, model-agnostic orchestration library.
//! No shell commands or external tools are executed by this crate.

pub mod agents;
pub mod claim_support;
pub mod engine;
pub mod evaluation;
pub mod evidence_passages;
pub mod memory_discovery;
pub mod progress;
pub mod store;

pub type BuddyError = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, BuddyError>;

/// Provider-neutral message shape for chat completion services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}

/// A provider implements one completion request. The engine controls all routing.
pub trait ModelProvider {
    fn complete(&self, messages: &[Message]) -> Result<String>;

    /// Streaming is optional for providers; old CLI providers still work.
    fn complete_streaming(
        &self,
        messages: &[Message],
        cancel: &progress::CancelFlag,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String> {
        progress::check_cancel(cancel)?;
        let answer = self.complete(messages)?;
        progress::check_cancel(cancel)?;
        on_delta(&answer);
        Ok(answer)
    }
}
