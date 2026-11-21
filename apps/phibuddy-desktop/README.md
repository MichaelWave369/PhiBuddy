# PhiBuddy Desktop v0.7 source alpha

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

**Security:** Twelve narrow Tauri commands are registered. Agent responses are plain text and are not executed or rendered as HTML. The capability manifest grants only Tauri core APIs. The local Ollama endpoint is hardcoded to `127.0.0.1:11434`. Tool execution remains absent.

## Memory Workshop v0.6

Open the **Memory** inspector tab to add, search and delete user-owned notes. Check notes to attach them to **only the next turn**. Attached note IDs appear in the Council inspector, resolved to the current local SQLite note content and user-supplied source labels. **No note is automatically extracted or included.** Notes are unverified user data. The context budget is five notes / 6,000 characters.

See [the v0.6 safety boundaries and acceptance checklist](../../docs/MEMORY_WORKSHOP_V0.6.md). Changes are not a production release and remain subject to actual Windows/Ollama verification.

## Memory Discovery v0.7

Enter a draft in chat and click **Find related notes**. The Memory tab previews up to eight locally ranked suggestions and their matched terms. **None** are attached unless you separately check **Attach next turn** and send. Notes 90+ days old are marked **Review recommended**, not treated as incorrect. The evidence inspector distinguishes unverified personal notes from independently verified external references, which are unavailable in this alpha.

See [v0.7 design, ranking limitations and smoke tests](../../docs/MEMORY_DISCOVERY_V0.7.md).

## Streaming v0.4 acceptance

See [the streaming design](../../docs/STREAMING_V0.4.md). Test real Ollama streaming and interrupt a Council on Windows before merging.
