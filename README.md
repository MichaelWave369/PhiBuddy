# Φ PhiBuddy

**Buddy · Homie · Dude** · An open-source, local-first Rust collaboration app from PHI369 Labs.

PhiBuddy v0.2 is a **CLI and Rust core**, not a completed desktop app. It uses one locally running Ollama model in sequential calls. All three agent identities are role prompts and orchestrated workflows, not conscious persons or separately trained models.

## Already implemented

- Solo, Workshop, **Quick Council** (3 independent drafts + synthesis), and **Debate Council** (3 independent drafts + 3 cross-critiques + synthesis).
- A durable SQLite **deliberation event log** that preserves the original model outputs, including partial results if an Ollama call fails.
- Original conversation persistence, separate session IDs, old v0.1 database compatibility, and JSONL event inspection.
- No shell execution, browser actions, filesystem edits, API keys, autonomous agents, or GitHub writes.
- Unit tests for mode routing, independent drafts, critique round, SQLite events, schema migration, and provider failure.

## Desktop workbench preview (v0.3, draft PR)

An opt-in **Tauri 2 + React desktop preview** now lives in [apps/phibuddy-desktop](apps/phibuddy-desktop/README.md). It shares the existing Rust core and local SQLite sessions. The UI displays the original Council drafts, critiques, and event timeline. It is a source preview, **not a published installer or tested hardware release**.

On Windows with Rust, Node.js 22+, WebView2, MSVC Build Tools, and Ollama installed:

```powershell
cd apps/phibuddy-desktop
npm install
npm run desktop
```

The desktop is restricted to local Ollama, and no agent can execute commands or edit files. See [the desktop architecture and test checklist](docs/DESKTOP_V0.3.md). The CLI below remains fully supported.

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

Desktop streaming, sourced memory, security policy, sandboxed tools, optional PhiOS adapters, XR and voice are **design proposals**, not implemented features. We reviewed external architectural proposals from Grok, Claude and Hydrogen but intentionally kept this first build small and testable.

MIT License. Copyright (c) 2026 Michael Hughes.
