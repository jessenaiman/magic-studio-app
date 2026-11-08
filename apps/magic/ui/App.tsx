import { PreservedWorkspaces } from "./components/RestoredWorkspace";
import { Tooltip } from "./components/Tooltip";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { importDroppedArtwork } from "./artwork-transfer";
import { connectDesktop } from "./desktop";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { executeCommand, toggleWindowSnap, WorkshopData } from "./api";
import { FilmView } from "./views/FilmView";
import { GalleryView } from "./views/GalleryView";
import { HomeView } from "./views/HomeView";
import { MayhemView } from "./views/MayhemView";
import { PreservedStoryboard } from "./views/StoryboardView";

export type ViewMode =
  | "home"
  | "project"
  | "storyboard"
  | "design"
  | "motion"
  | "gallery"
  | "animation"
  | "studio"
  | "mayhem";

const formatClock = (sec: number): string => {
  if (!Number.isFinite(sec) || sec < 0) return "0:00";
  const s = Math.floor(sec);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

export const App: React.FC = () => {
  const [view, setView] = useState<ViewMode>("home");
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [data, setData] = useState<WorkshopData | null>(null);
  const [toastMsg, setToastMsg] = useState<string>("");
  const [busy, setBusy] = useState<boolean>(false);
  const [audioPlaying, setAudioPlaying] = useState<boolean>(false);
  const [audioTime, setAudioTime] = useState<number>(0);
  const [audioDuration, setAudioDuration] = useState<number>(0);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const showToast = useCallback((msg: string) => {
    setToastMsg(msg);
  }, []);

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
      audioRef.current?.pause();
      await executeCommand("project.close");
      await refreshProject();
      setView("home");
    } catch (error) { showToast(String(error)); }
  };

  const launchCraftApp = async (app: string, label: string) => {
    try {
      await executeCommand("workspace.open", { app });
      showToast(`Launched ${label}`);
    } catch (error) {
      showToast(String(error));
    }
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
  }, [connectDesktop]);

  const projectName =
    data?.name ||
    projectPath?.split("/").pop()?.replace(/\.magic$/, "") ||
    "No project";

  const songAudioPath = data?.status?.stages?.find((s) => s.id === "song")?.inputs?.[0]?.path || null;

  useEffect(() => {
    setAudioPlaying(false);
    setAudioTime(0);
    setAudioDuration(0);
  }, [songAudioPath]);

  const toggleTopAudio = () => {
    const el = audioRef.current;
    if (!el || !songAudioPath) return;
    if (el.paused) {
      void el.play().catch((err) => showToast(String(err)));
    } else {
      el.pause();
    }
  };

  return (
    <div
      id="app-shell"
      className="app-shell"
    >
      {/* Top App Bar */}
      <header
        id="app-bar"
        data-tauri-drag-region="deep"
        className="app-bar"
      >
        <div className="app-identity app-identity-row" >
          <Tooltip label={projectPath ? "Return to Song Film Shelf (Home)" : "Magic Studio Home"}>
          <button
            id="brand"
            className="top-brand-btn"
            onClick={() => void closeProject()}
          >
            <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor" className="app-logo">
              <path d="M12 2l2.2 6.3L20.5 10.5l-6.3 2.2L12 19l-2.2-6.3L3.5 10.5l6.3-2.2L12 2zm6.5 12l1 2.8 2.8 1-2.8 1-1 2.8-1-2.8-2.8-1 2.8-1 1-2.8z" />
            </svg>
            {!projectPath && <span className="app-title">Magic Studio</span>}
          </button>
          </Tooltip>

          {projectPath ? (
            <button
              id="tab-project"
              className={`top-project-pill ${view === "project" ? "active" : ""}`}
              onClick={() => setView("project")}
              title={`Project workspace · ${projectName}`}
            >
              <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M2 4h7a3 3 0 0 1 3 3v13a2 2 0 0 0-2-2H2z" />
                <path d="M22 4h-7a3 3 0 0 0-3 3v13a2 2 0 0 1 2-2h8z" />
              </svg>
              <span id="project-name">{projectName}</span>
            </button>
          ) : (
            <span id="project-name" className="muted app-project-name" >
              {projectName}
            </span>
          )}

          <Tooltip label="Snap window to the top-right corner, or back to the center">
          <button
            id="window-snap"
            className="top-icon-ghost"
            onClick={() => void toggleWindowSnap().then((to) => showToast(`Window moved to ${to}`)).catch((e) => showToast(String(e)))}
          >
            ⇱
          </button>
          </Tooltip>
        </div>

        <nav
          id="mode-tabs"
          aria-label="Workspace views"
          className="top-mode-nav"
        >
          {(
            [
              ["storyboard", "Storyboard"],
              ["design", "Lyrics"],
              ["motion", "Motion"],
              ["gallery", "Gallery"],
              ["animation", "Animation"],
              ["studio", "Studio"],
              ["mayhem", "Mayhem"],
            ] as Array<[ViewMode, string]>
          ).filter(([id]) => projectPath || id === "gallery" || id === "mayhem").map(([id, label]) => (
            <button
              key={id}
              id={`tab-${id}`}
              className={`top-mode-tab ${view === id ? "active" : ""}`}
              onClick={() => setView(id)}
            >
              {id === "storyboard" && (
                <Tooltip label={data?.storyboard?.error || `${data?.storyboard?.count ?? 0} unfilled fields; ${data?.storyboard?.preflight?.errors ?? 0} layout errors`}>
                <span
                  aria-label={data?.storyboard?.complete ? "Storyboard complete" : "Storyboard incomplete"}
                  className={`app-storyboard-dot ${data?.storyboard?.complete ? "complete" : data?.storyboard?.status === "review" ? "review" : "incomplete"}`}
                >
                  ●
                </span>
                </Tooltip>
              )}
              {label}
            </button>
          ))}
        </nav>

        <div id="app-bar-tools" className="top-right-tools">
          {songAudioPath && (
            <>
              <audio
                ref={audioRef}
                src={convertFileSrc(songAudioPath)}
                preload="metadata"
                onPlay={() => setAudioPlaying(true)}
                onPause={() => setAudioPlaying(false)}
                onEnded={() => setAudioPlaying(false)}
                onTimeUpdate={(e) => setAudioTime(e.currentTarget.currentTime)}
                onLoadedMetadata={(e) => setAudioDuration(e.currentTarget.duration || 0)}
              />
              <Tooltip label={audioPlaying ? "Pause song preview" : "Play song preview"}>
              <button
                id="app-audio-toggle"
                className={`top-audio-pill ${audioPlaying ? "playing" : ""}`}
                onClick={toggleTopAudio}
              >
                <span className="top-audio-play-ico">{audioPlaying ? "⏸" : "▶"}</span>
                <span className="top-audio-clock">
                  {formatClock(audioTime)} / {formatClock(audioDuration)}
                </span>
              </button>
              </Tooltip>
            </>
          )}

          <div className="top-craft-dock" aria-label="ArtCraft Suite">
            <Tooltip label="Open PhotoCraft">
<button
              id="dock-photocraft"
              className="craft-dock-btn craft-photo"
              onClick={() => void launchCraftApp("photocraft", "PhotoCraft")}
            >
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2.2">
                <rect x="3" y="3" width="18" height="18" rx="3" />
                <circle cx="8.5" cy="8.5" r="1.5" />
                <path d="m21 15-5-5L5 21" />
              </svg>
            </button>
            </Tooltip>
            <Tooltip label="Open VectorCraft">
<button
              id="dock-vectorcraft"
              className="craft-dock-btn craft-vector"
              onClick={() => void launchCraftApp("vectorcraft", "VectorCraft")}
            >
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2.2">
                <circle cx="6" cy="6" r="2.5" />
                <circle cx="18" cy="6" r="2.5" />
                <circle cx="12" cy="18" r="2.5" />
                <path d="M8 7.5 11 16M16 7.5 13 16M8.5 6h7" />
              </svg>
            </button>
            </Tooltip>
            <Tooltip label="Open DesignCraft">
<button
              id="dock-designcraft"
              className="craft-dock-btn craft-design"
              onClick={() => void launchCraftApp("designcraft", "DesignCraft")}
            >
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2.2">
                <path d="M12 19l7-7 3 3-7 7-3-3z" />
                <path d="M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z" />
              </svg>
            </button>
            </Tooltip>
            <Tooltip label="Open EffectCraft">
<button
              id="dock-effectcraft"
              className="craft-dock-btn craft-effect"
              onClick={() => void launchCraftApp("effectcraft", "EffectCraft")}
            >
              <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                <path d="M12 3l2.1 5.4L19.5 10.5l-5.4 2.1L12 18l-2.1-5.4L4.5 10.5l5.4-2.1L12 3z" />
              </svg>
            </button>
            </Tooltip>
            <Tooltip label="Open FilmCraft">
<button
              id="dock-filmcraft"
              className="craft-dock-btn craft-film"
              onClick={() => void launchCraftApp("filmcraft", "FilmCraft")}
            >
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2.2">
                <rect x="3" y="4" width="18" height="16" rx="2" />
                <path d="M7 4v16M17 4v16M3 10h18M3 15h4M17 15h4" />
              </svg>
            </button>
            </Tooltip>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="app-view-host">
        {projectPath && <PreservedWorkspaces key={projectPath} project={projectPath} view={view} onToast={showToast} />}
        {projectPath && <PreservedStoryboard key={`storyboard-${projectPath}`} active={view === "storyboard"} projectPath={projectPath} storyboard={data?.storyboard} onRefresh={() => refreshProject()} onToast={showToast} />}
        {view === "gallery" ? <GalleryView /> : view === "mayhem" ? (
          <MayhemView onToast={showToast} />
        ) : view === "home" || !projectPath ? (
          <HomeView onOpenProject={openProject} onToast={showToast} />
        ) : view === "studio" ? (
          <FilmView projectPath={projectPath} onToast={showToast} />
        ) : null}
      </div>

      {/* Bottom Status Footer */}
      <footer
        id="app-footer"
        className="app-status-footer"
      >
        <span id="footer-status">
          {busy ? "Working…" : toastMsg || (projectPath ? `Saved · ${projectPath}` : "Ready")}
        </span>
        <span className="muted">Tauri v2 · React + Strict TypeScript</span>
      </footer>
    </div>
  );
};

export default App;
