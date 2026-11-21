# PhiBuddy v0.7: Memory Discovery (source alpha)

## User goal

Find previously saved notes relevant to a drafted message without giving an LLM unrestricted memory access. **Discovery, review, selection, and prompt attachment are separate operations.**

## Workflow

1. Write a draft message in the desktop chat composer. Do not send it yet.
2. Click **Find related notes** (this invokes one read-only, local Tauri command, `suggest_memories`). Results appear in the Memory inspector.
3. Read each full note and its human-entered source label, review-age warning, integer text-match score, and exact matched query words.
4. Click **Attach next turn** only for notes you choose. You can select at most five; the existing Rust engine enforces the 6,000-character serialized context budget at send time.
5. Send the message. This is the only operation that exposes selected notes to the configured Ollama model. The selection clears immediately, including after an interrupted or failed send.
6. Inspect that turn. Its attached note IDs are resolved from the active memory ledger; removed note content is shown as unavailable.

If you edit the draft, switch sessions, save or delete a note, or submit a message, the suggestion preview is cleared or updated to avoid misleading stale results. The search itself never selects any note.

## Deterministic lexical ranking, not semantic retrieval

- Up to **1,000** most recently saved local notes are examined. This bound is a performance cap and can omit relevant older records outside that pool.
- A query of 1–2,000 characters is split into unique lowercase alphanumeric tokens; common English stopwords and one-character terms are ignored.
- Each unique matched term in note body contributes **3 points**. A match only in the human-entered source label contributes **1 point**. An exact contiguous phrase in the body (query length at least six characters) adds **6 points**.
- Results require at least one matching token, rank by score, then prefer notes with less age-review concern, then newer saved dates, then stable ID order. Maximum **8** suggestions in the desktop.
- Scores are raw match heuristics, **not probabilities or assessments of accuracy**. They cannot resolve contradictions, synonyms, stale facts, misspellings, or malicious text.
- Non-English scripts have limited tokenization support; no language model, embedding service, FTS index, network lookup, or semantic inference is used.

## Review-age detection and evidence boundary

SQLite calculates elapsed days between now and `memory_notes.created_at`. Notes **90 days or older** trigger **Review recommended**. Notes whose dates cannot be parsed also get a warning. Dates younger than 90 days do not imply truth; older dates do not imply falsehood. This is a saved-at age indicator, not a claim that a source has changed.

All current records are `source_kind=user_explicit`: the **user typed the note and the optional source label**. A source label resembling a website URL is not verified by PhiBuddy. The evidence inspector explicitly displays:

- **Unverified user notes**, showing ID and current source label/body from SQLite.
- **Independently verified external sources: none**, because this version has **no** external verification connector or ingestion pipeline.

A model may generate `[memory:ID]` in an answer, but such text is not validated by the app. The inspector trusts only the stored ID event and current ledger row.

## Security / operational limits

The new `suggest_memories` Tauri command is **read-only** on the local SQLite database. It takes the draft text only when the user clicks and returns ranked notes to the local UI, never to Ollama. It grants no shell, filesystem browser, GitHub, general network, MCP, or cloud permissions. The existing selected-note execution route is unchanged.

Memory and conversation history remain **unencrypted**. Note content can be repeated in generated answers after attachment, and deleting the original note does not remove copies from those outputs or backups. Localhost Ollama does not prove the model remains offline: verify the selected model before sharing sensitive data.

## Automated acceptance

- Deterministic matching ranks note-body matches higher than source-label-only matches and explains the term overlap.
- Results with no lexical overlap produce an empty set; invalid queries fail.
- 90+ day and unparseable saved dates trigger review, without labeling notes inaccurate.
- The read-only candidate/suggestion path does not create sessions, record events, or prompt models.
- Existing v0.6 strict opt-in attachment, deletion, cancellation, and event tests remain green.
- React/TypeScript, Ubuntu + Windows Rust and Windows Tauri host/tests pass at the exact PR head.

## Manual Windows/Ollama smoke tests

1. Back up the SQLite database, start the desktop with an actually local Ollama model.
2. Save three notes: one with keywords in its body, one with keywords only in its user-supplied source label, and one unrelated note.
3. Draft a question including those words and click **Find related notes**. Confirm body match ranks above source-only match and unrelated note is absent.
4. Confirm suggestions are not selected by default. Click **Attach next turn** on one note only, send, and inspect its ID. Repeat without selecting and verify zero new memory attachments.
5. Use a harmless old test note (created timestamp set in a disposable DB) to verify the 90-day review warning.
6. Confirm the inspector states that external verification is unavailable, even for URL-looking labels.
7. Delete a previously attached note, reopen its turn, and confirm source body is no longer recoverable through the current ledger.
8. Verify no regressions in Council streaming, cancellation, and prior v0.6 memory behavior.

**Review only:** Keep this as a draft PR. Do not merge or publish an installer before CI and real Windows/Ollama checks.
