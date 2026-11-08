import React, { useCallback, useEffect, useState } from "react";
import { executeCommand } from "../api";

// The Mayhem evidence surface: reads saved analyzer reports and the logs the runtimes and
// gates already write. Opening this view never runs an analyzer and never polls.

interface LogSource {
  name: string;
  path: string;
  bytes?: number;
  modified?: number;
  tail?: string;
  error?: string;
}

interface Evidence {
  root?: string;
  report?: { error?: string; tools?: unknown[] };
  logs?: LogSource[];
}

export const MayhemView: React.FC<{ onToast: (message: string) => void }> = ({ onToast }) => {
  const [data, setData] = useState<Evidence | null>(null);
  const [readAt, setReadAt] = useState<Date | null>(null);
  const [selected, setSelected] = useState(0);

  const refresh = useCallback(async () => {
    try {
      const evidence = await executeCommand<Evidence>("mayhem.evidence", {});
      setData(evidence);
      setReadAt(new Date());
      setSelected(0);
    } catch (error) {
      onToast(String(error));
    }
  }, [onToast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const sources = data?.logs ?? [];
  const source = sources[selected];
  const age = (modified?: number) => (modified ? new Date(modified * 1000).toLocaleString() : "—");

  return (
    <section id="view-mayhem" style={{ display: "flex", height: "100%", minHeight: 0 }}>
      <aside style={{ width: 280, borderRight: "1px solid var(--border, #2a313c)", padding: 12, overflow: "auto" }}>
        <h2 style={{ marginTop: 0 }}>Mayhem</h2>
        <p className="muted" style={{ fontSize: 11 }}>
          Read {readAt ? readAt.toLocaleTimeString() : "…"} · no background scans or polling
        </p>
        <button id="mayhem-refresh" className="start" onClick={() => void refresh()}>Refresh data</button>
        <h3 className="muted" style={{ fontSize: 12 }}>Sources</h3>
        {sources.map((item, index) => (
          <button
            key={item.path}
            id={`mayhem-source-${index}`}
            className={index === selected ? "start" : "ghost"}
            style={{ display: "block", width: "100%", textAlign: "left", marginBottom: 6 }}
            onClick={() => setSelected(index)}
          >
            {item.name}
            <div className="muted" style={{ fontSize: 10 }}>{item.error ? "not present" : `${item.bytes ?? 0} bytes · ${age(item.modified)}`}</div>
          </button>
        ))}
      </aside>
      <main style={{ flex: 1, minWidth: 0, padding: 12, overflow: "auto" }}>
        <h2 style={{ marginTop: 0 }}>{source?.name ?? "Mayhem"}</h2>
        {data?.report?.error && <p className="muted">Analyzer report · {data.report.error}</p>}
        {source && (
          <>
            <p className="muted" style={{ fontSize: 11 }}>
              {source.path} · {source.error ?? `${source.bytes ?? 0} bytes · last write ${age(source.modified)}`}
            </p>
            <pre id="mayhem-tail" style={{ background: "var(--surface-2, #181c22)", border: "1px solid var(--border, #2a313c)", padding: 10, fontSize: 11, whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
              {source.error ? source.error : source.tail?.trimEnd() || "(empty)"}
            </pre>
          </>
        )}
      </main>
    </section>
  );
};
