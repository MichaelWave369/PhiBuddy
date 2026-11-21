# PhiBuddy v0.6 Memory Workshop (source alpha)

This milestone adds a graphical interface to the **user-controlled** v0.5 SQLite memory ledger and introduces **per-turn, explicit note attachment**. Neither the CLI nor the desktop automatically harvests, searches, or injects memories into prompts.

## Desktop workflow

1. Launch the Tauri desktop app and open the **Memory** inspector tab.
2. Type a note and optionally identify its source (e.g. "my design notebook"). Click **Save this note**.
3. Use **Search / show recent** to find a note with a literal substring search. The view displays up to 100 recent/matching notes.
4. Check **Attach next turn** next to one or more notes (maximum **five**).
5. The composer shows the selected count. Submit the next message. **Only that turn** receives those notes. The selection immediately clears so a future prompt cannot inherit them by accident.
6. In **Contributions** or **Timeline**, pick that turn. The **Notes attached to this turn** section shows the actual note IDs, source labels, and currently stored text. This proves which notes were supplied to the model, **not** that the model relied on or verified them.
7. Use **Delete note** and **Confirm delete** to remove it from the active memory ledger. Old turn events retain the ID alone. The inspector displays "Content unavailable after deletion" rather than restoring a deleted note.

You can also use the existing CLI commands `phibuddy memory add|list|search|forget`; both surfaces share the normal local database path.

## Architecture and security

- The desktop exposes four narrow Tauri commands: `list_memories`, `save_memory`, `delete_memory`, and `get_turn_memories`. There is **no** general shell, file, or HTTP command.
- `Engine::run_observed_with_memories` resolves user-selected IDs from SQLite **before** recording a turn or making a model request. Missing or duplicate IDs, more than five notes, and more than **6,000 total characters** (note bodies plus metadata) are rejected.
- The engine serializes selected notes as JSON into one ephemeral **user-role context message**, clearly labeled `UNTRUSTED DATA`, immediately before the current request. All independent Council drafts see the same context. Existing CLI calls continue through `run_observed` without memory context.
- Each selected note gets a `memory_attached` event with its **ID only**. This event is retained for a completed, interrupted, or failed turn. No note body or source label is duplicated in that event.
- The model is asked to identify user-note-based claims as `[memory:<note-id>]`, but the UI does **not** mistake these generated markings for verified citations. The inspector resolves its labels directly from SQLite.
- The previous v0.5 memory table and chat/event schema remain intact; the change adds one event type and a new optional invocation path. The database still contains private **unencrypted** data.
- Deleting a note does not remove its text from **already generated assistant messages**, partial output, exports, backups, WAL, filesystem snapshots, or Ollama's own operating environment. There is **no promise of total erasure** or proof of offline-only processing merely because Ollama listens on localhost.
- Notes may contain mistaken information or prompt-injection instructions. Labelling them "untrusted" mitigates but does not magically prevent model-level influence. Agents still have no tool-execution privileges.

## Validation gate

Automated:
- Existing Solo/Council/Workshop and cancellation tests must stay green on Windows and Ubuntu.
- Explicit note context test verifies the same selected note appears in all independent Council drafts; event metadata stores its ID without copying its body; completed chat history excludes the injected context message.
- Reuse test verifies selected memory **does not** carry over into the next turn.
- Missing/duplicate/deleted note IDs fail without beginning a turn or making a model request.
- Desktop source inspector test verifies that deleting an attached note changes its resolved content to unavailable, leaving only the ID in event history.
- React TypeScript/Vite and Windows Tauri host compilation and tests must pass.

Manual on the user's Windows system (still required):
- Start Ollama with a verified local model.
- Add a test note, search for it, select it, send a Solo and then a Quick Council message, and inspect their note-ID source records.
- Send another message without selecting anything and confirm **zero** `memory_attached` events for that new turn.
- Delete the note and confirm the prior turn inspector shows content unavailable.
- Restart the desktop and verify the remaining note list and prior ID-only events.
- Cancel an attached-note Council midway: the event ID should persist, but no incomplete user/assistant chat pair should be committed.

**Release policy:** draft PR, no automatic merge, no public installer, no real-world privacy assurance or performance claim without those manual checks.
