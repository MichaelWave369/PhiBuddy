# PhiBuddy security boundaries (v0.8 alpha)

PhiBuddy is a prototype, **not a vetted secure execution platform**. The active surfaces are a Rust CLI and Tauri desktop preview. Both call a local Ollama API and write **unencrypted** SQLite state. The desktop streams text but does not give models shell or GitHub execution permissions.

- `phibuddy-core` does **not** execute commands, modify repository files, invoke shell or browser tools, or read secrets.
- Agents only generate text. Their proposals must never be mistaken for performed changes.
- The Ollama endpoint argument is limited to localhost/loopback HTTP, but the separate Ollama process may have its own network settings. The app does not guarantee that all model side effects or configuration are offline.
- Conversation history and raw per-agent drafts/criticisms are stored in SQLite unencrypted. Users control filesystem access and backup policy. `events --jsonl` may reveal sensitive local prompts if redirected or shared.
- v0.8 adds a desktop **user-initiated** HTTPS fetch for text files on allowlisted hosts `raw.githubusercontent.com` and `www.rfc-editor.org`. No agent-generated output triggers network requests. All other hosts, HTTP, redirects, unusual ports, query strings, URL credentials, HTML and oversized text responses are denied. Source URLs must be approved by the user in the desktop field.
- Captured `external_evidence` snapshots are unencrypted SQLite data with local retrieval time and SHA-256 fingerprint. SHA-256 detects accidental content changes if recalculated, **not** publisher identity, semantic correctness, or resistance to malicious local DB edits. No authenticated signatures, independent cross-source verification, or factual claim checking exist.
- The source allowlist is an initial risk reduction, not a proof of safety against compromised TLS trust, local host resolution, unsafe user input inside a source, or model prompt injection if future code starts adding external text to prompts. Neither the core nor the UI auto-attaches evidence records to model conversations.
- v0.7 local suggestions are computed only after the user presses **Find related notes**. It ranks a bounded set of saved notes using literal token overlap and gives **no** model access to suggestions until the user independently selects a note and submits a turn. Scores describe text similarity, not trust, accuracy, or truth.
- Age-based review warnings (90+ days, or invalid saved timestamps) are heuristics. They do not prove a note is stale, dangerous, or wrong; newer notes are not automatically reliable.
- The evidence panel currently has **no independently verified external source records**, even if the user types a URL-looking source label. No webpage fetch, authenticity check, or external source verification is performed.
- v0.6 desktop allows users to explicitly select up to five saved notes as **untrusted reference context for exactly one turn**. The native engine validates IDs against the active SQLite ledger, enforces a context-size limit, and records only ID-based attachment events. It does not automatically search, collect, or reuse notes.
- An assistant could repeat selected note content in saved answers, critiques, or partial output; deletion of the original memory row does not erase those derived copies. References labeled `[memory:…]` by the model are **model-generated and not verified citations**.
- Desktop note management adds only narrowly scoped local SQLite commands, not arbitrary filesystem, shell, GitHub, or external network access.
- New `memory_notes` entries are saved only through explicit user commands. They are **not automatically used** as model context, and optional `source_ref` labels are user-supplied strings, not independently validated provenance. Notes can be stale or malicious.
- Memory deletion removes an active SQLite row, **not guaranteed erasure** from WAL files, backups, filesystem snapshots, or exported copies. SQLite secure-delete is enabled for connections opened by PhiBuddy.
- Council Lab evaluation reports contain full prompts and generated answers. They are written only to an explicitly requested local file, refuse overwriting existing files, and may be partially populated if a later test case fails.
- Human evaluation ratings are unverified judgments and cannot establish objective model accuracy or reproducibility. Council results consume more inference requests than Solo.
- Prompt injection and memory poisoning are **not** solved by system prompts. There are no external tools, making the impact of malicious text in this release narrower, but the generated answer can still be wrong or manipulated.
- The event log is append-only **through PhiBuddy's API**, not cryptographically tamper-evident. OS users/processes with DB write permission can edit it.
- The schema is additive from v0.1, but no backup automation or disaster recovery workflow is implemented. Back up the SQLite database before upgrading.
- Never enable autonomous Git, shell, network writes, or MCP plugins without a separate deny-by-default capability broker, strong approval UI, staged diffs, and real Windows sandbox tests.

To report vulnerabilities privately, contact the maintainers through the repository's GitHub security reporting features where available; do not publish exploit details in a public issue.
