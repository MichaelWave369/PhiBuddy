# PhiBuddy Desktop v0.3 preview

A Tauri 2 + React workbench built on the existing `phibuddy-core` Rust library.

**Included:** create/resume sessions; Solo, Council (Quick or Debate), Workshop; inspect raw per-agent draft/critique and event history; local Ollama using `qwen3:4b` by default; same SQLite user-data file as CLI. Calls are sequential and synchronous **at the model level**. The Tauri command runs on a blocking worker so the UI remains responsive, but this preview does not yet stream tokens or support cancellation.

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

**Privacy note:** The app rejects model names containing `cloud`, since Ollama can route some tags to remote inference. This check cannot guarantee a renamed model or custom alias is local; verify your Ollama model's actual backend before entering sensitive data. The app has no telemetry of its own.\n\n**Known limitations:** First build must download dependencies. No release binaries or mobile packaging. The desktop currently links against the shared `phibuddy-core` directly; a future provider adapter will remove the small duplicated Ollama client from the CLI and desktop. The desktop and CLI share the same database file: SQLite WAL permits concurrency, but long desktop model turns hold this app's internal mutex. We have not tested simultaneous CLI and GUI usage yet.

## Validation

- Frontend: `npm install && npm run build`.
- Rust desktop: `cargo check --manifest-path src-tauri/Cargo.toml`.
- Rust core: from repository root, `cargo test --workspace`.
- Manual: launch Ollama, start a Quick Council turn, verify three independent draft cards and one synthesis; repeat with Debate (three critiques). Close and reopen, confirm history and events persist.

**Security:** Only six explicit Tauri commands are registered. Agent responses are plain text and are not executed or rendered as HTML. The capability manifest grants only Tauri core APIs. The local Ollama endpoint is hardcoded to `127.0.0.1:11434`. Tool execution remains absent.
