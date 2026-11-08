import React, { useCallback, useEffect, useState } from "react";
import { executeCommand } from "../api";
import "../mayhem.css";

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
    <section id="view-mayhem" className="mayhem-layout">
      <aside className="mayhem-sources">
        <h2 className="mayhem-heading">Mayhem</h2>
        <p className="muted mayhem-note" >
          Read {readAt ? readAt.toLocaleTimeString() : "…"} · no background scans or polling
        </p>
        <button id="mayhem-refresh" className="start" onClick={() => void refresh()}>Refresh data</button>
        <h3 className="muted mayhem-subheading" >Sources</h3>
        {sources.map((item, index) => (
          <button
            key={item.path}
            id={`mayhem-source-${index}`}
            className={`mayhem-source ${index === selected ? "start" : "ghost"}`}
            onClick={() => setSelected(index)}
          >
            {item.name}
            <div className="muted mayhem-source-meta" >{item.error ? "not present" : `${item.bytes ?? 0} bytes · ${age(item.modified)}`}</div>
          </button>
        ))}
      </aside>
      <main className="mayhem-main">
        <h2 className="mayhem-heading">{source?.name ?? "Mayhem"}</h2>
        {data?.report?.error && <p className="muted">Analyzer report · {data.report.error}</p>}
        {source && (
          <>
            <p className="muted mayhem-note" >
              {source.path} · {source.error ?? `${source.bytes ?? 0} bytes · last write ${age(source.modified)}`}
            </p>
            <pre id="mayhem-tail" className="mayhem-tail">
              {source.error ? source.error : source.tail?.trimEnd() || "(empty)"}
            </pre>
          </>
        )}
      </main>
    </section>
  );
};
