import { useEffect, useRef, useState } from "react";
import SessionList from "./components/SessionList";
import EventFeed from "./components/EventFeed";

export type Session = {
  id: string;
  started_at: number;
  event_count: number;
};

export type HookEvent = {
  id: string;
  event_type: string;
  payload: unknown;
  ts: number;
};

const styles: Record<string, React.CSSProperties> = {
  layout: { display: "flex", height: "100vh", overflow: "hidden" },
  sidebar: {
    width: 280,
    borderRight: "1px solid #222",
    overflowY: "auto",
    padding: 12,
  },
  main: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    overflow: "hidden",
  },
  header: {
    padding: "12px 16px",
    borderBottom: "1px solid #222",
    display: "flex",
    alignItems: "center",
    gap: 10,
  },
  dot: { width: 8, height: 8, borderRadius: "50%", background: "#22c55e" },
  title: { fontSize: 14, fontWeight: 600, letterSpacing: "0.05em" },
};

export default function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [liveEvents, setLiveEvents] = useState<HookEvent[]>([]);
  const esRef = useRef<EventSource | null>(null);

  useEffect(() => {
    fetch("/api/sessions?limit=50")
      .then((r) => r.json())
      .then(setSessions)
      .catch(console.error);
  }, []);

  // Subscribe to the live SSE stream.
  useEffect(() => {
    const es = new EventSource("/events");
    esRef.current = es;
    es.onmessage = (e) => {
      try {
        const payload = JSON.parse(e.data) as Record<string, unknown>;
        const ev: HookEvent = {
          id: crypto.randomUUID(),
          event_type: (payload.hook_event_name as string) ?? "unknown",
          payload,
          ts: Date.now(),
        };
        setLiveEvents((prev) => [ev, ...prev].slice(0, 500));

        // Bump session list when a new session_id appears.
        const sid = payload.session_id as string | undefined;
        if (sid) {
          setSessions((prev) => {
            const existing = prev.find((s) => s.id === sid);
            if (existing) {
              return prev.map((s) =>
                s.id === sid ? { ...s, event_count: s.event_count + 1 } : s,
              );
            }
            return [
              { id: sid, started_at: Date.now(), event_count: 1 },
              ...prev,
            ];
          });
        }
      } catch (_) {}
    };
    return () => es.close();
  }, []);

  return (
    <div style={styles.layout}>
      <aside style={styles.sidebar}>
        <SessionList
          sessions={sessions}
          selectedId={selectedId}
          onSelect={setSelectedId}
        />
      </aside>
      <div style={styles.main}>
        <header style={styles.header}>
          <span style={styles.dot} />
          <span style={styles.title}>claude-guardian</span>
        </header>
        <EventFeed sessionId={selectedId} liveEvents={liveEvents} />
      </div>
    </div>
  );
}
