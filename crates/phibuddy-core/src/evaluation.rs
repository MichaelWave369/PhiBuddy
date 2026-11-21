//! Local Solo-versus-Council evaluation with transparent costs, not self-assigned accuracy.
use crate::{
    agents::{Agent, Mode},
    engine::{CouncilDepth, Engine},
    ModelProvider, Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Instant};

#[derive(Clone, Debug, Deserialize)]
pub struct EvalSuite {
    pub cases: Vec<EvalCase>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EvalCase {
    pub id: String,
    pub prompt: String,
    /// Optional human-supplied reference; never passed to the model.
    #[serde(default)]
    pub reference: Option<String>,
}

impl EvalSuite {
    pub fn validate(&self) -> Result<()> {
        if self.cases.is_empty() || self.cases.len() > 50 {
            return Err("evaluation suite must contain 1 to 50 cases".into());
        }
        let mut ids = HashSet::new();
        for case in &self.cases {
            if case.id.is_empty()
                || case.id.len() > 64
                || !case
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err(format!("invalid evaluation case id: {}", case.id).into());
            }
            if !ids.insert(&case.id) {
                return Err(format!("duplicate evaluation case id: {}", case.id).into());
            }
            if case.prompt.trim().is_empty() || case.prompt.chars().count() > 20_000 {
                return Err(format!("invalid prompt in evaluation case: {}", case.id).into());
            }
            if case
                .reference
                .as_ref()
                .is_some_and(|r| r.chars().count() > 20_000)
            {
                return Err(format!("reference too long in evaluation case: {}", case.id).into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalOrder {
    SoloFirst,
    CouncilFirst,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvalAnswer {
    pub answer: String,
    pub elapsed_ms: u128,
    /// The number of sequential model calls the successful workflow requires.
    pub model_requests: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvalRecord {
    pub case_id: String,
    pub prompt: String,
    pub reference: Option<String>,
    pub order: EvalOrder,
    pub council_depth: String,
    pub solo: EvalAnswer,
    pub council: EvalAnswer,
}

/// Each variant receives its own new session and identical prompt with empty history.
/// Ordering alternates across cases to reduce one systematic warm-cache bias.
pub fn evaluate_case<P: ModelProvider>(
    engine: &mut Engine<P>,
    case: &EvalCase,
    depth: CouncilDepth,
    order: EvalOrder,
) -> Result<EvalRecord> {
    let run = |engine: &mut Engine<P>, mode: Mode| -> Result<EvalAnswer> {
        let session = engine.store().create_session(mode, Agent::Buddy)?;
        let started = Instant::now();
        let turn = engine.run_with_depth(&session, &case.prompt, depth)?;
        Ok(EvalAnswer {
            answer: turn.answer,
            elapsed_ms: started.elapsed().as_millis(),
            model_requests: match mode {
                Mode::Solo => 1,
                Mode::Council => {
                    if depth == CouncilDepth::Debate {
                        7
                    } else {
                        4
                    }
                }
                Mode::Workshop => unreachable!("evaluation never uses Workshop"),
            },
        })
    };
    let (solo, council) = match order {
        EvalOrder::SoloFirst => {
            let solo = run(engine, Mode::Solo)?;
            let council = run(engine, Mode::Council)?;
            (solo, council)
        }
        EvalOrder::CouncilFirst => {
            let council = run(engine, Mode::Council)?;
            let solo = run(engine, Mode::Solo)?;
            (solo, council)
        }
    };
    Ok(EvalRecord {
        case_id: case.id.clone(),
        prompt: case.prompt.clone(),
        reference: case.reference.clone(),
        order,
        council_depth: depth.to_string(),
        solo,
        council,
    })
}

/// Ratings are provided by a human reviewer. They are not generated or
/// independently verified by PhiBuddy.
#[derive(Clone, Debug, Deserialize)]
pub struct RatingSheet {
    pub ratings: Vec<HumanRating>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HumanRating {
    pub case_id: String,
    pub solo_score: u8,
    pub council_score: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct RatingSummary {
    pub rated_cases: usize,
    pub score_scale: &'static str,
    pub evidence_type: &'static str,
    pub solo_mean: f64,
    pub council_mean: f64,
    pub solo_wins: usize,
    pub council_wins: usize,
    pub ties: usize,
}

/// Aggregates an exact set of case IDs. Missing, extra, repeated, or out-of-range
/// ratings are errors rather than silently biasing a comparison.
pub fn summarize_ratings(report_ids: &[String], sheet: &RatingSheet) -> Result<RatingSummary> {
    if report_ids.is_empty() || report_ids.len() > 50 {
        return Err("report must contain 1 to 50 cases".into());
    }
    let expected: HashSet<&str> = report_ids.iter().map(String::as_str).collect();
    if expected.len() != report_ids.len() || sheet.ratings.len() != report_ids.len() {
        return Err("report and ratings must contain the same unique case IDs".into());
    }
    let mut seen = HashSet::new();
    let mut solo_total = 0usize;
    let mut council_total = 0usize;
    let (mut solo_wins, mut council_wins, mut ties) = (0, 0, 0);
    for rating in &sheet.ratings {
        if !expected.contains(rating.case_id.as_str()) || !seen.insert(rating.case_id.as_str()) {
            return Err(format!("unknown or repeated rated case ID: {}", rating.case_id).into());
        }
        if rating.solo_score > 5 || rating.council_score > 5 {
            return Err(format!("rating for {} must be in the range 0 to 5", rating.case_id).into());
        }
        solo_total += rating.solo_score as usize;
        council_total += rating.council_score as usize;
        match rating.council_score.cmp(&rating.solo_score) {
            std::cmp::Ordering::Greater => council_wins += 1,
            std::cmp::Ordering::Less => solo_wins += 1,
            std::cmp::Ordering::Equal => ties += 1,
        }
    }
    Ok(RatingSummary {
        rated_cases: report_ids.len(),
        score_scale: "0..5",
        evidence_type: "human_supplied_unverified",
        solo_mean: solo_total as f64 / report_ids.len() as f64,
        council_mean: council_total as f64 / report_ids.len() as f64,
        solo_wins,
        council_wins,
        ties,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{store::Store, Message};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Mock {
        calls: AtomicUsize,
    }

    impl ModelProvider for Mock {
        fn complete(&self, messages: &[Message]) -> Result<String> {
            // Distinct roles do not get the other variant's completed chat history.
            assert_eq!(messages.len(), 2);
            let n = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            Ok(format!("response {n}"))
        }
    }

    fn suite() -> EvalSuite {
        EvalSuite {
            cases: vec![EvalCase {
                id: "case_01".into(),
                prompt: "Give a short design decision".into(),
                reference: Some("Human reference only".into()),
            }],
        }
    }

    #[test]
    fn validates_case_ids_and_input_bounds() {
        assert!(suite().validate().is_ok());
        let mut invalid = suite();
        invalid.cases.push(invalid.cases[0].clone());
        assert!(invalid.validate().is_err());
        invalid.cases[1].id = "../bad".into();
        assert!(invalid.validate().is_err());
        invalid.cases[1].id = "case_02".into();
        invalid.cases[1].prompt = " ".into();
        assert!(invalid.validate().is_err());
        assert!(EvalSuite { cases: Vec::new() }.validate().is_err());
    }

    #[test]
    fn quick_comparison_is_isolated_and_reports_call_count() {
        let provider = Mock {
            calls: AtomicUsize::new(0),
        };
        let mut engine = Engine::new(provider, Store::in_memory().unwrap());
        let record = evaluate_case(
            &mut engine,
            &suite().cases[0],
            CouncilDepth::Quick,
            EvalOrder::SoloFirst,
        )
        .unwrap();
        assert_eq!(record.solo.model_requests, 1);
        assert_eq!(record.council.model_requests, 4);
        assert_eq!(engine.provider().calls.load(Ordering::Relaxed), 5);
        assert_eq!(record.reference.as_deref(), Some("Human reference only"));
        assert_eq!(engine.store().list_sessions().unwrap().len(), 2);
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("solo_first"));
    }

    #[test]
    fn human_rating_summary_requires_complete_valid_case_set() {
        let cases = vec!["alpha".to_string(), "beta".to_string()];
        let sheet = RatingSheet { ratings: vec![
            HumanRating { case_id: "alpha".into(), solo_score: 3, council_score: 4 },
            HumanRating { case_id: "beta".into(), solo_score: 5, council_score: 2 },
        ] };
        let summary = summarize_ratings(&cases, &sheet).unwrap();
        assert_eq!(summary.rated_cases, 2);
        assert_eq!(summary.solo_mean, 4.0);
        assert_eq!(summary.council_mean, 3.0);
        assert_eq!(summary.solo_wins, 1);
        assert_eq!(summary.council_wins, 1);
        let missing = RatingSheet { ratings: vec![sheet.ratings[0].clone()] };
        assert!(summarize_ratings(&cases, &missing).is_err());
        let duplicate = RatingSheet { ratings: vec![
            sheet.ratings[0].clone(),
            sheet.ratings[0].clone(),
        ] };
        assert!(summarize_ratings(&cases, &duplicate).is_err());
        let invalid = RatingSheet { ratings: vec![
            HumanRating { case_id: "alpha".into(), solo_score: 6, council_score: 4 },
            sheet.ratings[1].clone(),
        ] };
        assert!(summarize_ratings(&cases, &invalid).is_err());
    }

    #[test]
    fn debate_comparison_can_run_council_first() {
        let provider = Mock {
            calls: AtomicUsize::new(0),
        };
        let mut engine = Engine::new(provider, Store::in_memory().unwrap());
        let record = evaluate_case(
            &mut engine,
            &suite().cases[0],
            CouncilDepth::Debate,
            EvalOrder::CouncilFirst,
        )
        .unwrap();
        assert_eq!(record.council.model_requests, 7);
        assert_eq!(record.solo.model_requests, 1);
        assert_eq!(engine.provider().calls.load(Ordering::Relaxed), 8);
    }
}
