import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Session = { id: string; mode: string; agent: string; created_at: string };
type Message = { role: string; actor: string | null; content: string };
type Event = {
  sequence: number; schema_version: number; session_id: string;
  turn_id: string; kind: string; actor: string | null;
  content: string; created_at: string;
};
type Agent = "buddy" | "homie" | "dude";
type Mode = "solo" | "council" | "workshop";
type Depth = "quick" | "debate";

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
  const [error, setError] = useState("");
  const [status, setStatus] = useState("Checking Ollama…");
  const [inspector, setInspector] = useState<"events" | "contributions">("contributions");
  const [selectedTurn, setSelectedTurn] = useState<string | null>(null);

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
     "workshop_build", "solo_answer", "synthesis"].includes(e.kind)
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

  async function newSession() {
    if (busy) return;
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
    if (busy) return;
    setError("");
    try {
      await loadSession(id);
      setActiveId(id);
    } catch (e) { setError(errorText(e)); }
  }

  async function submit() {
    const input = prompt.trim();
    if (!activeId || !input || busy) return;
    setBusy(true);
    setError("");
    try {
      await invoke("send_prompt", { sessionId: activeId, input, depth });
      setPrompt("");
      await loadSession(activeId);
      await reloadSessions();
      setInspector("contributions");
    } catch (e) {
      setError(errorText(e));
      // Even a failed Council can have durable partial events.
      try { await loadSession(activeId); } catch { /* Preserve original error. */ }
    } finally { setBusy(false); }
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
          {busy && <p className="busy" role="status">Crew working… Ollama requests are sequential. This preview shows complete turns, not token streaming.</p>}
        </section>
        <section className="composer">
          {active?.mode === "council" && <label className="depth">Council depth
            <select value={depth} disabled={busy} onChange={(e) => setDepth(e.target.value as Depth)}>
              <option value="quick">Quick · 4 model calls</option>
              <option value="debate">Debate · 7 model calls</option>
            </select>
          </label>}
          {error && <div className="error" role="alert">{error}</div>}
          <label className="sr-only" htmlFor="prompt">Your message</label>
          <textarea id="prompt" rows={4} maxLength={20000} value={prompt}
            disabled={!active || busy} onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); void submit(); } }}
            placeholder={active ? "Ask the crew anything… (Ctrl+Enter to send)" : "Create a session to begin"} />
          <div className="composer-actions"><small>Human approval required for future execution tools.</small>
            <button className="primary" disabled={!active || !prompt.trim() || busy}
              onClick={() => void submit()}>{busy ? "Working…" : "Send to crew"}</button></div>
        </section>
      </main>

      <aside className="inspector">
        <div className="inspector-head"><h2>Council inspector</h2><small>Actual recorded outputs</small></div>
        <div className="tabs" role="tablist" aria-label="Inspector views">
          <button role="tab" aria-selected={inspector === "contributions"}
            className={inspector === "contributions" ? "active" : ""}
            onClick={() => setInspector("contributions")}>Contributions</button>
          <button role="tab" aria-selected={inspector === "events"}
            className={inspector === "events" ? "active" : ""}
            onClick={() => setInspector("events")}>Timeline</button>
        </div>
        {turns.length > 0 && <label className="turn-select">Turn
          <select value={inspectedTurn ?? ""} onChange={(e) => setSelectedTurn(e.target.value)}>
            {turns.map((t, i) => <option key={t} value={t}>Turn {turns.length - i} · {t.slice(0, 8)}</option>)}
          </select></label>}
        <div className="inspector-scroll">
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
        </div>
        <footer className="inspector-foot">Recorded events are evidence of model output, not proof that its claims are true.</footer>
      </aside>
    </div>
  );
}
