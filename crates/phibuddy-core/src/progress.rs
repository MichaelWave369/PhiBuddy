//! Ephemeral progress notifications and cooperative cancellation.
use crate::Result;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

pub type CancelFlag = AtomicBool;

#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "turn cancelled by user")
    }
}
impl std::error::Error for Cancelled {}

pub fn check_cancel(cancel: &CancelFlag) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(Box::new(Cancelled))
    } else {
        Ok(())
    }
}

/// Messages sent to the desktop UI, not persisted on every token.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TurnProgress {
    TurnStarted {
        turn_id: String,
    },
    StageStarted {
        agent: String,
        phase: String,
    },
    Delta {
        agent: String,
        phase: String,
        text: String,
    },
    StageFinished {
        agent: String,
        phase: String,
    },
}
