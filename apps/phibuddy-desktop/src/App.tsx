import { useEffect, useMemo, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";

type Session = { id: string; mode: string; agent: string; created_at: string };
type Message = { role: string; actor: string | null; content: string };
type Event = {
  sequence: number; schema_version: number; session_id: string;
  turn_id: string; kind: string; actor: string | null;
  content: string; created_at: string;
};
type MemoryRecord = {
  id: string; content: string; source_kind: string;
  source_ref: string | null; created_at: string;
};
type TurnMemoryRef = { id: string; memory: MemoryRecord | null };
type PassageSelection = {
  evidence_id: string; start_line: number; end_line: number;
};
type PassageAnchor = { selection: PassageSelection; sha256: string };
type PassagePreview = {
  selection: PassageSelection;
  source_url: string; retrieved_at: string; sha256: string;
  quote: string; status: "retrieved_not_fact_checked";
};
type TurnPassageRef = { anchor: PassageAnchor; preview: PassagePreview | null };
type EvidenceRecord = {
  id: string;
  source_url: string;
  source_host: string;
  content_type: string;
  content_sha256: string;
  retrieved_at: string;
  content: string;
  status: "retrieved_not_fact_checked";
};
type MemorySuggestion = {
  note: MemoryRecord;
  relevance_score: number;
  matched_terms: string[];
  age_days: number | null;
  review_recommended: boolean;
  provenance_status: "unverified_user_note";
};

type Agent = "buddy" | "homie" | "dude";
type Mode = "solo" | "council" | "workshop";
type Depth = "quick" | "debate";
type TurnProgress =
  | { event: "turn_started"; turn_id: string }
  | { event: "stage_started"; agent: string; phase: string }
  | { event: "delta"; agent: string; phase: string; text: string }
  | { event: "stage_finished"; agent: string; phase: string };

const agents: Agent[] = ["buddy", "homie", "dude"];
const modes: Mode[] = ["solo", "council", "workshop"];
const initials: Record<Agent, string> = { buddy: "B", homie: "H", dude: "D" };

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [events, setEvents] = useState<Event[]>([]);
  const [mode, setMode] = useState<Mode>("solo");
  const [agent, setAgent] = useState<Agent>("buddy");
  const [depth, setDepth] = useState<Depth>("quick");
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [cancelRequested, setCancelRequested] = useState(false);
  const [currentStage, setCurrentStage] = useState("");
  const [liveOutput, setLiveOutput] = useState("");
  const [error, setError] = useState("");
  const [status, setStatus] = useState("Checking Ollama…");
  const [inspector, setInspector] = useState<"events" | "contributions" | "memory" | "evidence">("contributions");
  const [memories, setMemories] = useState<MemoryRecord[]>([]);
  const [memorySearch, setMemorySearch] = useState("");
  const [memoryDraft, setMemoryDraft] = useState("");
  const [memorySource, setMemorySource] = useState("");
  const [memoryError, setMemoryError] = useState("");
  const [memoryBusy, setMemoryBusy] = useState(false);
  const [selectedMemoryIds, setSelectedMemoryIds] = useState<string[]>([]);
  const [activeMemoryCount, setActiveMemoryCount] = useState(0);
  const [turnMemoryRefs, setTurnMemoryRefs] = useState<TurnMemoryRef[]>([]);
  const [memoryRevision, setMemoryRevision] = useState(0);
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  const [suggestedNotes, setSuggestedNotes] = useState<MemorySuggestion[]>([]);
  const [suggestedFor, setSuggestedFor] = useState("");
  const [suggesting, setSuggesting] = useState(false);
  // Discard old async suggestions when a draft/session/memory changes.
  const suggestionRequestRef = useRef(0);
  const [selectedTurn, setSelectedTurn] = useState<string | null>(null);
  const [evidence, setEvidence] = useState<EvidenceRecord[]>([]);
  const [evidenceUrl, setEvidenceUrl] = useState("");
  const [evidenceBusy, setEvidenceBusy] = useState(false);
  const [evidenceError, setEvidenceError] = useState("");
  const [evidenceDeleteConfirm, setEvidenceDeleteConfirm] = useState<string | null>(null);
  const [evidenceSelected, setEvidenceSelected] = useState<string | null>(null);
  const [passageStart, setPassageStart] = useState("1");
  const [passageEnd, setPassageEnd] = useState("3");
  const [passagePreview, setPassagePreview] = useState<PassagePreview | null>(null);
  const [selectedPassages, setSelectedPassages] = useState<PassageSelection[]>([]);
  const [activePassageCount, setActivePassageCount] = useState(0);
  const [turnPassages, setTurnPassages] = useState<TurnPassageRef[]>([]);

  const active = sessions.find((s) => s.id === activeId);
  const visibleMessages = messages.filter((m) => m.role !== "contribution");
  const turns = useMemo(
    () => [...new Set(events.map((e) => e.turn_id))].reverse(),
    [events]
  );
  const inspectedTurn = selectedTurn && turns.includes(selectedTurn) ? selectedTurn : turns[0];
  const turnEvents = events.filter((e) => e.turn_id === inspectedTurn);
  const contributions = turnEvents.filter((e) =>
    ["agent_draft", "agent_critique", "workshop_plan", "workshop_ideas",
     "workshop_build", "solo_answer", "synthesis", "stage_partial"].includes(e.kind)
  );

  async function loadSession(id: string) {
    const [chat, log] = await Promise.all([
      invoke<Message[]>("get_messages", { sessionId: id }),
      invoke<Event[]>("get_events", { sessionId: id }),
    ]);
    setMessages(chat);
    setEvents(log);
    setSelectedTurn(log.length ? log[log.length - 1].turn_id : null);
  }

  async function loadMemories(query = "") {
    const result = await invoke<MemoryRecord[]>("list_memories", {
      query: query.trim() || null,
    });
    setMemories(result);
  }

  async function refreshMemories() {
    setMemoryError("");
    try { await loadMemories(memorySearch); }
    catch (e) { setMemoryError(errorText(e)); }
  }

  async function saveMemory() {
    if (busy || memoryBusy || !memoryDraft.trim()) return;
    suggestionRequestRef.current += 1;
    setSuggesting(false);
    setMemoryBusy(true);
    setMemoryError("");
    try {
      await invoke<MemoryRecord>("save_memory", {
        content: memoryDraft.trim(),
        sourceRef: memorySource.trim() || null,
      });
      setMemoryDraft("");
      setMemorySource("");
      setMemorySearch("");
      setSuggestedNotes([]);
      setSuggestedFor("");
      await loadMemories();
      setMemoryRevision((n) => n + 1);
    } catch (e) { setMemoryError(errorText(e)); }
    finally { setMemoryBusy(false); }
  }

  async function deleteMemory(id: string) {
    if (busy || memoryBusy || pendingDelete !== id) return;
    suggestionRequestRef.current += 1;
    setSuggesting(false);
    setMemoryBusy(true);
    setMemoryError("");
    try {
      await invoke<boolean>("delete_memory", { memoryId: id });
      setSelectedMemoryIds((ids) => ids.filter((value) => value !== id));
      setSuggestedNotes((items) => items.filter((item) => item.note.id !== id));
      setPendingDelete(null);
      await loadMemories(memorySearch);
      setMemoryRevision((n) => n + 1);
    } catch (e) { setMemoryError(errorText(e)); }
    finally { setMemoryBusy(false); }
  }

  function toggleMemory(id: string) {
    if (busy || memoryBusy) return;
    if (!selectedMemoryIds.includes(id) && selectedMemoryIds.length >= 5) {
      setMemoryError("At most five notes may be attached to one turn.");
      return;
    }
    setMemoryError("");
    setSelectedMemoryIds((ids) =>
      ids.includes(id) ? ids.filter((value) => value !== id) : [...ids, id]
    );
  }

  async function suggestFromDraft() {
    if (busy || memoryBusy || suggesting) return;
    const query = prompt.trim();
    if (!query) return;
    const requestId = ++suggestionRequestRef.current;
    setInspector("memory");
    setMemoryError("");
    setSuggesting(true);
    try {
      const found = await invoke<MemorySuggestion[]>("suggest_memories", { query });
      if (requestId === suggestionRequestRef.current) {
        setSuggestedNotes(found);
        setSuggestedFor(query);
      }
    } catch (e) {
      if (requestId === suggestionRequestRef.current) {
        setSuggestedNotes([]);
        setSuggestedFor("");
        setMemoryError(errorText(e));
      }
    } finally {
      if (requestId === suggestionRequestRef.current) setSuggesting(false);
    }
  }

  async function loadEvidence() {
    const items = await invoke<EvidenceRecord[]>("list_external_evidence");
    setEvidence(items);
  }

  async function captureEvidence() {
    if (busy || evidenceBusy || !evidenceUrl.trim()) return;
    setEvidenceBusy(true);
    setEvidenceError("");
    try {
      const record = await invoke<EvidenceRecord>("fetch_external_evidence", {
        url: evidenceUrl.trim(),
      });
      await loadEvidence();
      setEvidenceSelected(record.id);
      setEvidenceUrl("");
    } catch (e) {
      setEvidenceError(errorText(e));
    } finally {
      setEvidenceBusy(false);
    }
  }

  async function removeEvidence(id: string) {
    if (busy || evidenceBusy || evidenceDeleteConfirm !== id) return;
    setEvidenceBusy(true);
    setEvidenceError("");
    try {
      await invoke<boolean>("delete_external_evidence", { evidenceId: id });
      setEvidenceDeleteConfirm(null);
      if (evidenceSelected === id) setEvidenceSelected(null);
      if (passagePreview?.selection.evidence_id === id) setPassagePreview(null);
      setSelectedPassages((items) => items.filter((item) => item.evidence_id !== id));
      await loadEvidence();
    } catch (e) {
      setEvidenceError(errorText(e));
    } finally {
      setEvidenceBusy(false);
    }
  }

  async function inspectPassage(evidenceId: string) {
    if (busy || evidenceBusy) return;
    const startLine = Number(passageStart);
    const endLine = Number(passageEnd);
    if (!Number.isSafeInteger(startLine) || !Number.isSafeInteger(endLine) || startLine < 1 || endLine < startLine) {
      setEvidenceError("Choose valid positive start and end lines.");
      return;
    }
    setEvidenceError("");
    setEvidenceBusy(true);
    setPassagePreview(null);
    try {
      const preview = await invoke<PassagePreview>("preview_evidence_passage", {
        evidenceId, startLine, endLine,
      });
      setPassagePreview(preview);
    } catch (e) {
      setEvidenceError(errorText(e));
    } finally { setEvidenceBusy(false); }
  }

  function attachPreviewPassage() {
    if (!passagePreview || busy || evidenceBusy) return;
    const next = passagePreview.selection;
    if (selectedPassages.length >= 3) {
      setEvidenceError("Attach at most three passages per turn.");
      return;
    }
    if (selectedPassages.some((entry) =>
      entry.evidence_id === next.evidence_id &&
      entry.start_line === next.start_line &&
      entry.end_line === next.end_line
    )) {
      setEvidenceError("This exact passage is already selected.");
      return;
    }
    setEvidenceError("");
    setSelectedPassages((items) => [...items, { ...next }]);
    setPassagePreview(null);
  }

  async function reloadSessions() {
    const result = await invoke<Session[]>("get_sessions");
    setSessions(result);
    return result;
  }

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [result, ping] = await Promise.all([
          invoke<Session[]>("get_sessions"),
          invoke<string>("check_ollama"),
        ]);
        if (cancelled) return;
        setSessions(result);
        setStatus(ping);
        if (result[0]) {
          setActiveId(result[0].id);
          await loadSession(result[0].id);
        }
      } catch (e) {
        if (!cancelled) {
          setStatus("Ollama unavailable (existing history still works)");
          setError(errorText(e));
          try {
            const result = await invoke<Session[]>("get_sessions");
            if (!cancelled) {
              setSessions(result);
              if (result[0]) { setActiveId(result[0].id); await loadSession(result[0].id); }
            }
          } catch (failure) {
            if (!cancelled) setError(errorText(failure));
          }
        }
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // Evidence records are local and can be inspected without Ollama.
  useEffect(() => {
    void loadEvidence().catch((e) => setEvidenceError(errorText(e)));
  }, []);

  useEffect(() => {
    let ignore = false;
    if (!activeId || !inspectedTurn) {
      setTurnPassages([]);
      return;
    }
    void invoke<TurnPassageRef[]>("get_turn_passages", {
      sessionId: activeId, turnId: inspectedTurn,
    }).then((items) => {
      if (!ignore) setTurnPassages(items);
    }).catch((e) => {
      if (!ignore) {
        setTurnPassages([]);
        setEvidenceError(errorText(e));
      }
    });
    return () => { ignore = true; };
  }, [activeId, inspectedTurn, events, evidence]);

  // Memory management works even when Ollama is offline.
  useEffect(() => {
    void loadMemories().catch((e) => setMemoryError(errorText(e)));
  }, []);

  useEffect(() => {
    let cancelled = false;
    if (!activeId || !inspectedTurn) {
      setTurnMemoryRefs([]);
      return;
    }
    void invoke<TurnMemoryRef[]>("get_turn_memories", {
      sessionId: activeId,
      turnId: inspectedTurn,
    }).then((items) => {
      if (!cancelled) setTurnMemoryRefs(items);
    }).catch((e) => {
      if (!cancelled) {
        setTurnMemoryRefs([]);
        setMemoryError(errorText(e));
      }
    });
    return () => { cancelled = true; };
  }, [activeId, inspectedTurn, events, memoryRevision]);

  async function newSession() {
    if (busy || memoryBusy) return;
    suggestionRequestRef.current += 1;
    setSuggesting(false);
    setSelectedMemoryIds([]);
    setSelectedPassages([]);
    setPassagePreview(null);
    setSuggestedNotes([]);
    setSuggestedFor("");
    setError("");
    try {
      const session = await invoke<Session>("create_session", { mode, agent });
      await reloadSessions();
      setActiveId(session.id);
      setMessages([]);
      setEvents([]);
      setSelectedTurn(null);
    } catch (e) { setError(errorText(e)); }
  }

  async function openSession(id: string) {
    if (busy || memoryBusy) return;
    suggestionRequestRef.current += 1;
    setSuggesting(false);
    setSelectedMemoryIds([]);
    setSelectedPassages([]);
    setPassagePreview(null);
    setSuggestedNotes([]);
    setSuggestedFor("");
    setError("");
    try {
      await loadSession(id);
      setActiveId(id);
    } catch (e) { setError(errorText(e)); }
  }

  async function submit() {
    const input = prompt.trim();
    if (!activeId || !input || busy || memoryBusy || evidenceBusy) return;
    // Each reference requires explicit user choice for exactly this turn.
    const memoryIds = [...selectedMemoryIds];
    const passageSelections = selectedPassages.map((item) => ({ ...item }));
    suggestionRequestRef.current += 1;
    setSuggesting(false);
    setSelectedMemoryIds([]);
    setSelectedPassages([]);
    setPassagePreview(null);
    setSuggestedNotes([]);
    setSuggestedFor("");
    setActiveMemoryCount(memoryIds.length);
    setActivePassageCount(passageSelections.length);
    setBusy(true);
    setCancelRequested(false);
    setCurrentStage("Preparing model…");
    setLiveOutput("");
    setError("");
    const onProgress = new Channel<TurnProgress>();
    onProgress.onmessage = (update) => {
      if (update.event === "stage_started") {
        setCurrentStage(update.agent.toUpperCase() + " · " + update.phase.replaceAll("_", " "));
        setLiveOutput("");
      } else if (update.event === "delta") {
        setLiveOutput((current) => current + update.text);
      } else if (update.event === "stage_finished") {
        setCurrentStage(update.agent.toUpperCase() + " finished " + update.phase.replaceAll("_", " "));
      }
    };
    try {
      await invoke("send_prompt", {
        sessionId: activeId, input, depth, memoryIds, passageSelections, onProgress,
      });
      setPrompt("");
      await loadSession(activeId);
      await reloadSessions();
      setInspector("contributions");
      setLiveOutput("");
    } catch (e) {
      setError(errorText(e));
      // A stopped model may have written durable partial events; show them.
      try { await loadSession(activeId); } catch { /* Preserve original error. */ }
    } finally {
      setBusy(false);
      setCancelRequested(false);
      setCurrentStage("");
      setActiveMemoryCount(0);
      setActivePassageCount(0);
    }
  }

  async function cancel() {
    if (!busy || cancelRequested) return;
    try {
      const accepted = await invoke<boolean>("cancel_turn");
      if (accepted) {
        setCancelRequested(true);
        setCurrentStage("Cancellation requested. Stopping at the next Ollama stream chunk…");
      }
    } catch (e) { setError(errorText(e)); }
  }

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand"><div className="brand-mark">Φ</div>
          <div><strong>PhiBuddy</strong><span>PHI369 LABS · DESKTOP PREVIEW</span></div>
        </div>
        <h2>Start a workspace</h2>
        <label htmlFor="mode">Mode</label>
        <select id="mode" value={mode} disabled={busy}
          onChange={(e) => setMode(e.target.value as Mode)}>
          {modes.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
        <label htmlFor="agent">Lead agent</label>
        <select id="agent" value={agent} disabled={busy}
          onChange={(e) => setAgent(e.target.value as Agent)}>
          {agents.map((a) => <option key={a} value={a}>{a}</option>)}
        </select>
        <button className="primary wide" onClick={() => void newSession()} disabled={busy}>+ New session</button>
        <div className="section-title">Saved sessions <span>{sessions.length}</span></div>
        <nav className="session-list" aria-label="Saved sessions">
          {sessions.map((s) => (
            <button key={s.id} className={"session " + (s.id === activeId ? "selected" : "")}
              disabled={busy} onClick={() => void openSession(s.id)}>
              <span className="session-badge">{s.mode === "council" ? "3" : initials[s.agent as Agent] ?? "Φ"}</span>
              <span className="session-text"><strong>{s.mode.toUpperCase()}</strong>
                <small>{s.id.slice(0, 8)} · {s.agent}</small></span>
            </button>
          ))}
          {sessions.length === 0 && <p className="hint">No sessions yet. Create one to begin.</p>}
        </nav>
        <div className="sidebar-foot"><span className="dot" /> {status}<small>No telemetry · Local-first</small></div>
      </aside>

      <main className="chat-column">
        <header className="header">
          <div><h1>{active ? active.mode.toUpperCase() + " ROOM" : "WELCOME TO PHIBUDDY"}</h1>
            <p>{active ? "Agent: " + active.agent + " · Session " + active.id.slice(0, 8) : "Three collaborators, one local workshop"}</p>
          </div>
          <div className="crew"><span title="Buddy">B</span><span title="Homie">H</span><span title="Dude">D</span></div>
        </header>
        <section className="chat-log" aria-live="polite" aria-label="Conversation">
          {!active && <div className="welcome"><div className="phi-large">Φ</div><h2>Meet the crew.</h2>
            <p>Buddy reasons. Homie engineers. Dude explores. Create a session to start.</p></div>}
          {active && visibleMessages.length === 0 && <div className="welcome"><h2>Your workshop is ready.</h2>
            <p>Ask a question. Council mode collects independent drafts; Debate adds cross-critiques.</p></div>}
          {visibleMessages.map((m, i) => (
            <article className={"message " + (m.role === "user" ? "from-user" : "from-crew")} key={i}>
              <div className="message-label">{m.role === "user" ? "YOU" : "PHIBUDDY · " + (m.actor || "crew")}</div>
              <div className="message-body">{m.content}</div>
            </article>
          ))}
          {busy && (
            <section className="live-reply" aria-label="Current AI generation">
              <div className="message-label" role="status">{currentStage || "Crew working…"}</div>
              <div className="message-body" aria-live="off">{liveOutput || "Waiting for the first token…"}</div>
              <p className="hint">Live Ollama output · {activeMemoryCount} notes · {activePassageCount} passages attached · Incomplete turns aren't saved as chat replies</p>
            </section>
          )}
        </section>
        <section className="composer">
          {active?.mode === "council" && <label className="depth">Council depth
            <select value={depth} disabled={busy} onChange={(e) => setDepth(e.target.value as Depth)}>
              <option value="quick">Quick · 4 model calls</option>
              <option value="debate">Debate · 7 model calls</option>
            </select>
          </label>}
          <div className="memory-composer">
            <span>{selectedMemoryIds.length > 0
              ? `${selectedMemoryIds.length} note(s) selected for the next turn`
              : "No memory notes attached by default"}</span>
            <button type="button" disabled={busy || memoryBusy}
              onClick={() => setInspector("memory")}>Choose notes</button>
            <button type="button" disabled={busy || memoryBusy || suggesting || !prompt.trim()}
              onClick={() => void suggestFromDraft()}>
              {suggesting ? "Finding…" : "Find related notes"}
            </button>
            {selectedMemoryIds.length > 0 && <button type="button" disabled={busy}
              onClick={() => setSelectedMemoryIds([])}>Clear</button>}
          </div>
          <div className="passage-composer" aria-label="Source passage attachments">
            <span>{selectedPassages.length} selected source passage(s) for next turn only</span>
            <button type="button" disabled={busy || evidenceBusy}
              onClick={() => setInspector("evidence")}>Choose source passage</button>
            {selectedPassages.length > 0 && <button type="button" disabled={busy}
              onClick={() => setSelectedPassages([])}>Clear passages</button>}
          </div>
          {error && <div className="error" role="alert">{error}</div>}
          <label className="sr-only" htmlFor="prompt">Your message</label>
          <textarea id="prompt" rows={4} maxLength={20000} value={prompt}
            disabled={!active || busy} onChange={(e) => {
              suggestionRequestRef.current += 1;
              setSuggesting(false);
              setPrompt(e.target.value);
              setSuggestedNotes([]);
              setSuggestedFor("");
            }}
            onKeyDown={(e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); void submit(); } }}
            placeholder={active ? "Ask the crew anything… (Ctrl+Enter to send)" : "Create a session to begin"} />
          <div className="composer-actions"><small>Human approval required for future execution tools.</small>
            {busy && <button className="cancel" disabled={cancelRequested}
              onClick={() => void cancel()}>{cancelRequested ? "Stopping…" : "Cancel run"}</button>}
            <button className="primary" disabled={!active || !prompt.trim() || busy}
              onClick={() => void submit()}>{busy ? "Generating…" : "Send to crew"}</button></div>
        </section>
      </main>

      <aside className="inspector">
        <div className="inspector-head"><h2>Workshop inspector</h2>
          <small>Agent evidence and user-controlled memory</small></div>
        <div className="tabs" role="tablist" aria-label="Inspector views">
          <button role="tab" aria-selected={inspector === "contributions"}
            className={inspector === "contributions" ? "active" : ""}
            onClick={() => setInspector("contributions")}>Contributions</button>
          <button role="tab" aria-selected={inspector === "events"}
            className={inspector === "events" ? "active" : ""}
            onClick={() => setInspector("events")}>Timeline</button>
          <button role="tab" aria-selected={inspector === "memory"}
            className={inspector === "memory" ? "active" : ""}
            onClick={() => setInspector("memory")}>Memory</button>
          <button role="tab" aria-selected={inspector === "evidence"}
            className={inspector === "evidence" ? "active" : ""}
            onClick={() => setInspector("evidence")}>Evidence</button>
        </div>
        {inspector !== "memory" && inspector !== "evidence" && turns.length > 0 && <label className="turn-select">Turn
          <select value={inspectedTurn ?? ""} onChange={(e) => setSelectedTurn(e.target.value)}>
            {turns.map((t, i) => <option key={t} value={t}>Turn {turns.length - i} · {t.slice(0, 8)}</option>)}
          </select></label>}
        <div className="inspector-scroll">
          {inspector === "memory" ? (
            <section className="memory-workshop">
              <p className="hint">Notes are saved only when you ask. Choose up to five for the <strong>next</strong> message. Search suggestions are local text matches, never an automatic selection.</p>
              <section className="suggestion-results" aria-label="Memory suggestions">
                <h3>Related notes from your draft</h3>
                <p className="hint">Press <strong>Find related notes</strong> below the chat to search the local ledger. Nothing gets sent to Ollama until you explicitly attach a note and send the message.</p>
                {suggestedFor && <p className="suggested-query">Searched draft: {suggestedFor.length > 100 ? suggestedFor.slice(0, 100) + "…" : suggestedFor}</p>}
                {suggestedFor && suggestedNotes.length === 0 && <p className="hint">No matching saved notes.</p>}
                {suggestedNotes.map((match) => (
                  <article className="suggestion-card" key={match.note.id}>
                    <label className="memory-note-select">
                      <input type="checkbox" checked={selectedMemoryIds.includes(match.note.id)}
                        disabled={busy || memoryBusy} onChange={() => toggleMemory(match.note.id)} />
                      <strong>Attach next turn</strong>
                    </label>
                    <div className="memory-note-body">{match.note.content}</div>
                    <p className="suggested-match">Text match score: {match.relevance_score} · Matched: {match.matched_terms.join(", ")}</p>
                    <div className="memory-source-label">
                      Unverified user note · {match.note.source_ref || "No source label"} · {match.note.created_at}
                    </div>
                    {match.review_recommended && <p className="memory-age-warning">
                      Review recommended: {match.age_days === null
                        ? "date could not be determined"
                        : `${match.age_days} days since saving (90+ day threshold)`}.
                      Age does not establish whether a note is true or false.
                    </p>}
                    <small className="memory-id">ID: {match.note.id}</small>
                  </article>
                ))}
              </section>
              {memoryError && <p className="error" role="alert">{memoryError}</p>}
              <label htmlFor="memory-draft">New memory note</label>
              <textarea id="memory-draft" value={memoryDraft} maxLength={4096} rows={3}
                disabled={busy || memoryBusy}
                onChange={(e) => setMemoryDraft(e.target.value)}
                placeholder="A fact, preference, or design decision to save explicitly…" />
              <label htmlFor="memory-source">Source label (optional)</label>
              <input id="memory-source" value={memorySource} maxLength={1024}
                disabled={busy || memoryBusy}
                onChange={(e) => setMemorySource(e.target.value)}
                placeholder="e.g. my project notes · 2026-11" />
              <button className="primary wide" disabled={busy || memoryBusy || !memoryDraft.trim()}
                onClick={() => void saveMemory()}>Save this note</button>
              <div className="memory-search">
                <label htmlFor="memory-query">Find notes</label>
                <input id="memory-query" value={memorySearch} maxLength={1024}
                  disabled={busy || memoryBusy}
                  onChange={(e) => setMemorySearch(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") void refreshMemories(); }}
                  placeholder="Search text (literal match)" />
                <button type="button" disabled={busy || memoryBusy}
                  onClick={() => void refreshMemories()}>Search / show recent</button>
              </div>
              <p className="hint">{memories.length} note(s) shown · {selectedMemoryIds.length}/5 selected · At most 100 results</p>
              {memories.map((note) => (
                <article className="memory-note" key={note.id}>
                  <label className="memory-note-select">
                    <input type="checkbox" checked={selectedMemoryIds.includes(note.id)}
                      disabled={busy || memoryBusy} onChange={() => toggleMemory(note.id)} />
                    <strong>Attach next turn</strong>
                  </label>
                  <div className="memory-note-body">{note.content}</div>
                  <div className="memory-source-label">User note · {note.source_ref || "No source label"} · {note.created_at}</div>
                  <small className="memory-id">ID: {note.id}</small>
                  {pendingDelete === note.id ? (
                    <div className="delete-actions">
                      <span>Delete from active ledger?</span>
                      <button type="button" className="cancel" disabled={busy || memoryBusy}
                        onClick={() => void deleteMemory(note.id)}>Confirm delete</button>
                      <button type="button" disabled={memoryBusy}
                        onClick={() => setPendingDelete(null)}>Keep</button>
                    </div>
                  ) : <button type="button" className="delete-memory" disabled={busy || memoryBusy}
                    onClick={() => setPendingDelete(note.id)}>Delete note</button>}
                </article>
              ))}
              {memories.length === 0 && <p className="hint">No notes found. Save one above to begin.</p>}
            </section>
          ) : inspector === "evidence" ? (
            <section className="evidence-registry" aria-label="External evidence registry">
              <h3>External source snapshots</h3>
              <p className="hint">Only explicit, user-requested HTTPS text fetches from GitHub Raw or the RFC Editor. Retrieved documents are <strong>not</strong> independently fact-checked, and are never automatically passed to the AI.</p>
              {evidenceError && <p className="error" role="alert">{evidenceError}</p>}
              <label htmlFor="evidence-url">Public source URL</label>
              <input id="evidence-url" value={evidenceUrl} maxLength={2048}
                onChange={(e) => setEvidenceUrl(e.target.value)}
                disabled={evidenceBusy || busy}
                placeholder="https://www.rfc-editor.org/rfc/rfc9111.txt" />
              <button className="primary wide" disabled={evidenceBusy || busy || !evidenceUrl.trim()}
                onClick={() => void captureEvidence()}>
                {evidenceBusy ? "Retrieving…" : "Retrieve and save snapshot"}
              </button>
              <p className="hint">No redirects. No login, HTML, arbitrary websites, or agent-initiated fetching. Text is capped at 256 KiB and stored unencrypted in local SQLite. A snapshot is not fact-checked.</p>
              <div className="registry-divider">
                <h3>Saved retrieved sources ({evidence.length})</h3>
                <button type="button" disabled={evidenceBusy || busy}
                  onClick={() => void loadEvidence().catch((e) => setEvidenceError(errorText(e)))}>
                  Refresh
                </button>
              </div>
              {evidence.map((record) => (
                <article key={record.id} className="evidence-source-record">
                  <div className="evidence-source-label">Retrieved · Not fact-checked</div>
                  <div className="evidence-source-url">{record.source_url}</div>
                  <p className="evidence-meta">
                    {record.source_host} · {record.content_type} · {record.retrieved_at} · {record.content.length} text characters
                  </p>
                  <p className="evidence-hash">SHA-256: {record.content_sha256}</p>
                  <button type="button" disabled={evidenceBusy}
                    onClick={() => setEvidenceSelected(evidenceSelected === record.id ? null : record.id)}>
                    {evidenceSelected === record.id ? "Hide saved text" : "Inspect saved text"}
                  </button>
                  {evidenceSelected === record.id && (
                    <div className="passage-picker">
                      <p className="hint">Saved source has {record.content.split("\n").length} lines. The first 200 appear with line numbers; any valid range may be previewed.</p>
                      <pre className="evidence-snapshot">{record.content.split("\n").slice(0, 200).map((line, i) =>
                        `${i + 1} | ${line}`
                      ).join("\n")}</pre>
                      <div className="passage-range">
                        <label>First line
                          <input inputMode="numeric" value={passageStart} disabled={evidenceBusy || busy}
                            onChange={(e) => { setPassageStart(e.target.value); setPassagePreview(null); }} />
                        </label>
                        <label>Last line (inclusive)
                          <input inputMode="numeric" value={passageEnd} disabled={evidenceBusy || busy}
                            onChange={(e) => { setPassageEnd(e.target.value); setPassagePreview(null); }} />
                        </label>
                      </div>
                      <button type="button" disabled={evidenceBusy || busy}
                        onClick={() => void inspectPassage(record.id)}>Preview exact passage</button>
                      {passagePreview?.selection.evidence_id === record.id && (
                        <div className="passage-exact-preview">
                          <strong>Exact stored quote · not independently verified</strong>
                          <pre>{passagePreview.quote}</pre>
                          <p className="evidence-hash">
                            Lines {passagePreview.selection.start_line}-{passagePreview.selection.end_line} · SHA-256 {passagePreview.sha256}
                          </p>
                          <button type="button" disabled={evidenceBusy || busy}
                            onClick={attachPreviewPassage}>Attach this passage to next turn</button>
                        </div>
                      )}
                    </div>
                  )}
                  {evidenceDeleteConfirm === record.id ? (
                    <div className="delete-actions">
                      <span>Delete this local snapshot?</span>
                      <button className="cancel" type="button" disabled={evidenceBusy || busy}
                        onClick={() => void removeEvidence(record.id)}>Confirm delete</button>
                      <button type="button" disabled={evidenceBusy}
                        onClick={() => setEvidenceDeleteConfirm(null)}>Keep</button>
                    </div>
                  ) : <button type="button" className="delete-memory" disabled={evidenceBusy || busy}
                    onClick={() => setEvidenceDeleteConfirm(record.id)}>Delete snapshot</button>}
                </article>
              ))}
              <p className="hint">{selectedPassages.length}/3 user-selected passages queued for next turn. They will not be reused automatically.</p>
              {selectedPassages.map((passage, i) => (
                <div className="queued-passage" key={`${passage.evidence_id}-${passage.start_line}-${passage.end_line}`}>
                  <span>#{i + 1} · {passage.evidence_id.slice(0, 8)} · lines {passage.start_line}-{passage.end_line}</span>
                  <button type="button" disabled={busy}
                    onClick={() => setSelectedPassages((items) => items.filter((x) => x !== passage))}>Remove</button>
                </div>
              ))}
              {evidence.length === 0 && <p className="hint">No retrieved snapshots yet. They remain separate from personal memory.</p>}
              <section className="verified-evidence">
                <h3>Source categories</h3>
                <p className="hint"><strong>Personal notes:</strong> entered by you, never externally verified by this app.</p>
                <p className="hint"><strong>Retrieved sources:</strong> directly fetched text, with URL, timestamp and fingerprint, but no factual validation.</p>
                <p className="hint"><strong>Unverified claims:</strong> statements generated by agents are not fact-checked merely because a source snapshot exists.</p>
              </section>
            </section>
          ) : (
            <>
              {turnMemoryRefs.length > 0 && <section className="turn-sources">
                <h3>Notes attached to this turn</h3>
                <p className="hint">Selected notes are user-provided, unverified context. This inspector proves attachment, not model reliance or factual accuracy.</p>
                {turnMemoryRefs.map((item) => (
                  <article className="source-record" key={item.id}>
                    <div className="memory-source-label">{item.memory
                      ? `Unverified user note · ${item.memory.source_ref || "No source label"}`
                      : "Note no longer in active ledger"}</div>
                    <div className="memory-note-body">{item.memory?.content ?? "(Content unavailable after deletion)"}</div>
                    <small className="memory-id">{item.id}</small>
                  </article>
                ))}
              </section>}
              {turnPassages.length > 0 && <section className="turn-sources">
                <h3>Exact source excerpts attached to this turn</h3>
                <p className="hint">Each anchor came from an explicitly selected stored passage. Quotes are exact snapshot text. AI interpretations are separate, unverified outputs. No claim has been independently fact-checked.</p>
                {turnPassages.map((entry, i) => (
                  <article className="source-record" key={i}>
                    <div className="memory-source-label">Source excerpt (not fact-checked) · Lines {entry.anchor.selection.start_line}-{entry.anchor.selection.end_line}</div>
                    <div className="evidence-source-url">{entry.preview?.source_url ?? "Snapshot deleted, changed, or unavailable"}</div>
                    <p className="evidence-hash">SHA-256 at attachment: {entry.anchor.sha256}</p>
                    {entry.preview ? <pre className="evidence-snapshot">{entry.preview.quote}</pre>
                      : <p className="hint">Original quote unavailable. The event log retains only the source ID, line range and fingerprint.</p>}
                    <small className="memory-id">ID: {entry.anchor.selection.evidence_id}</small>
                  </article>
                ))}
              </section>}
                            <section className="verified-evidence">
                <h3>External evidence status</h3>
                <p className="hint"><strong>Direct quote:</strong> exact line excerpt from the saved snapshot, anchored by ID and SHA-256. <strong>Inference:</strong> Buddy's interpretation, not a source fact. <strong>Verified claim:</strong> unavailable; this version has no independent claim checker. Generated citations are not validated.</p>
              </section>
                            {inspector === "contributions" ? (
            contributions.length ? contributions.map((e) =>
              <article className="evidence" key={e.sequence}>
                <div className="evidence-label">{e.actor ?? "system"} · {e.kind.replaceAll("_", " ")}</div>
                <div className="evidence-body">{e.content}</div>
              </article>
            ) : <p className="hint">Contributions and their exact text will appear after a completed or interrupted turn.</p>
          ) : turnEvents.length ? turnEvents.map((e) =>
            <article className="timeline-row" key={e.sequence}>
              <span className="sequence">#{e.sequence}</span>
              <div><strong>{e.kind.replaceAll("_", " ")}</strong><small>{e.actor ?? "system"} · {e.created_at}</small></div>
            </article>
          ) : <p className="hint">No recorded events for this session yet.</p>}
            </>
          )}
        </div>
        <footer className="inspector-foot">Memory is opt-in per turn. Attachments are audited by ID, not copied to the event log. Deleted notes may still appear in generated answers or backups.</footer>
      </aside>
    </div>
  );
}
