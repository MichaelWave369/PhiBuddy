# PhiBuddy v0.4 streaming and cancellation

This feature keeps the local Rust core and existing CLI behavior intact while adding **real** streamed Ollama output to the Tauri desktop.

## Runtime contract

- `ModelProvider::complete_streaming` is optional. A provider implementing only `complete` still works and emits one complete delta; that fallback cannot interrupt a long blocking call.
- The desktop Ollama adapter uses `stream: true` and parses NDJSON frames. Each received `message.content` chunk is delivered through a typed Tauri 2 `Channel<TurnProgress>`.
- The core emits `turn_started`, `stage_started`, `delta`, and `stage_finished` progress notifications. Those are ephemeral UI events, **not model-generated progress percentages**.
- The UI displays the current stage and actual partial output. During a Council, each stage replaces the ephemeral preview while its **completed** original text is preserved as a SQLite deliberation event.
- A `cancel_turn` command sets an atomic cancellation flag **without acquiring the engine mutex**. The current model request exits when it observes the flag. The active slot is cleared even if the worker fails.
- If a stream is cancelled or fails after producing text, a `stage_partial` event stores that incomplete text. The turn ends with `turn_cancelled` or `turn_failed` and **does not enter future conversation history**.

## Explicit limitations

- Cancellation is cooperative. A stalled TCP read may delay response to the cancellation flag until Ollama produces another stream chunk or the request times out.
- Token chunks are emitted to the UI on arrival, but SQLite retains only completed stages and any interrupted stage text. It does **not** persist each individual token, avoiding excessive disk writes.
- The engine still holds a mutex during a model turn, so other commands that query the same store can wait until it ends. The cancellation command is independent and remains available.
- An existing CLI-only provider still uses the non-streaming fallback.
- The default local Ollama tag is `qwen3:4b`, but a localhost Ollama API can in principle route cloud models. Desktop rejects obvious `cloud` tags; local-only behavior must still be independently verified.
- No tools, filesystem operations, shell commands, external deployments, or autonomous agent execution have been introduced.
- No claim is made that every network interruption can be interrupted instantly.

## Acceptance

1. Existing CLI/core unit tests pass unchanged.
2. New unit test: Debate emits seven stages and seven fallback deltas.
3. New unit test: interrupting a stage stores `stage_partial` followed by `turn_cancelled`, without persisting a chat answer.
4. Windows Tauri Rust tests and compile checks pass, and React/TypeScript production build passes.
5. **Manual Windows + Ollama smoke test**: a Solo response visibly streams; Cancel stops a long Council before subsequent agents run; restart preserves partial evidence but not an incomplete chat pair.
6. No permissions were added to Tauri capability manifests.

## Quality gates before merge

- Review exact-head CI on Windows and Ubuntu.
- Validate with an actual local Ollama installation, not just mocks and compilation.
- Keep the PR in draft until manual acceptance passes.
