import { PreservedWorkspaces } from "./components/RestoredWorkspace";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { importDroppedArtwork } from "./artwork-transfer";
import { connectDesktop } from "./desktop";
import React, { useEffect, useRef, useState } from "react";
import { executeCommand, WorkshopData } from "./api";
import { FilmView } from "./views/FilmView";
import { GalleryView } from "./views/GalleryView";
import { HomeView } from "./views/HomeView";
import { PreservedStoryboard } from "./views/StoryboardView";

export type ViewMode =
  | "home"
  | "project"
  | "storyboard"
  | "design"
  | "motion"
  | "gallery"
  | "animation"
  | "studio";

export const App: React.FC = () => {
  const [view, setView] = useState<ViewMode>("home");
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [data, setData] = useState<WorkshopData | null>(null);
  const [toastMsg, setToastMsg] = useState<string>("");
  const [busy, setBusy] = useState<boolean>(false);

  const showToast = (msg: string) => {
    setToastMsg(msg);
  };

  const liveState = useRef({ view, projectPath, busy, toastMsg });
  liveState.current = { view, projectPath, busy, toastMsg };
  const refreshVersion = useRef(0);
  const initialized = useRef(false);
  const transferBusy = useRef(false);
  const refreshProject = async (initial = false) => {
    const version = ++refreshVersion.current;
    const snapshot = await invoke<{ project: string | null; data: WorkshopData | null; initialView: ViewMode }>("session_snapshot");
    if (version !== refreshVersion.current) return;
    const changed = snapshot.project !== liveState.current.projectPath;
    setProjectPath(snapshot.project);
    setData(snapshot.data);
    if (initial && !initialized.current) setView(snapshot.initialView);
    else if (changed) setView(snapshot.project ? "project" : "home");
    initialized.current = true;
  };

  const openProject = async (path: string, initialView: ViewMode = "project") => {
    setBusy(true);
    try {
      await executeCommand("project.open", { path });
      await refreshProject();
      setView(initialView);
      showToast(`Opened ${path.split("/").pop()}`);
    } catch (err) {
      showToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const closeProject = async () => {
    try {
      await executeCommand("project.close");
      await refreshProject();
      setView("home");
    } catch (error) { showToast(String(error)); }
  };

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview().onDragDropEvent(event => {
      if (event.payload.type !== "drop" || disposed) return;
      const project = liveState.current.projectPath;
      if (!project) { showToast("Open a project before dropping artwork"); return; }
      if (transferBusy.current || liveState.current.busy) { showToast("Wait for the current operation before dropping artwork"); return; }
      transferBusy.current = true;
      setBusy(true);
      void importDroppedArtwork(event.payload.paths, project, executeCommand).then(async count => {
        await refreshProject();
        if (disposed || liveState.current.projectPath !== project) return;
        for (const frame of document.querySelectorAll("iframe")) frame.contentWindow?.postMessage({magicAssetsImported:project}, location.origin);
        setView("project");
        showToast(`Imported ${count} artwork file${count === 1 ? "" : "s"} · choose where to place them`);
      }).catch(error => { if (!disposed) showToast(String(error)); })
        .finally(() => { transferBusy.current = false; if (!disposed) setBusy(false); });
    }).then(stop => { if (disposed) stop(); else unlisten = stop; }).catch(error => showToast(`File drop unavailable: ${error}`));
    return () => { disposed = true; unlisten?.(); };
  }, []);

  useEffect(() => {
    let disposed = false;
    let disconnect: (() => void) | undefined;
    void (async () => {
      disconnect = await connectDesktop(
        () => {
          const state = liveState.current;
          return { ok: true, renderer: "tauri", ready: true,
            view: state.projectPath || state.view === "gallery" ? state.view : "home", requestedView: state.view,
            project: state.projectPath, busy: state.busy, status: state.toastMsg };
        },
        next => setView(next as ViewMode),
        () => refreshProject(),
      );
      if (disposed) { disconnect(); return; }
      await refreshProject(true);
      if (!disposed) await invoke("desktop_ready");
    })().catch(error => showToast(String(error)));
    return () => { disposed = true; ++refreshVersion.current; disconnect?.(); };
    // Rebind native listeners when the control module changes during development;
    // an old listener otherwise keeps reporting the previous draft contract.
  }, [connectDesktop]);

  const projectName =
    data?.name ||
    projectPath?.split("/").pop()?.replace(/\.magic$/, "") ||
    "No project";


  return (
    <div
      id="app-shell"
      style={{
        display: "grid",
        gridTemplateRows: "48px 1fr 28px",
        height: "100vh",
        width: "100vw",
        background: "var(--bg, #0e1116)",
        color: "var(--text, #e6edf3)",
        overflow: "hidden",
      }}
    >
      {/* Top App Bar */}
      <header
        id="app-bar"
        data-tauri-drag-region="deep"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 16px",
          background: "var(--surface-2, #181c22)",
          borderBottom: "1px solid var(--border, #2a313c)",
          gap: "12px",
        }}
      >
        <div className="app-identity" style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          <button
            id="brand"
            className="ghost"
            onClick={() => void closeProject()}
            style={{ fontWeight: 700 }}
          >
            ✦ Magic Studio
          </button>
          <span
            id="project-name"
            className="muted"
            style={{ fontSize: "13px", fontWeight: 600 }}
          >
            {projectName}
          </span>
        </div>

        {(
          <nav
            id="mode-tabs"
            aria-label="Workspace views"
            style={{ display: "flex", gap: "6px" }}
          >
            {(
              [
                ["project", "Project"],
                ["storyboard", "Storyboard"],
                ["design", "Lyrics"],
                ["motion", "Motion"],
                ["gallery", "Gallery"],
                ["animation", "Animation"],
                ["studio", "Studio"],
              ] as Array<[ViewMode, string]>
            ).filter(([id]) => projectPath || id === "gallery").map(([id, label]) => (
              <button
                key={id}
                id={`tab-${id}`}
                className={view === id ? "start" : "ghost"}
                onClick={() => setView(id)}
              >
                {id === "storyboard" && <span aria-label={data?.storyboard?.complete ? "Storyboard complete" : "Storyboard incomplete"}
                  title={data?.storyboard?.error || `${data?.storyboard?.count ?? 0} unfilled fields; ${data?.storyboard?.preflight?.errors ?? 0} layout errors`}
                  style={{ color: data?.storyboard?.complete ? "#5bd69b" : data?.storyboard?.status === "review" ? "#eabb62" : "#ed8796", marginRight: 6 }}>●</span>}
                {label}
              </button>
            ))}
          </nav>
        )}
      </header>

      {/* Main Content Area */}
      <div style={{ overflow: "hidden", height: "100%" }}>
        {projectPath && <PreservedWorkspaces key={projectPath} project={projectPath} view={view} onToast={showToast} />}
        {projectPath && <PreservedStoryboard key={`storyboard-${projectPath}`} active={view === "storyboard"} projectPath={projectPath} storyboard={data?.storyboard} onRefresh={() => refreshProject()} onToast={showToast} />}
        {view === "gallery" ? <GalleryView /> : view === "home" || !projectPath ? (
          <HomeView onOpenProject={openProject} onToast={showToast} />
        ) : view === "studio" ? (
          <FilmView projectPath={projectPath} onToast={showToast} />
        ) : null}
      </div>

      {/* Bottom Status Footer */}
      <footer
        id="app-footer"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 16px",
          fontSize: "11px",
          background: "var(--surface-2, #181c22)",
          borderTop: "1px solid var(--border, #2a313c)",
        }}
      >
        <span id="footer-status">
          {busy ? "Working…" : toastMsg || (projectPath ? `Ready · ${projectPath}` : "Ready")}
        </span>
        <span className="muted">Tauri v2 · React + Strict TypeScript</span>
      </footer>
    </div>
  );
};

export default App;
