# PhiBuddy security boundaries (v0.2)

PhiBuddy is a prototype, **not a vetted secure execution platform**. The active surface is a Rust CLI making HTTP requests to a local Ollama server and writing unencrypted SQLite state.

- `phibuddy-core` does **not** execute commands, modify repository files, invoke shell or browser tools, or read secrets.
- Agents only generate text. Their proposals must never be mistaken for performed changes.
- The Ollama endpoint argument is limited to localhost/loopback HTTP, but the separate Ollama process may have its own network settings. The app does not guarantee that all model side effects or configuration are offline.
- Conversation history and raw per-agent drafts/criticisms are stored in SQLite unencrypted. Users control filesystem access and backup policy. `events --jsonl` may reveal sensitive local prompts if redirected or shared.
- Prompt injection and memory poisoning are **not** solved by system prompts. There are no external tools, making the impact of malicious text in this release narrower, but the generated answer can still be wrong or manipulated.
- The event log is append-only **through PhiBuddy's API**, not cryptographically tamper-evident. OS users/processes with DB write permission can edit it.
- The schema is additive from v0.1, but no backup automation or disaster recovery workflow is implemented. Back up the SQLite database before upgrading.
- Never enable autonomous Git, shell, network writes, or MCP plugins without a separate deny-by-default capability broker, strong approval UI, staged diffs, and real Windows sandbox tests.

To report vulnerabilities privately, contact the maintainers through the repository's GitHub security reporting features where available; do not publish exploit details in a public issue.
