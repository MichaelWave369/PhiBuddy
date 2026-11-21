# Φ PhiBuddy

**Buddy · Homie · Dude** · An open-source, local-first Rust collaboration app from PHI369 Labs.

PhiBuddy has a **Rust CLI, reusable core, and Tauri 2 + React desktop preview**. It uses one locally running Ollama model in sequential calls. All three agent identities are role prompts and orchestrated workflows, not conscious persons or separately trained models.

## Already implemented

- Solo, Workshop, **Quick Council** (3 independent drafts + synthesis), and **Debate Council** (3 independent drafts + 3 cross-critiques + synthesis).
- A durable SQLite **deliberation event log** that preserves the original model outputs, including partial results if an Ollama call fails.
- Original conversation persistence, separate session IDs, old v0.1 database compatibility, and JSONL event inspection.
- No shell execution, browser actions, filesystem edits, API keys, autonomous agents, or GitHub writes.
- Unit tests for mode routing, independent drafts, critique round, SQLite events, schema migration, and provider failure.

## Desktop workbench preview (v0.3)

An opt-in **Tauri 2 + React desktop preview** now lives in [apps/phibuddy-desktop](apps/phibuddy-desktop/README.md). It shares the existing Rust core and local SQLite sessions. The UI displays the original Council drafts, critiques, and event timeline. It is a source preview, **not a published installer or tested hardware release**.

On Windows with Rust, Node.js 22+, WebView2, MSVC Build Tools, and Ollama installed:

```powershell
cd apps/phibuddy-desktop
npm install
npm run desktop
```

The desktop is restricted to local Ollama, and no agent can execute commands or edit files. See [the desktop architecture and test checklist](docs/DESKTOP_V0.3.md). The CLI below remains fully supported.

## Live desktop streaming (v0.4)

The desktop now supports real Ollama token streaming through Tauri channels, per-agent stage status, and a Cancel run button. Incomplete stages are traceable but **never** enter completed conversation history. See [streaming design and acceptance checklist](docs/STREAMING_V0.4.md).

## Council Lab and user-managed memory (v0.5)

The CLI now supports `eval` for side-by-side Solo-versus-Council experiments and `memory` for explicitly saved, source-labeled notes. These are *separate* features: benchmarks never add to your regular chat history, and memory notes are **not** automatically injected into agent prompts.

```powershell
cargo run -p phibuddy -- eval --cases docs/eval_suite_smoke.json --output council-quick.jsonl
cargo run -p phibuddy -- eval-score --report council-quick.jsonl --ratings my-human-ratings.json
cargo run -p phibuddy -- memory add --text "Be careful with unsupported claims" --source-ref "personal note"
cargo run -p phibuddy -- memory list
cargo run -p phibuddy -- memory search "claims"
```

See [Council Lab design](docs/COUNCIL_LAB_V0.5.md) and [Memory ledger guide](docs/MEMORY_V0.5.md). The benchmark report contains full prompts and answers; handle it as private data. The memory database is unencrypted.

## Desktop Memory Workshop (v0.6)

The desktop now offers an explicit **Memory** tab with save, literal search, per-turn note selection, deletion confirmation, and an attribution inspector. Selected notes are injected **only for that one message** and never harvested automatically. The event log retains selected note IDs, not copies of note bodies.

See [Memory Workshop v0.6 design and acceptance](docs/MEMORY_WORKSHOP_V0.6.md). This is development source, not an installer or production memory system.

## Memory Discovery (v0.7)

**Find related notes** in the desktop composer now runs a deterministic **local** lexical search over up to 1,000 saved user notes. It previews up to 8 matches, shows the terms responsible for each relevance score, and warns when a note is 90+ days old or has an unreadable saved date. It never silently selects or sends a note to a model: check **Attach next turn** yourself.

The inspector distinguishes **unverified user notes** from **independently verified external sources**, explicitly displaying that no external source verification is implemented yet. Source labels, including URL-looking strings, remain user-supplied data.

Read [Memory Discovery v0.7 design and acceptance](docs/MEMORY_DISCOVERY_V0.7.md). This is development source, not an installer or externally verified evidence system.

## External Evidence Registry (v0.8)

The desktop **Evidence** tab lets you explicitly retrieve UTF-8 plaintext from a small HTTPS source allowlist. Every local snapshot has a URL, source host, retrieval timestamp, and SHA-256 fingerprint, and is kept **separate** from user memory and chat history. Neither retrieval nor storage makes a document factually verified. Evidence snapshots are **not automatically shown to Ollama**, and no agent can invoke retrieval.

Supported inputs include `https://www.rfc-editor.org/rfc/rfc9111.txt` and a GitHub Raw text file such as `https://raw.githubusercontent.com/rust-lang/rust/master/README.md`. Fetches reject arbitrary hosts, HTML, redirects, credentials, query strings, non-UTF-8 data, and responses over 256 KiB.

Read [Evidence Registry v0.8 architecture and acceptance](docs/EVIDENCE_REGISTRY_V0.8.md). Review source only; not a release or a general web browser.

Set `PHIBUDDY_DB_PATH` in your test shell to use an isolated SQLite database. A fresh clone by itself still shares the normal database path.

## Evidence-backed reasoning: explicit source passages (v0.9)

The desktop can now **preview an exact range of lines from a previously saved external source**. You must separately choose **Attach this passage to next turn**. The core limits attachments to three passages and 6000 serialized context characters; selected passages never carry over to another message.

For each affected turn, the inspector shows the quote reconstructed from the saved text, source URL, line range and SHA-256 fingerprint. Deleting or changing the source makes the original quote unavailable in the inspector. The event log retains source IDs and fingerprints without copying entire passages.

This is an evidence *trace*, not independent fact checking: Buddy's interpretations are still AI-generated. See [Source Passage v0.9 guide](docs/SOURCE_PASSAGES_V0.9.md).

## Claim Support Lab (v1.0 alpha, draft)

**Claim Support Lab** lets you paste one claim from a model answer and compare it against the exact external passages **actually attached to that turn**. It reports direct text occurrence, overlap-only, no textual match, or missing/changed evidence. The action is entirely local and read-only: no model judging, network call, autonomous research, write to SQLite, or fabricated truth verdict.

A text match is not factual entailment. The lab cannot determine whether a source's claims are true or whether a paraphrased inference is correct. See [Claim Support Lab design and acceptance](docs/CLAIM_SUPPORT_LAB_V1.0.md).

## Run the CLI on Windows 11

Prerequisites: Rust stable toolchain (`cargo --version`), Ollama (`ollama list`) and an installed model. `qwen3:4b` is the default and can be changed using `--model`.

From the **directory containing `Cargo.toml`**:

```powershell
cargo test --workspace
cargo run -p phibuddy -- check
cargo run -p phibuddy -- chat --mode solo --agent buddy
cargo run -p phibuddy -- chat --mode council --depth quick --show-contributions
cargo run -p phibuddy -- chat --mode council --depth debate --show-contributions
cargo run -p phibuddy -- chat --mode workshop
```

`quick` makes four sequential Ollama calls; `debate` makes seven. On a 12 GB GPU, start with `quick`, then try `debate` for difficult questions. Each response may be slow depending on the model and prompt size.

In interactive chat: `:exit` quits, `:id` shows the session ID, `:help` shows options. `:paste` begins a multi-line prompt; end with `:end` on a line by itself. (Maximum prompt length: 20,000 characters.)

**Inspect real agent contributions:**

```powershell
cargo run -p phibuddy -- sessions
cargo run -p phibuddy -- history YOUR_SESSION_ID
cargo run -p phibuddy -- events YOUR_SESSION_ID
cargo run -p phibuddy -- events YOUR_SESSION_ID --jsonl
cargo run -p phibuddy -- chat --session YOUR_SESSION_ID --depth debate --show-contributions
```

The `events` command is **read-only** and prints the audit-style deliberation stream. It does not export to a file unless you use shell redirection, e.g. `... --jsonl > events.jsonl` (be aware those events contain your prompts and model output).

The database defaults to `%LOCALAPPDATA%\PhiBuddy\buddy.sqlite3` on Windows and `~/.local/share/phibuddy/buddy.sqlite3` elsewhere. Override with `--db PATH`. Data is unencrypted. Back up your v0.1 database before upgrading; this release creates an additive table without deleting old history.

## Build validation

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

See [Architecture](docs/ARCHITECTURE.md), [Validation](docs/VALIDATION.md) and [Security](SECURITY.md). **No claim of an externally verified release** until CI and live Windows smoke tests succeed.

## What comes later

Independent claim verification, general web retrieval, semantic source matching, sandboxed tools, optional PhiOS adapters, XR and voice are **design proposals**, not implemented features. We reviewed external architectural proposals from Grok, Claude and Hydrogen but intentionally kept this first build small and testable.

MIT License. Copyright (c) 2026 Michael Hughes.
