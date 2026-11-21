# PhiBuddy desktop workbench v0.3 (preview)

## Decision: reuse Rust core; add a narrow Tauri adapter

The Tauri 2 Rust host calls `phibuddy-core` without rewriting the Solo/Council/Workshop logic. The React frontend issues only six named commands: get_sessions, create_session, get_messages, get_events, check_ollama, send_prompt. No generic filesystem, shell, HTTP, plugin, or GitHub command is exposed to the webview.

The frontend displays saved chat, the original recorded per-agent text, and the event timeline. For a complete Council turn, the inspector distinguishes drafts, critiques, and synthesis; it never reconstructs agent viewpoints from the final answer. The GUI stores no data in localStorage.

## Deliberate v0.3 limits

- React is selected for this first preview for ecosystem familiarity. Svelte remains feasible; switching should follow observed accessibility/development problems, not unsupported framework safety claims.
- Inference still uses non-streaming Ollama HTTP calls through a blocking worker. Long Councils can take time. No fake percentages or token-stream claims.
- The desktop Rust adapter temporarily contains its own Ollama client implementation; moving it to a provider crate is later work to avoid widening the core before its API has stabilized.
- A mutex serializes prompts and DB queries within the desktop app. It prevents overlapping in-app turns but also delays event polling during a turn. True live-event streaming requires an engine stage callback or a separate event reader.
- Only local 127.0.0.1 Ollama is supported, with no arbitrary endpoint controls. External network adapters require explicit privacy policy later.
- No sandbox or tool executor has been implemented. The native process retains normal application privileges, but models have no commands to use those privileges.
- The preview intentionally disables release bundling and has no installer. The desktop requires a local build environment.
- For upgrades: make a backup of `%LOCALAPPDATA%\PhiBuddy\buddy.sqlite3` and related WAL/SHM files while PhiBuddy is closed.

## Acceptance checklist

1. Existing CLI CI remains green on Windows and Ubuntu.
2. Desktop TypeScript and Vite build green.
3. Tauri Rust host checks on Windows.
4. On Windows with Ollama, new Solo and Quick Council sessions finish with correct answer and original contribution text.
5. Debate produces three draft and three critique events before synthesis.
6. Close/reopen app and verify durable sessions/events.
7. No unapproved executor or network feature is present.
