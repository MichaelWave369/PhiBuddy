# PhiBuddy Desktop v0.3 preview

A Tauri 2 + React workbench built on the existing `phibuddy-core` Rust library.

**Included:** create/resume sessions; Solo, Council (Quick or Debate), Workshop; inspect raw per-agent draft/critique and event history; local Ollama using `qwen3:4b` by default; same SQLite user-data file as CLI. Model calls remain sequential, but the desktop streams real Ollama tokens and accepts a cooperative cancellation request. Completed stages and interrupted partial text are recorded separately; unfinished turns do not become conversation history.

**Excluded:** shell execution, file modifications, GitHub actions, MCP, cloud inference, background tasks, file imports, telemetry, installers, and production release claims.

## Run on Windows

Install Node.js 22+, Rust, MSVC Build Tools (C++ workload), WebView2, and Ollama. Install a local model:

```powershell
ollama pull qwen3:4b
```

In `apps/phibuddy-desktop`:

```powershell
npm install
npm run desktop
```

To use an already installed model, configure `PHIBUDDY_MODEL` in PowerShell before launching:

```powershell
$env:PHIBUDDY_MODEL = "qwen3.5:9b"
npm run desktop
```

**Privacy note:** The app rejects model names containing `cloud`, since Ollama can route some tags to remote inference. This check cannot guarantee a renamed model or custom alias is local; verify your Ollama model's actual backend before entering sensitive data. The app has no telemetry of its own.

**Known limitations:** Cancellation waits for the next Ollama stream chunk (or request timeout) during a stalled read. The engine mutex still blocks other DB reads while a turn runs, but cancellation is independent.  First build must download dependencies. No release binaries or mobile packaging. The desktop currently links against the shared `phibuddy-core` directly; a future provider adapter will remove the small duplicated Ollama client from the CLI and desktop. The desktop and CLI share the same database file: SQLite WAL permits concurrency, but long desktop model turns hold this app's internal mutex. We have not tested simultaneous CLI and GUI usage yet.

## Validation

- Frontend: `npm install && npm run build`.
- Rust desktop: `cargo check --manifest-path src-tauri/Cargo.toml`.
- Rust core: from repository root, `cargo test --workspace`.
- Manual: launch Ollama, start a Quick Council turn, verify three independent draft cards and one synthesis; repeat with Debate (three critiques). Close and reopen, confirm history and events persist.

**Security:** Only six explicit Tauri commands are registered. Agent responses are plain text and are not executed or rendered as HTML. The capability manifest grants only Tauri core APIs. The local Ollama endpoint is hardcoded to `127.0.0.1:11434`. Tool execution remains absent.

## Streaming v0.4 acceptance

See [the streaming design](../../docs/STREAMING_V0.4.md). Test real Ollama streaming and interrupt a Council on Windows before merging.
