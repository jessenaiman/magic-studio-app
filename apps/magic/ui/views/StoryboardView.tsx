import React, { useEffect, useRef, useState } from "react";
import { executeCommand, fileUrl, StoryboardData } from "../api";

interface Slot { label: string; kind: string; text?: string; image?: string; hidden?: boolean }
interface Page { page: number; archetype: string; slots: Slot[] }
interface Props { projectPath: string; storyboard?: StoryboardData; onRefresh: () => Promise<void>; onToast: (message: string) => void }
const colors: Record<string, string> = { incomplete: "#ed8796", review: "#eabb62", complete: "#5bd69b" };
const panel: React.CSSProperties = { background: "var(--surface-2, #181c22)", borderRadius: 8, padding: 14, overflow: "auto", minHeight: 0 };

export const StoryboardView: React.FC<Props> = ({ projectPath, storyboard, onRefresh, onToast }) => {
  const [pages, setPages] = useState<Page[]>([]);
  const [check, setCheck] = useState<StoryboardData>(storyboard || {});
  const [pageIndex, setPageIndex] = useState(0);
  const [preview, setPreview] = useState("");
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const request = useRef(0);
  const folder = projectPath.replace(/[/\\][^/\\]+$/, "");
  const docPath = `${folder}/storyboard.designcraft`;
  const page = pages[Math.min(pageIndex, Math.max(0, pages.length - 1))];
  const report = check.pages?.find(item => item.page === page?.page);
  const title = (item: Page) => item.slots.find(slot => /^(cover_title|world_heading|char_heading|scene_title|scene_id)$/.test(slot.label))?.text || `${item.archetype} ${item.page + 1}`;

  const load = async () => {
    const current = ++request.current;
    setError("");
    try {
      const data = await executeCommand<{ path: string; pages: Page[] }>("storyboard.pages");
      const completion = await executeCommand<StoryboardData>("storyboard.check");
      if (current !== request.current) return;
      setPages(data.pages); setCheck(completion);
      setPageIndex(index => Math.min(index, Math.max(0, data.pages.length - 1)));
    } catch (cause) {
      if (current === request.current) { setPages([]); setPreview(""); setError(String(cause)); }
    }
  };
  useEffect(() => { setPageIndex(0); void load(); return () => { ++request.current; }; }, [projectPath]);
  useEffect(() => {
    let cancelled = false;
    setPreview("");
    if (page) {
      const out = `${folder}/storyboard/.preview/designcraft-${page.page}.png`;
      void executeCommand("storyboard.thumbnail", { page: page.page, out, scale: 0.7 })
        .then(() => { if (!cancelled) setPreview(`${fileUrl(out)}?v=${Date.now()}`); })
        .catch(cause => { if (!cancelled) setError(String(cause)); });
    }
    return () => { cancelled = true; };
  }, [page, projectPath]);

  const action = async (name: string, run: () => Promise<unknown>, refresh = false) => {
    setBusy(name); setError("");
    try { await run(); if (refresh) { await load(); await onRefresh(); } }
    catch (cause) { setError(String(cause)); onToast(String(cause)); }
    finally { setBusy(""); }
  };

  return <div id="view-storyboard" style={{ display: "grid", gridTemplateColumns: "210px minmax(0,1fr) 260px", gap: 12, padding: 12, height: "100%", minHeight: 0 }}>
    <aside style={panel}>
      <strong>Storyboard</strong>
      <p className="muted">DesignCraft · {pages.length} pages</p>
      <button id="storyboard-open-editor" className="start" disabled={!!busy || !pages.length}
        onClick={() => void action("Opening DesignCraft…", async () => {
          const result = await executeCommand<{ control: number }>("storyboard.open", { control: 7979 });
          onToast(`DesignCraft connected on port ${result.control}`);
        })}>Open in DesignCraft</button>
      <button id="storyboard-refresh" className="ghost" disabled={!!busy}
        onClick={() => void action("Refreshing…", load)}>Refresh saved document</button>
      <nav aria-label="Storyboard pages" style={{ display: "grid", gap: 6, marginTop: 16 }}>
        {pages.map((item, index) => {
          const status = check.pages?.find(p => p.page === item.page)?.status || "incomplete";
          return <button key={item.page} id={`sb-nav-${item.page + 1}`} className={page?.page === item.page ? "start" : "ghost"}
            aria-current={page?.page === item.page ? "page" : undefined} onClick={() => setPageIndex(index)} style={{ textAlign: "left" }}>
            <span title={status} aria-label={status} style={{ color: colors[status] }}>●</span> {item.page + 1}. {title(item)}
          </button>;
        })}
      </nav>
      <button id="storyboard-export" className="ghost" disabled={!!busy || !pages.length}
        onClick={() => void action("Exporting PDF…", async () => {
          await executeCommand("storyboard.export", { pdf: `${folder}/storyboard.pdf` }); onToast("Saved storyboard.pdf");
        })}>Export PDF</button>
    </aside>
    <main style={{ ...panel, display: "flex", flexDirection: "column", background: "var(--surface-1, #12151a)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
        <button id="sb-prev" className="ghost" disabled={pageIndex === 0} onClick={() => setPageIndex(i => i - 1)}>←</button>
        <strong>{page ? title(page) : "DesignCraft storyboard"}</strong>
        <button id="sb-next" className="ghost" disabled={!pages.length || pageIndex >= pages.length - 1} onClick={() => setPageIndex(i => i + 1)}>→</button>
      </div>
      <div id="sb-cards" style={{ flex: 1, minHeight: 200, display: "grid", placeItems: "center", overflow: "auto" }}>
        {preview ? <img src={preview} alt={page ? title(page) : "Storyboard page"} style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }} />
          : <p className="muted">{pages.length ? "Rendering saved DesignCraft page…" : "No readable DesignCraft storyboard yet."}</p>}
      </div>
      {!!error && <p role="alert" style={{ color: "#ed8796", overflowWrap: "anywhere" }}>{error}</p>}
      <p role="status" className="muted">{busy || "Save in DesignCraft, then refresh here. You can move freely between all views."}</p>
      {!pages.length && <button id="storyboard-new" className="start" disabled={!!busy}
        onClick={() => void action("Creating storyboard…", () => executeCommand("storyboard.new", { path: docPath, overwrite: false }), true)}>Create from the DesignCraft template</button>}
    </main>
    <aside style={panel}>
      <strong><span style={{ color: colors[check.status || "incomplete"] }}>●</span> {check.complete ? "Storyboard complete" : "Storyboard in progress"}</strong>
      <p className="muted">{check.count ?? 0} unfilled fields · {check.preflight?.errors ?? 0} layout errors</p>
      <p>A render can be a draft. The video is not complete until its storyboard is complete.</p>
      <h3 style={{ fontSize: 14 }}>This page</h3>
      {!report?.unfilled.length && !report?.issues?.length && page && <p>All fields filled; no layout issues on this page.</p>}
      {report?.unfilled.map(item => <details key={item.label} style={{ marginBottom: 8 }}>
        <summary style={{ color: "#ed8796", cursor: "pointer" }}>{item.label.replace(/_/g, " ")}</summary>
        <p className="muted">{item.text}</p><code style={{ overflowWrap: "anywhere", fontSize: 11 }}>{item.fix}</code>
      </details>)}
      {report?.issues?.map((issue, i) => <p key={i} style={{ color: "#eabb62", fontSize: 12 }}>{issue.message}</p>)}
      <details style={{ marginTop: 18 }}><summary>CLI and live control</summary>
        <pre style={{ whiteSpace: "pre-wrap", fontSize: 11 }}>magic-cli --project "{projectPath}" storyboard check{"\n\n"}magic-cli --project "{projectPath}" storyboard open --control 7979{"\n\n"}designcraft-cli app --port 7979 --method document.inspect{"\n\n"}designcraft-cli mcp --connect 7979</pre>
      </details>
    </aside>
  </div>;
};
