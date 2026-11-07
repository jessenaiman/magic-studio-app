import { useEffect, useRef, useState } from "react";
import { executeCommand } from "../api";
import { ExportReport, exportIconState } from "../export-state";
import "../export-button.css";

function editorDrafts() {
  return [...document.querySelectorAll("iframe")].some(frame => {
    try {
      const workspace = frame.contentWindow as unknown as {acFeedback?: {snapshot: () => {authoring?: {draftDesigns?: number; draftStyleFields?: number}}}};
      const draft = workspace?.acFeedback?.snapshot().authoring;
      return !!(draft?.draftDesigns || draft?.draftStyleFields);
    } catch { return false; }
  });
}

export function FilmExportButton({project, options, disabled, onBusy, onToast}: {
  project: string; options: Record<string, unknown>; disabled: boolean;
  onBusy: (busy: boolean) => void; onToast: (message: string) => void;
}) {
  const [report, setReport] = useState<ExportReport | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState("");
  const [drafts, setDrafts] = useState(false);
  const alive = useRef(true), inFlight = useRef(false), polling = useRef(false), revision = useRef(0);
  const optionsKey = JSON.stringify(options);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    const version = ++revision.current;
    setReport(null); setError("");
    const refresh = async () => {
      setDrafts(editorDrafts());
      if (polling.current || inFlight.current) return;
      polling.current = true;
      try {
        const value = await executeCommand<ExportReport>("video.film", {...JSON.parse(optionsKey), _expected_project:project});
        if (alive.current && revision.current === version) setReport(value);
      } catch { if (alive.current && revision.current === version) setReport(null); }
      finally { polling.current = false; }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 3000);
    return () => { ++revision.current; window.clearInterval(timer); };
  }, [project, optionsKey]);
  const status = exportIconState(report, running, error, drafts);
  const label = status === "running" ? "Rendering final video" : status === "ready" ? "Open validated export" : status === "error" ? "Export failed · click to retry" : "Render final video";
  const activate = async () => {
    if (inFlight.current) return;
    if (editorDrafts()) { onToast("Save the editor changes before exporting the final video"); return; }
    if (status === "ready" && report?.export?.path) {
      try { await executeCommand("video.open_export", {...JSON.parse(optionsKey), _expected_project:project}); } catch (failure) { onToast(String(failure)); }
      return;
    }
    const version = revision.current;
    inFlight.current = true; setRunning(true); setError(""); onBusy(true);
    try {
      await executeCommand("video.export", {...JSON.parse(optionsKey), _expected_project:project});
      const checked = await executeCommand<ExportReport>("video.film", {...JSON.parse(optionsKey), _expected_project:project});
      if (exportIconState(checked, false, "", false) !== "ready") throw new Error(checked.validation?.reason || checked.validation?.error || "Export finished but output validation did not pass");
      if (alive.current && revision.current === version) { setReport(checked); onToast("Export ready · click the green icon to open the video"); }
    } catch (failure) {
      if (alive.current && revision.current === version) { setError(String(failure)); onToast(String(failure)); }
    } finally {
      inFlight.current = false;
      if (alive.current) { setRunning(false); onBusy(false); }
    }
  };
  return <button id="film-export" className={`ghost film-export-icon ${status}`} disabled={disabled || running}
    aria-label={label} aria-busy={running} title={error || label} data-export-state={status} onClick={() => void activate()}>
    {status === "running" ? <span className="film-export-spinner" aria-hidden="true" /> :
      <svg aria-hidden="true" viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="1.8">
        {status === "ready" ? <><path d="m5 12 4 4L19 6"/><path d="M20 14v6H4V4h10"/></> : status === "error" ? <><path d="m12 3 10 18H2Z"/><path d="M12 9v5m0 3v1"/></> : <><path d="M4 14v6h16v-6M12 16V3m-5 5 5-5 5 5"/></>}
      </svg>}
  </button>;
}
