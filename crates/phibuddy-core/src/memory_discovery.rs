//! Local, deterministic lexical note suggestions. No model inference, embeddings,
//! external verification, or automatic prompt attachment occurs here.
use std::collections::{BTreeSet, HashSet};

use serde::Serialize;

use crate::{
    store::{MemoryCandidate, MemoryRecord},
    Result,
};

/// A score is a *text match heuristic*, never an estimate of factual correctness.
#[derive(Clone, Debug, Serialize)]
pub struct MemorySuggestion {
    pub note: MemoryRecord,
    pub relevance_score: u32,
    pub matched_terms: Vec<String>,
    pub age_days: Option<i64>,
    /// Notes 90+ days old (or with an unparseable timestamp) deserve review,
    /// but should never be automatically discarded or declared false.
    pub review_recommended: bool,
    pub provenance_status: &'static str,
}

const STOP_WORDS: &[&str] = &[
    "about", "after", "again", "also", "and", "are", "before", "but", "can",
    "does", "for", "from", "have", "how", "into", "its", "not", "our",
    "the", "their", "them", "this", "that", "these", "those", "use", "using",
    "was", "were", "what", "when", "where", "which", "who", "why", "with",
    "would", "your", "you", "me", "my", "we", "a", "an", "to", "of", "on",
    "in", "is", "it", "be", "or", "as", "at", "by", "do", "if",
];

fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter_map(|token| {
            let token = token.to_lowercase();
            (token.chars().count() >= 2 && !STOP_WORDS.contains(&token.as_str()))
                .then_some(token)
        })
        .collect()
}

/// Rank only existing local user notes. No session or message state is mutated.
pub fn rank_suggestions(
    query: &str,
    candidates: Vec<MemoryCandidate>,
    limit: usize,
) -> Result<Vec<MemorySuggestion>> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 2_000 {
        return Err("suggestion query must be 1 to 2,000 characters".into());
    }
    let terms = tokens(query);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let query_lower = query.to_lowercase();
    let mut ranked = Vec::new();
    for candidate in candidates {
        // Count unique matching tokens; repeated words do not inflate scores.
        let body_tokens: HashSet<String> = tokens(&candidate.note.content).into_iter().collect();
        let source_tokens: HashSet<String> = candidate
            .note
            .source_ref
            .as_deref()
            .map(tokens)
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut score = 0;
        let mut matched_terms = Vec::new();
        for term in &terms {
            if body_tokens.contains(term) {
                score += 3;
                matched_terms.push(term.clone());
            } else if source_tokens.contains(term) {
                score += 1;
                matched_terms.push(term.clone());
            }
        }
        if score == 0 {
            continue;
        }
        if query_lower.chars().count() >= 6
            && candidate.note.content.to_lowercase().contains(&query_lower)
        {
            score += 6;
        }
        let review_recommended = candidate.age_days.is_none()
            || candidate.age_days.is_some_and(|age| age >= 90);
        ranked.push(MemorySuggestion {
            note: candidate.note,
            relevance_score: score,
            matched_terms,
            age_days: candidate.age_days,
            review_recommended,
            provenance_status: "unverified_user_note",
        });
    }
    ranked.sort_by(|a, b| {
        b.relevance_score
            .cmp(&a.relevance_score)
            .then_with(|| a.review_recommended.cmp(&b.review_recommended))
            .then_with(|| a.age_days.unwrap_or(i64::MAX).cmp(&b.age_days.unwrap_or(i64::MAX)))
            .then_with(|| a.note.id.cmp(&b.note.id))
    });
    ranked.truncate(limit.min(12));
    Ok(ranked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, body: &str, source: Option<&str>, age: Option<i64>) -> MemoryCandidate {
        MemoryCandidate {
            note: MemoryRecord {
                id: id.into(),
                content: body.into(),
                source_kind: "user_explicit".into(),
                source_ref: source.map(str::to_string),
                created_at: "2026-11-01 00:00:00".into(),
            },
            age_days: age,
        }
    }

    #[test]
    fn relevance_is_deterministic_and_exposes_match_reasons() {
        let notes = vec![
            candidate("source", "Unrelated text", Some("Rust storage"), Some(2)),
            candidate("body", "Rust storage with SQLite storage", None, Some(9)),
            candidate("other", "Painting flowers", None, Some(1)),
        ];
        let result = rank_suggestions("Rust storage", notes, 10).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].note.id, "body");
        assert!(result[0].relevance_score > result[1].relevance_score);
        assert_eq!(result[0].matched_terms, vec!["rust", "storage"]);
        assert_eq!(result[1].provenance_status, "unverified_user_note");
    }

    #[test]
    fn old_and_unknown_timestamps_flag_review_not_inaccuracy() {
        let notes = vec![
            candidate("recent", "local networking", None, Some(12)),
            candidate("old", "local networking", None, Some(120)),
            candidate("unknown", "local networking", None, None),
        ];
        let found = rank_suggestions("networking", notes, 10).unwrap();
        assert_eq!(found[0].note.id, "recent");
        assert!(!found[0].review_recommended);
        assert!(found[1].review_recommended);
        assert!(found[2].review_recommended);
    }

    #[test]
    fn stopwords_unmatched_and_limits_are_safe() {
        let notes = vec![candidate("one", "network security", None, Some(2))];
        assert!(rank_suggestions("", notes.clone(), 5).is_err());
        assert!(rank_suggestions(&"x".repeat(2001), notes.clone(), 5).is_err());
        assert!(rank_suggestions("the and or", notes.clone(), 5).unwrap().is_empty());
        assert!(rank_suggestions("painting", notes.clone(), 5).unwrap().is_empty());
        assert!(rank_suggestions("network", notes, 0).unwrap().is_empty());
    }
}
