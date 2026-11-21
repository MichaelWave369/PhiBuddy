# PhiBuddy v0.9 source-passage reasoning

## Purpose

v0.8 recorded user-requested public text snapshots. v0.9 adds *one-turn, human-chosen source excerpts*, strictly separating exact quote text from model interpretation and unsupported claims.

The app does not do objective fact checking, web search, semantic verification, or a trustworthy citation evaluation. It can establish that a particular text excerpt was part of a saved source snapshot at an attached SHA-256 fingerprint.

## Desktop workflow

1. Start the v0.9 desktop using `PHIBUDDY_DB_PATH` pointed at a **new, disposable SQLite database** if you are testing. A fresh clone without that environment setting opens your normal PhiBuddy database.
2. In **Evidence**, explicitly retrieve one supported public text file from the v0.8 allowlist, or use a previously saved snapshot.
3. Expand **Inspect saved text**. The first 200 lines are numbered in the UI, and any other valid range is accepted.
4. Enter 1-based start/end line numbers (inclusive) and click **Preview exact passage**. The Rust core reads the saved source, recomputes SHA-256, rejects mismatches and out-of-range requests, and supplies a bounded exact quote.
5. Click **Attach this passage to next turn** to explicitly queue it. Preview alone never attaches anything.
6. Send a Solo, Council, or Workshop prompt. Up to three selected passages are allowed; each selected passage is no longer than 1,600 characters, and all source context JSON plus wrapper text must not exceed 6,000 characters.
7. Open the turn in **Contributions** or **Timeline**. The inspector displays the source quote, URL, original line range and hash. If the source is removed or changed, the old quote becomes unavailable while the audit anchor remains.
8. Send another prompt without choosing a passage. It must receive no source passage context.

## What's stored, what's shown

- `external_evidence` retains user-retrieved text, URL, MIME, retrieved-at time and SHA-256. It is distinct from `memory_notes`.
- New event kind `source_passage_attached` contains JSON of `{selection:{evidence_id,start_line,end_line},sha256}`. It **does not copy the passage body**, source URL or other private content.
- The engine wraps selected passages as **untrusted reference data**, requires the LLM to separate **direct quotation**, **inference** and **unsupported/unverified claims**, and forbids claiming that the source is independently verified. These are prompts, not guarantees of LLM compliance.
- In a Council, all independent drafts receive the same excerpts. The selection is not persisted into normal user/assistant chat history; model-generated answers or intermediate drafts can still repeat quoted text.
- An old binary that doesn't recognize the new event kind may fail when opening these turns. Back up your SQLite database before testing or downgrading.

## Threat and accuracy limits

A byte-level exact quote proves only what the saved text contained. SHA-256 is a content fingerprint, not an author signature, independent timestamp or external truth check. A malicious document can contain instructions, false claims, or unsafe code. The model has no execution tools and must treat excerpts as untrusted, but prompt injection is not mathematically eliminated.

The ledger and chats are **unencrypted**. Deleting a saved source does not delete copies from old model answers, exported reports, WAL/backups, or filesystem snapshots. The inspector intentionally refuses to reconstruct deleted or hash-mismatched source text.

Source selection happens in the desktop UI only. The CLI still works without sources. There is no agent-initiated browser, automatic evidence-to-model injection, external source search or claimed verification verdict.

## Acceptance tests

Automated:
- Exact CRLF/LF line quote reconstruction and retained URL/hash/line metadata.
- Invalid line ranges, duplicate passages, missing/deleted sources, and oversize passages rejected before starting a turn or inference.
- SHA-256 recomputed before preview and turn context creation.
- Identical source context supplied to all independent Council calls; no carryover to next turn.
- Source audit events contain only ID/range/hash, not excerpt bytes.
- Turn inspector resolves original quote only while source record and fingerprint still match.
- Existing Solo/Workshop/Council, cancellation, manual memory and v0.8 source fetch protections remain green.
- Ubuntu/Windows Rust fmt, Clippy and tests; React build; Windows Tauri host checks.

Manual, on Windows with verified local Ollama:
1. Clone the feature branch, point `PHIBUDDY_DB_PATH` to an unused test database.
2. Retrieve a short RFC or GitHub Raw file and inspect it.
3. Preview a passage and confirm that previewing alone does not queue or send it.
4. Queue a passage, send a prompt, inspect the exact anchor and quote. Repeat without attaching and verify no new source event.
5. Delete the snapshot, reopen the prior turn, and verify that original quoted text becomes unavailable in the inspector.
6. Try an invalid range and a passage longer than 1,600 characters; neither should reach Ollama.
7. Interrupt a Council with a selected passage and verify no incomplete normal chat pair is committed.

**Release gate:** This is a draft PR. No merge, public installer or claim of fully verified inference until CI and manual acceptance have passed.
