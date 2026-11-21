# PhiBuddy v0.5 Evidence-Labelled Memory Ledger

The first memory milestone is deliberately **user-controlled**. It is NOT automatic long-term model memory and does not inject notes into model prompts.

## Commands (from the repo root)

```powershell
cargo run -p phibuddy -- memory add --text "Prefer explicit evidence before consensus" --source-ref "my architecture notes"
cargo run -p phibuddy -- memory list
cargo run -p phibuddy -- memory search "evidence"
cargo run -p phibuddy -- memory forget "PASTE-SAVED-MEMORY-ID"
```

On Windows, notes are stored in `%LOCALAPPDATA%\PhiBuddy\buddy.sqlite3` by default, alongside the existing sessions. The schema migration adds a `memory_notes` table without deleting old conversation history. Every saved entry gets a UUID, timestamp, `source_kind=user_explicit`, and optional source label supplied by the user. That label is **not independently verified provenance**.

Search uses a simple SQLite case-insensitive substring match (ASCII folding, not semantic retrieval). The list/search result limit defaults to 50 and is capped at 200. Notes are not automatically extracted from user chats or model outputs and are never silently inserted into Council context.

## Privacy and deletion

Data is stored locally **without encryption**. Running with your standard OS account protects the directory through normal filesystem permissions, but this is not a cryptographic guarantee. Deleting a note removes the active SQLite row. SQLite secure-delete is enabled for newly opened databases, but deleted fragments may remain in WAL files, backups, filesystem snapshots, or other copies. **Do not treat the command as secure erasure of every copy.**

Back up the database before upgrade and close both CLI and desktop before copying or restoring SQLite files. Never place private memory dumps in a public issue, PR, or benchmark file.

## Why memory is not auto-injected yet

Evidence labels and user control must come before retrieval-augmented prompts. A note can be stale, misleading, or contain prompt-injection instructions, so later opt-in retrieval needs clear scope, source attribution, age limits, and untrusted-data boundaries.

## Acceptance

1. Explicit write, list, literal search, and delete work with no Ollama process running.
2. Memory inputs with empty/oversized content are rejected.
3. Existing v0.1-v0.4 databases migrate additively without losing old chat history.
4. No new Tauri permissions, network API, or autonomous memory ingestion is introduced.
