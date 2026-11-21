# PhiBuddy v0.5 Council Lab

Council Lab is a **measurement harness**, not a model-based correctness judge.

## Running a local experiment

Use Rust and an **actually local** Ollama model. From the repo root:

```powershell
cargo run -p phibuddy -- --model qwen3:4b eval --cases docs/eval_suite_smoke.json --output council-quick.jsonl --depth quick
cargo run -p phibuddy -- --model qwen3:4b eval --cases docs/eval_suite_smoke.json --output council-debate.jsonl --depth debate
```

These commands create new output files and refuse to overwrite existing ones. Each completed case emits a JSON object on one line; if Ollama fails midway, earlier completed cases remain in the file. Evaluation conversations run against an **in-memory SQLite database** and do not pollute your persistent chat history.

Each case tests the *same prompt* against two isolated sessions, using the same model: Solo Buddy (1 inference request) and Council Quick (4 requests) or Debate (7 requests). The execution order alternates across cases to reduce a systematic warm-cache advantage. Reported elapsed time includes model inference and local event writes. The optional `reference` comes from the human-created case file and is **never** passed to the model.

## What this measures

- The raw Solo and Council answers, available for independent human inspection.
- Wall-clock elapsed milliseconds for each full workflow.
- Model-request counts (1 vs. 4 or 7), not token- or energy-normalized costs.
- Which workflow ran first, the original question, case ID, and optional human reference.

## What this does NOT establish

- **No automatic accuracy score:** outputs require human review, objective unit tests, or a separately validated judge.
- **Not compute-matched:** three agents receive 4-7 model requests and thus usually more total inference than Solo.
- **Not a blinded trial:** the JSONL explicitly labels which answer is from Solo or Council.
- **No verified token accounting:** model requests are not equivalent to token counts or VRAM measurements.
- **No cross-model comparisons:** both variants use the same configured Ollama model; a model-family evaluation needs a separate controlled experiment.

## Input limits and privacy

The input format is `{"cases":[{"id":"unique_key","prompt":"...","reference":"optional"}]}`. Case IDs are limited to letters, digits, underscores, and dashes. Max 50 cases; prompt max 20,000 characters. The CLI rejects model names containing `cloud`, but **a loopback Ollama endpoint is not cryptographic proof of local-only inference**. Verify your model configuration before evaluating sensitive material.

Reports contain the complete user questions and all evaluated answers; keep them private. The current CLI outputs JSONL on disk only when you supply `--output`. No external telemetry or upload is implemented.

## Acceptance

1. Mock evaluation verifies 1/4 requests for Quick and 1/7 for Debate.
2. Test suites reject empty cases, duplicate IDs, invalid IDs, and overlong prompts.
3. Rust formatting, Clippy, and tests pass on Ubuntu and Windows.
4. Manually run the smoke suite against Ollama on Windows, inspect output and relative latency.
5. Do not claim Council is superior without an external scoring protocol and cost-controlled evidence.
