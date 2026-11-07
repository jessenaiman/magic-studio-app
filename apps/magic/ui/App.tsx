import { PreservedWorkspaces } from "./components/RestoredWorkspace";
import { invoke } from "@tauri-apps/api/core";
import { connectDesktop } from "./desktop";
import React, { useEffect, useRef, useState } from "react";
import { executeCommand, WorkshopData } from "./api";
import { FilmView } from "./views/FilmView";
import { HomeView } from "./views/HomeView";
import { StoryboardView } from "./views/StoryboardView";

export type ViewMode =
  | "home"
  | "project"
  | "storyboard"
  | "design"
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
  const refreshProject = async (initial = false) => {
    const version = ++refreshVersion.current;
    const snapshot = await invoke<{ project: string | null; data: WorkshopData | null; initialView: ViewMode }>("session_snapshot");
    if (version !== refreshVersion.current) return;
    const changed = snapshot.project !== liveState.current.projectPath;
    setProjectPath(snapshot.project);
    setData(snapshot.data);
    if (initial) setView(snapshot.initialView);
    else if (changed) setView(snapshot.project ? "project" : "home");
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
    let disconnect: (() => void) | undefined;
    void (async () => {
      disconnect = await connectDesktop(
        () => {
          const state = liveState.current;
          return { ok: true, renderer: "tauri", ready: true,
            view: state.projectPath ? state.view : "home", requestedView: state.view,
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
  }, []);

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
        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
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

        {projectPath && (
          <nav
            id="mode-tabs"
            aria-label="Workspace views"
            style={{ display: "flex", gap: "6px" }}
          >
            {(
              [
                ["project", "Project"],
                ["storyboard", "Storyboard"],
                ["design", "Lyrics (.ecproj)"],
                ["animation", "Animation (.pcraft / .ecproj)"],
                ["studio", "Studio (.fcproj)"],
              ] as Array<[ViewMode, string]>
            ).map(([id, label]) => (
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
        {view === "home" || !projectPath ? (
          <HomeView onOpenProject={openProject} onToast={showToast} />
        ) : view === "storyboard" ? (
          <StoryboardView
            projectPath={projectPath}
            storyboard={data?.storyboard}
            onRefresh={() => refreshProject()}
            onToast={showToast}
          />
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
