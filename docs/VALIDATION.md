# PhiBuddy v0.2 validation notes

## Evidence already established on the user's Windows machine (v0.1)

- Rust toolchain and Cargo installed.
- `cargo test --workspace`: 6 of 6 unit tests passed for the original source package.
- `cargo run -p phibuddy -- check`: local Ollama reachable.
- `cargo run -p phibuddy -- chat --mode solo --agent buddy`: model returned real responses.

These observations apply to **v0.1**, not automatically to v0.2.

## v0.2 status

- Source constructed and reviewed for consistency; CI validation and live Ollama test **not yet claimed** until results are returned.
- Added SQLite deliberation event log, per-agent trace API, quick/debate Council depths, and migration/failure tests.
- No GUI, daemon, tool execution, model streaming, real quality benchmark, or security sandbox exists.

## Commands for maintainer

```powershell
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p phibuddy -- check
cargo run -p phibuddy -- chat --mode council --depth debate --show-contributions
cargo run -p phibuddy -- sessions
cargo run -p phibuddy -- events <SESSION_ID> --jsonl
```

Try `:paste` then paste a large prompt, finishing with `:end` on its own line. Use `:id` to see the current session ID, and `:exit` to stop.

If CI fails, do not describe the source as release-ready. Diagnose, patch, and rerun exact-head checks before considering a merge. Never merge without human approval.
