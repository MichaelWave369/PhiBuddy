# Changelog

## 1.0.0-alpha.1 (draft source, not a stable release)

- Introduced Claim Support Lab: a user-triggered, read-only check of one explicitly pasted claim against only source passages attached to a specified recorded turn.
- Distinguishes exact text occurrence, shared terms only, no textual match, and source unavailable after deletion or fingerprint mismatch. No status is factual verification.
- Re-validates the current source snapshot SHA-256 and line range; does not copy evidence into new databases or create agent events.
- Adds a desktop inspector section for claim input, review results, source URL/line/hash references and the currently available original excerpt.
- Ensures claim review result is cleared on turn/session/source or input changes; no retained automated verdict or external network/model call.
- Adds regression tests for exact match, lexical false positives, missing/invalid turns, deleted sources and zero-source turns.
- No new model/tool permissions, data ingestion, or source fetch capabilities.


## 0.9.0-alpha.1 (draft, not released)

- Added exact line-range passage previews from saved external evidence snapshots, verified against the stored SHA-256 before being shown or used.
- Added separate explicit passage selection (up to 3 per turn, up to 1600 characters each, 6000 total characters including context wrapper), never automatic retrieval or carryover.
- Added a source attachment audit event containing only the snapshot ID, inclusive line range and fingerprint, not the quote text.
- Added a turn source inspector which reconstructs quotes from the current saved snapshot and labels missing/changed/deleted source material as unavailable.
- Updated orchestration so identical selected source passages reach all Council calls, while remaining absent from completed conversation history unless the model repeats them.
- Distinguished exact source quotes, AI inferences, and unverified claims; independent factual verification still does not exist.
- Added source range, digest, deleted-source, opt-in, reuse and inspector regression tests. No new agent execution privileges.


## 0.8.0-alpha.1 (draft source, not released)

- Added a user-triggered external text evidence registry for two allowlisted URL families: GitHub Raw and RFC Editor plain-text RFCs.
- Each captured source stores a local UTF-8 text snapshot, canonical HTTPS URL, host, content type, fetch timestamp, and SHA-256 fingerprint in a separate SQLite table.
- Hardened the fetch path: HTTPS only, fixed domain/path allowlist, no embedded credentials/query/fragment, no custom ports, no redirects or proxy use, 8-second connect and 25-second overall timeouts, 256 KiB response cap, no HTML/binary processing.
- Introduced a desktop Evidence inspector tab with explicit retrieve, snapshot inspection, and deletion confirmation. Agent-generated text cannot trigger fetching.
- Distinguishes user-saved notes, retrieved-not-fact-checked sources, and unverified model claims. No source is represented as independently fact-checked.
- Added deterministic source-URL policy tests, core digest-validation tests, and SQLite snapshot persistence/deletion tests.
- No automatic source citations, claim verification, prompt injection, agent tools, shell, or autonomous browsing.


## 0.7.0-alpha.1 (draft source, not released)

- Added on-demand, deterministic local lexical relevance suggestions from saved user notes, with exact matched terms and integer text-match scores.
- Added creation-age review warnings at 90+ days or when a saved date cannot be parsed. Age alone does not establish whether a note is false or outdated.
- Added a desktop **Find related notes** action. Results are previews only; every attachment still requires a separate user selection for the next turn.
- Distinguished unverified user notes from **zero** independently verified external references in the source inspector; no web retrieval or citation verification is implied.
- Added unit tests for deterministic ranking, stopwords, input bounds, recency-review markers, and no-memory-attachment side effects.
- No new model prompts, autonomous tools, external APIs, or SQLite schema changes.


## 0.6.0-alpha.1 (draft source, not released)

- Added a desktop Memory Workshop using the explicit v0.5 local SQLite memory ledger.
- Introduced per-turn, user-selected source notes with maximum 5 notes / 6,000 context characters, no automatic carryover to later prompts.
- Added ID-only `memory_attached` events and per-turn source inspection that reflects deletion of the active memory record.
- Added deletion confirmation and regression tests for selected context, missing and duplicate IDs, next-turn non-reuse and deleted-note attribution.
- Preserved all existing CLI and agent tool-denial boundaries; no cloud sync or silent auto-ingestion.


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
