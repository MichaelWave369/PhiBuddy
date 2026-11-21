# PhiBuddy v0.8: Evidence Registry (source alpha)

## Purpose

v0.7 suggested **personal notes** using local text matching. v0.8 starts a distinct evidence channel for documents fetched directly from their public HTTPS endpoints. It is *not* a fact-checker, general browser, search engine, web crawler, or autonomous agent tool.

The registry distinguishes:
1. **User notes:** manually saved by the user, optional source label, content unverified.
2. **Retrieved text snapshots:** obtained on explicit user request from a constrained HTTPS allowlist; source URL and response bytes recorded with timestamp and SHA-256. Publisher authenticity, claims and factual interpretation are *not* independently verified.
3. **Unverified claims:** agent prose not linked to a validated claim-checking procedure. A snapshot's existence does not upgrade any answer to verified.

## Isolated alpha test database

A different Git checkout does **not** normally mean a different database. By default the desktop still uses `%LOCALAPPDATA%\\PhiBuddy\\buddy.sqlite3` on Windows. To test without modifying your normal history, set `PHIBUDDY_DB_PATH` to a disposable, new file before launching:

```powershell
$env:PHIBUDDY_DB_PATH = Join-Path (Get-Location) 'phibuddy-v08-test.sqlite3'
npm run desktop
```

The path override applies only to the process launched from that shell. Existing SQLite data remains at its usual location; never reuse an important file as the test path. Remember that WAL files and backups can contain snapshot data, so deletion is not a full secure erase.

## Desktop workflow

Open **Evidence** in the right-side inspector. Paste a supported public HTTPS text URL, then click **Retrieve and save snapshot**. The desktop retrieves the text and stores it locally. You can inspect the exact saved text, content type, source host, retrieval time and SHA-256 fingerprint, or delete the snapshot with a confirmation action.

Use one of these sample addresses:

```text
https://www.rfc-editor.org/rfc/rfc9111.txt
https://raw.githubusercontent.com/rust-lang/rust/master/README.md
```

Actual data is fetched when the user clicks the button, not on app launch or in the background. No Ollama model receives the source text. No source is automatically imported into user memory, quoted as evidence, or added to any Council workflow.

## Network security boundary

The user-triggered Tauri backend uses `reqwest` and Rustls with:
- HTTPS only; **exact** hosts `raw.githubusercontent.com` or `www.rfc-editor.org`.
- GitHub Raw paths must name `owner/repo/ref/file` or deeper; RFC Editor accepts `/rfc/rfc<number>.txt` only. Encoded path segments are rejected.
- No embedded credentials, query strings, fragment identifiers, custom ports, redirects, cookies, auto proxy use, or arbitrary URL schemes.
- TLS certificate verification through the default stack, no certificate bypass, 8-second connection timeout and 25-second total timeout.
- HTTP success status required; plaintext, Markdown or octet-stream content types allowed only when the decoded bytes are valid UTF-8 text without NUL/embedded control characters.
- 256 KiB hard cap on *downloaded* response bytes. Known Content-Length over the cap fails before body reading; unknown length is bounded by a `take` reader.
- No HTML parsing, arbitrary content processing, JavaScript execution or shell/file execution.

Even allowlisted GitHub Raw documents may contain hostile prose: **retrieved text is treated as untrusted data**. No automatic model ingestion is implemented.

## Storage and provenance

New SQLite table `external_evidence`, separate from `memory_notes`, stores a UUID, URL, host, response MIME, retrieval timestamp, SHA-256 and complete accepted text body. The core verifies the content fingerprint before insertion; the snapshot can be inspected later without refetching. Source records are *retrieved_not_fact_checked*. Deletion removes the active row, not backups or filesystem copies. Existing chat and memory tables remain unchanged.

The snapshot timestamp is the local database insertion time, not a trusted timestamp authority. SHA-256 binds the stored content bytes to a digest but is not a publisher signature or guarantee of originality. The app does not independently validate authorship, cross-check multiple documents or verify claims against these sources.

## Tests and acceptance

Automated:
- URL policy rejects HTTP, local/private IPs, spoofed subdomains, credentials, custom ports, queries, fragments, percent-encoded paths, nonallowlisted domains, and wrong RFC paths.
- SQLite record storage rejects malformed or mismatched hashes, oversized text, and empty text; evidence stays separate from memory/conversations.
- A snapshot survives an on-disk database restart and disappears from active queries after explicit deletion.
- Existing Rust, Council, memory selection and cancellation tests pass on Ubuntu and Windows.
- React/TypeScript build and Windows Tauri build/tests pass at the exact PR head.

Manual Windows/Ollama smoke tests:
1. Back up the existing SQLite database and launch a local model.
2. Retrieve a short GitHub Raw Markdown source; inspect URL, content, hash and timestamp. Compare hash to a locally calculated SHA-256 if desired.
3. Retrieve a moderate RFC text file, then deliberately try a disallowed host, redirect/HTML URL and invalid local address, confirming rejection.
4. Restart PhiBuddy and verify snapshot records remain visible with no network call.
5. Delete a test record and verify active history no longer contains it.
6. Ask Buddy about the same topic and confirm the fetched snapshot is **not automatically included** in the conversation, event log or generated citation.
7. Test memory selection and cancellation from prior versions for regressions.

## Explicitly deferred

No general-purpose web access, nonallowlisted endpoints, crawling, OAuth, document/PDF parsing, embeddings, model-side browsing, automatic source injection or verified claim verdicts. A future evidence broker should implement explicit claim mapping, source authenticity checks, correlation rules and permissions before promising fact checking.

**Review only:** draft PR, no merge, no installer, no claim of hardware verification until the stated tests pass.
