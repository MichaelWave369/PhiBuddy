//! Read-only lexical checks against passages explicitly attached to a turn.
//! No LLM judging, no source fetching, no truth/entailment claims.
use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    evidence_passages::{preview_passage, PassageAnchor},
    store::{EventKind, Store},
    Result,
};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextMatchKind {
    ExactTextFound,
    TextOverlapOnly,
    NoTextMatch,
    SourceUnavailable,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClaimFinding {
    pub anchor: PassageAnchor,
    pub source_url: Option<String>,
    pub text_match: TextMatchKind,
    pub shared_terms: Vec<String>,
    pub original_quote: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClaimReview {
    pub claim: String,
    pub turn_id: String,
    pub findings: Vec<ClaimFinding>,
    pub status: &'static str,
}

const STOP: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "can", "do", "for", "from", "has", "have",
    "in", "is", "it", "of", "on", "or", "our", "that", "the", "their", "there", "these", "this",
    "to", "was", "were", "which", "with", "you", "your",
];

fn words(input: &str) -> BTreeSet<String> {
    input
        .split(|ch: char| !ch.is_alphanumeric())
        .filter_map(|part| {
            let term = part.to_lowercase();
            (term.chars().count() > 2 && !STOP.contains(&term.as_str())).then_some(term)
        })
        .collect()
}

/// Reviews one human-entered statement against **only** the source passages
/// attached to this recorded turn. Text matching cannot establish truth.
pub fn review_claim(
    store: &Store,
    session_id: &str,
    turn_id: &str,
    claim: &str,
) -> Result<ClaimReview> {
    let claim = claim.trim();
    if !(12..=600).contains(&claim.chars().count()) {
        return Err("claim must contain 12 to 600 characters".into());
    }
    if claim.chars().any(|ch| ch.is_control() && ch != '\n') {
        return Err("claim contains unsupported control characters".into());
    }
    if store.get_session(session_id)?.is_none() {
        return Err("unknown session".into());
    }
    let events = store.events(session_id)?;
    if !events
        .iter()
        .any(|e| e.turn_id == turn_id && e.kind == EventKind::TurnStarted)
    {
        return Err("turn does not exist in this session".into());
    }

    let anchors = events
        .iter()
        .filter(|e| e.turn_id == turn_id && e.kind == EventKind::SourcePassageAttached)
        .map(|e| serde_json::from_str::<PassageAnchor>(&e.content))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if anchors.len() > 12 {
        return Err("too many source anchors in this turn".into());
    }

    let claim_words = words(claim);
    let findings = anchors
        .into_iter()
        .map(|anchor| {
            // Deleted, corrupted or hash-mismatched snapshots may not be used
            // to resurrect source text or assert a current match.
            let preview = preview_passage(store, &anchor.selection)
                .ok()
                .filter(|p| p.sha256 == anchor.sha256);
            let Some(preview) = preview else {
                return ClaimFinding {
                    anchor,
                    source_url: None,
                    text_match: TextMatchKind::SourceUnavailable,
                    shared_terms: Vec::new(),
                    original_quote: None,
                };
            };
            let quote_words = words(&preview.quote);
            let shared_terms = claim_words
                .intersection(&quote_words)
                .cloned()
                .collect::<Vec<_>>();
            let text_match = if preview.quote.contains(claim) {
                TextMatchKind::ExactTextFound
            } else if shared_terms.len() >= 2 {
                TextMatchKind::TextOverlapOnly
            } else {
                TextMatchKind::NoTextMatch
            };
            ClaimFinding {
                anchor,
                source_url: Some(preview.source_url),
                text_match,
                shared_terms,
                original_quote: Some(preview.quote),
            }
        })
        .collect();
    Ok(ClaimReview {
        claim: claim.to_string(),
        turn_id: turn_id.to_string(),
        findings,
        status: "textual_comparison_only_not_fact_checked",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agents::{Agent, Mode},
        evidence_passages::PassageSelection,
    };
    use sha2::{Digest, Sha256};

    fn fixture() -> (Store, String, String, String) {
        let store = Store::in_memory().unwrap();
        let session = store.create_session(Mode::Solo, Agent::Buddy).unwrap();
        let turn = Store::new_turn_id();
        store
            .append_event(&session.id, &turn, EventKind::TurnStarted, None, "Question")
            .unwrap();
        let content = "The Rust service stores session events in SQLite.\nA network model can produce incorrect answers.\nThis is not truth.";
        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let source = store
            .record_external_evidence(
                "https://raw.githubusercontent.com/example/example/main/doc.md",
                "raw.githubusercontent.com",
                "text/plain",
                &hash,
                content,
            )
            .unwrap();
        let selection = PassageSelection {
            evidence_id: source.id.clone(),
            start_line: 1,
            end_line: 2,
        };
        let anchor = PassageAnchor {
            selection,
            sha256: hash,
        };
        store
            .append_event(
                &session.id,
                &turn,
                EventKind::SourcePassageAttached,
                None,
                &serde_json::to_string(&anchor).unwrap(),
            )
            .unwrap();
        (store, session.id, turn, source.id)
    }

    #[test]
    fn exact_quote_is_detected_but_not_labelled_verified() {
        let (store, session, turn, _) = fixture();
        let review = review_claim(
            &store,
            &session,
            &turn,
            "The Rust service stores session events in SQLite.",
        )
        .unwrap();
        assert_eq!(review.findings.len(), 1);
        assert_eq!(review.findings[0].text_match, TextMatchKind::ExactTextFound);
        assert_eq!(review.status, "textual_comparison_only_not_fact_checked");
        assert!(review.findings[0]
            .original_quote
            .as_ref()
            .unwrap()
            .contains("SQLite"));
    }

    #[test]
    fn word_overlap_is_only_a_hint_not_an_entailment_verdict() {
        let (store, session, turn, _) = fixture();
        let review =
            review_claim(&store, &session, &turn, "SQLite events will never be lost").unwrap();
        assert_eq!(
            review.findings[0].text_match,
            TextMatchKind::TextOverlapOnly
        );
        assert_eq!(review.findings[0].shared_terms, vec!["events", "sqlite"]);
        let unrelated = review_claim(
            &store,
            &session,
            &turn,
            "Bananas are growing outside the station",
        )
        .unwrap();
        assert_eq!(unrelated.findings[0].text_match, TextMatchKind::NoTextMatch);
    }

    #[test]
    fn unavailable_sources_never_get_treated_as_matched() {
        let (store, session, turn, id) = fixture();
        assert!(store.delete_external_evidence(&id).unwrap());
        let result = review_claim(
            &store,
            &session,
            &turn,
            "The Rust service stores session events in SQLite.",
        )
        .unwrap();
        assert_eq!(
            result.findings[0].text_match,
            TextMatchKind::SourceUnavailable
        );
        assert!(result.findings[0].original_quote.is_none());
        assert!(result.findings[0].source_url.is_none());
    }

    #[test]
    fn missing_attachments_and_invalid_sessions_cannot_borrow_other_evidence() {
        let (store, session, turn, _) = fixture();
        assert!(review_claim(
            &store,
            "fake-session",
            &turn,
            "The Rust service stores session events in SQLite."
        )
        .is_err());
        assert!(review_claim(
            &store,
            &session,
            "fake-turn",
            "The Rust service stores session events in SQLite."
        )
        .is_err());
        assert!(review_claim(&store, &session, &turn, "short").is_err());
        let next = Store::new_turn_id();
        store
            .append_event(&session, &next, EventKind::TurnStarted, None, "Another")
            .unwrap();
        let unrelated = review_claim(
            &store,
            &session,
            &next,
            "The Rust service stores session events in SQLite.",
        )
        .unwrap();
        assert!(unrelated.findings.is_empty());
        assert_eq!(store.events(&session).unwrap().len(), 3);
    }
}
