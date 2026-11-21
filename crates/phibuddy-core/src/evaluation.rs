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
