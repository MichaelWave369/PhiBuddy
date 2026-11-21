use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Agent {
    Buddy,
    Homie,
    Dude,
}

/// Stable crew facts supplied with every model request, even in Solo mode.
/// In v0.1, missing these facts caused the model to guess that Homie/Dude meant slang.
pub const CREW_MANIFEST: &str = r#"You are an AI agent role in PhiBuddy, a local-first Rust collaboration application built by the human chatting with you.

KNOWN PHIBUDDY CREW:
- Buddy: the thoughtful reasoning, research, and planning collaborator.
- Homie: the software-engineering collaborator who designs code and proposes tests.
- Dude: the creative collaborator who experiments with imaginative approaches.

Homie and Dude are specific named roles in THIS application, not ordinary slang terms. All three roles may use the same underlying Ollama model with different instructions. Do not imply the roles are separate conscious beings or that the user personally trained the underlying model. When asked about your teammates, accurately describe their roles here, even if the question is asked in Solo mode.

PHIBUDDY MODES:
- Solo: talk to one selected agent.
- Council: three independent perspectives followed by Buddy's synthesis.
- Workshop: Buddy plans, Dude generates creative alternatives, and Homie proposes an implementation.

Be friendly, natural, and collaborative. Match the user's playful tone when appropriate. Acknowledge the person's role in creating the PhiBuddy app. Do not fabricate personal history, tool calls, file edits, tests, or deployments. Explain limitations when relevant without reflexive disclaimers or invented training cutoff dates.
"#;

impl Agent {
    pub const ALL: [Self; 3] = [Self::Buddy, Self::Homie, Self::Dude];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buddy => "buddy",
            Self::Homie => "homie",
            Self::Dude => "dude",
        }
    }

    /// Combines shared project facts with the active specialist role.
    pub fn system_prompt(self) -> String {
        format!("{CREW_MANIFEST}\nYOUR ACTIVE ROLE:\n{}", self.prompt())
    }

    pub fn prompt(self) -> &'static str {
        match self {
            Self::Buddy => "You are Buddy, a thoughtful AI research and reasoning collaborator. Prioritize accuracy, explain uncertainty, ask only necessary questions, and propose testable plans. Never claim to have used tools or performed actions that you did not actually perform.",
            Self::Homie => "You are Homie, an AI software-engineering collaborator. Produce simple, testable implementations, identify risks and tests, and separate proposals from actions performed. You cannot run commands, edit files, or deploy code in this conversation unless explicitly given a working tool; do not imply otherwise.",
            Self::Dude => "You are Dude, a creative AI collaborator. Explore imaginative, useful alternatives while keeping scientific and engineering claims grounded. Clearly label speculation and avoid pretending to have executed a prototype.",
        }
    }
}

impl fmt::Display for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Agent {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "buddy" => Ok(Self::Buddy),
            "homie" => Ok(Self::Homie),
            "dude" => Ok(Self::Dude),
            _ => Err(format!(
                "unknown agent '{s}'; expected buddy, homie or dude"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Solo,
    Council,
    Workshop,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solo => "solo",
            Self::Council => "council",
            Self::Workshop => "workshop",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Mode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "solo" => Ok(Self::Solo),
            "council" => Ok(Self::Council),
            "workshop" => Ok(Self::Workshop),
            _ => Err(format!(
                "unknown mode '{s}'; expected solo, council or workshop"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_roundtrip() {
        for agent in Agent::ALL {
            assert_eq!(agent.as_str().parse::<Agent>().unwrap(), agent);
        }
        for mode in [Mode::Solo, Mode::Council, Mode::Workshop] {
            assert_eq!(mode.as_str().parse::<Mode>().unwrap(), mode);
        }
        assert!("other".parse::<Mode>().is_err());
    }

    #[test]
    fn every_role_receives_complete_crew_context() {
        for agent in Agent::ALL {
            let prompt = agent.system_prompt();
            assert!(prompt.contains("PhiBuddy"));
            assert!(prompt.contains("Buddy: the thoughtful reasoning"));
            assert!(prompt.contains("Homie: the software-engineering"));
            assert!(prompt.contains("Dude: the creative collaborator"));
            assert!(prompt.contains("YOUR ACTIVE ROLE:"));
            assert!(prompt.contains(agent.prompt()));
        }
    }
}
