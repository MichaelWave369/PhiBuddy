# PhiBuddy v1.0 alpha: Claim Support Lab

## Goal

v0.9 lets a user attach an exact saved source excerpt to a single Council/Solo/Workshop turn. v1.0 adds a **user-directed local text comparison** so a confident model claim does not automatically inherit the credibility of that attachment.

**Not a stable 1.0 release.** This is `1.0.0-alpha.1`, a review-only implementation.

## What the Lab does

1. Open a previously recorded session and select a turn in Contributions or Timeline.
2. Paste **one claim**, 12–600 characters, in the Claim Support Lab panel.
3. Click **Compare against attached passages**. Only the excerpts explicitly attached to *that turn* are eligible. The app will not search unrelated stored evidence or automatically fetch new pages.
4. Review the result for each passage, with the source URL, SHA-256 and exact 1-based inclusive line range. Current source text is reconstructed only if its saved fingerprint and line range are valid.

The possible statuses are deliberately lexical:

- **Exact wording occurs in source:** the user's exact claim characters are present in the excerpt. A match says only that the source *contains those words*, not that the source or claim is true.
- **Related wording only:** at least two distinct non-stopword terms overlap. This is a weak editing/search hint, not a semantic entailment or confidence score.
- **No textual match:** neither criterion matched. It does not prove the claim is false or unsupported by another source.
- **Source unavailable:** the snapshot was removed, has changed, or its stored digest or selected line range is invalid, so source text is not recreated.

If no source was attached to that turn, the result has **zero findings** and states that no attached source can be checked. It never invents a citation or checks random database records.

## Security and privacy

The `review_turn_claim` Tauri command calls the shared Rust core and performs only SQLite reads. There is no model call, external HTTP request, autonomous search, new tool capability, persistence of user-reviewed claims, or automatic result logging.

Input is bounded to 12–600 characters and rejects control characters. The source anchor is read from the existing turn's audit event; the source snapshot hash is recomputed before any quote is displayed or compared. If the underlying snapshot is deleted or altered, its quote is unavailable instead of regenerated.

This does **not** solve prompt injection, external truth verification, author authentication, plagiarism, entailment or claim contradiction. A text fragment can be copied from a false source. Word overlap can occur in a sentence meaning the opposite. The model answer may already have repeated a source excerpt into its recorded output, which deleting the original snapshot won't erase. Local SQLite remains unencrypted.

## Automated checks

- Deterministic exact text occurrence, with `textual_comparison_only_not_fact_checked` as the always-explicit review status.
- Lexical overlap demonstrates why shared words cannot be called support or entailment.
- Unrelated claims show no match; missing/deleted sources show unavailable without returning original text.
- Wrong session, missing turn and invalid/short claims are rejected.
- Only passages actually attached to the chosen turn are checked; a new turn with no attachment has no findings.
- Existing v0.1-v0.9 event, memory, passage, streaming and provider tests remain green on Windows and Ubuntu.
- React build and Windows Tauri host compile/tests must be green on the exact review head.

## Manual acceptance on Windows

Use the existing `PHIBUDDY_DB_PATH` override to point the desktop at an unused test file. Run a known local Ollama model, retrieve a short public source in Evidence and select a few lines. Attach the passage to one turn and ask Buddy to paraphrase it.

In Contributions, paste an exact sentence from the excerpt. It should show **Exact wording occurs in source** while emphasizing that this does not certify truth. Next paste a claim with overlapping words but different meaning. It should show overlap only. Test unrelated wording, a turn with no attached source, and a source that has been deleted. Restart to verify the existing source anchors remain, but the temporary claim review is not saved.

No merge or public release until exact-head CI is green and the user-device smoke test is reviewed.

## Later research direction

Human-reviewed claim-to-source relationships, claim decomposition, negation-aware evaluation, cross-source contradiction detection, and trusted source authentication are **future work**, not claims about this alpha.
