# Changelog

## 0.2.0 (source candidate; awaiting Cargo validation)

- Added append-only-by-application SQLite deliberation event log with schema version, turn ID, phase, role and source text.
- Added Quick (4-call) and Debate (7-call) Council depths; Debate includes independent drafts, three cross-critiques, and synthesis.
- Exposed actual contributions via `--show-contributions` and trace via `events <session> --jsonl`.
- Added multi-line `:paste` input for longer architecture prompts.
- Persist partial drafts and a failure event when an Ollama call fails; incomplete turns do not enter normal conversation history.
- Atomic commit of user, contributions, answer and terminal event for successful turns.
- Added tests covering Council independence, debate stages, failures, event sequence and v0.1 SQLite compatibility.
- Updated ADR, security limitations and Windows+Linux CI proposal.

This is not a proof that Rust compilation, local model performance or security audits have passed. Use the included test commands and report results.
