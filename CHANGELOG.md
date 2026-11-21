# Changelog

## 0.5.0-alpha.1 (draft source; not released)

- Added Council Lab: isolated Solo/Quick/Debate comparisons using the same local Ollama model, with actual answers, timing, model-request counts and alternating order.
- Added a validator for up to 50 JSON evaluation cases, incremental JSONL reports, and an optional human-rated 0–5 summary with strict case-ID matching.
- Added user-managed, source-labelled memory notes in an additive SQLite table, with list, literal search, and active-record deletion.
- Added deterministic mock-evaluation, rating-validation, SQLite migration, memory persistence/deletion, and input-boundary regression tests.
- Made no changes to Tauri permission grants, agent execution privileges, hosted inference, or automated memory ingestion.
- Local Ollama smoke testing, security review, and a statistically meaningful model-quality study remain outstanding.

This is an **alpha candidate** for review, not a published installer or verified benchmark.


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
