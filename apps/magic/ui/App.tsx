import { invoke } from "@tauri-apps/api/core";
import React, { useEffect, useState } from "react";
import { executeCommand, pickFile, WorkshopData } from "./api";
import { AnimationView } from "./views/AnimationView";
import { FilmView } from "./views/FilmView";
import { HomeView } from "./views/HomeView";
import { LyricsView } from "./views/LyricsView";
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

  const refreshProject = async (pathOverride?: string) => {
    const activePath = pathOverride ?? projectPath;
    if (!activePath) return;
    try {
      const status = await executeCommand<WorkshopData>("workshop.status", {
        project: activePath,
      });
      setData(status);
    } catch (err) {
      showToast(String(err));
    }
  };

  const openProject = async (path: string, initialView: ViewMode = "project") => {
    setBusy(true);
    try {
      await executeCommand("project.open", { path });
      setProjectPath(path);
      const status = await executeCommand<WorkshopData>("workshop.status", {
        project: path,
      });
      setData(status);
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
    } catch {
      // ignore if already closed
    }
    setProjectPath(null);
    setData(null);
    setView("home");
  };

  useEffect(() => {
    void (async () => {
      try {
        const current = await invoke<string | null>("current_project");
        if (current) {
          await openProject(current, "project");
        }
      } catch {
        // start at Home when no project is open on startup
      }
    })();
  }, []);

  const songStage = data?.status?.stages?.find((s) => s.id === "song");
  const transcriptStage = data?.status?.stages?.find(
    (s) => s.id === "transcript",
  );
  const audioInput = songStage?.inputs?.[0];
  const assets = data?.assets?.assets || [];
  const projectName =
    data?.name ||
    projectPath?.split("/").pop()?.replace(/\.magic$/, "") ||
    "No project";

  const handleSetAudio = async () => {
    const selected = await pickFile({
      title: "Choose Song Audio",
      extensions: ["wav", "mp3", "flac", "m4a", "aac", "ogg"],
    });
    if (typeof selected !== "string" || !selected) return;
    setBusy(true);
    try {
      await executeCommand("workshop.set_audio", { path: selected });
      await refreshProject();
      showToast("Linked song audio");
    } catch (err) {
      showToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleTranscribe = async () => {
    setBusy(true);
    try {
      await executeCommand("audio.analyze", { analyze_beats: false });
      await refreshProject();
      showToast("Transcription complete");
    } catch (err) {
      showToast(String(err));
    } finally {
      setBusy(false);
    }
  };

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
                {label}
              </button>
            ))}
          </nav>
        )}
      </header>

      {/* Main Content Area */}
      <div style={{ overflow: "hidden", height: "100%" }}>
        {view === "home" || !projectPath ? (
          <HomeView onOpenProject={openProject} onToast={showToast} />
        ) : view === "storyboard" ? (
          <StoryboardView projectPath={projectPath} onToast={showToast} />
        ) : view === "design" ? (
          <LyricsView
            projectPath={projectPath}
            audioPath={audioInput?.path}
            onToast={showToast}
          />
        ) : view === "animation" ? (
          <AnimationView
            projectPath={projectPath}
            assets={assets}
            onRefresh={() => refreshProject()}
            onToast={showToast}
          />
        ) : view === "studio" ? (
          <FilmView projectPath={projectPath} onToast={showToast} />
        ) : (
          /* Project Overview: Craft Project Files & Song Setup */
          <div
            id="view-project"
            style={{
              display: "grid",
              gridTemplateColumns: "340px 1fr",
              gap: "16px",
              height: "100%",
              padding: "16px",
            }}
          >
            <aside
              id="blocks"
              style={{
                background: "var(--surface-2, #181c22)",
                borderRadius: "8px",
                padding: "14px",
                display: "flex",
                flexDirection: "column",
                gap: "14px",
                overflowY: "auto",
              }}
            >
              <section
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: "8px",
                  paddingBottom: "12px",
                  borderBottom: "1px solid var(--border, #2a313c)",
                }}
              >
                <strong>1. Song & Audio</strong>
                <span className="muted" style={{ fontSize: "12px" }}>
                  {audioInput?.name || "No song audio linked yet"}
                </span>
                <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
                  <button
                    id="song-upload"
                    className="start"
                    disabled={busy}
                    onClick={handleSetAudio}
                  >
                    {audioInput ? "Change Audio…" : "Link Audio…"}
                  </button>
                  {audioInput && (
                    <button
                      id="song-transcribe"
                      className="ghost"
                      disabled={busy}
                      onClick={handleTranscribe}
                    >
                      Transcribe Audio
                    </button>
                  )}
                </div>
                {transcriptStage?.why && (
                  <span className="muted" style={{ fontSize: "11px" }}>
                    {transcriptStage.why}
                  </span>
                )}
              </section>

              <section
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: "8px",
                  paddingBottom: "12px",
                  borderBottom: "1px solid var(--border, #2a313c)",
                }}
              >
                <strong>2. Linked Craft Project Files (.magic)</strong>
                <button
                  className="ghost"
                  style={{ textAlign: "left" }}
                  onClick={() => setView("storyboard")}
                >
                  Storyboard: storyboard.designcraft
                </button>
                <button
                  className="ghost"
                  style={{ textAlign: "left" }}
                  onClick={() => setView("design")}
                >
                  EffectCraft Lyrics: lettering/lyrics.ecproj
                </button>
                <button
                  className="ghost"
                  style={{ textAlign: "left" }}
                  onClick={() => setView("animation")}
                >
                  PhotoCraft & EffectCraft Assets ({assets.length})
                </button>
                <button
                  className="ghost"
                  style={{ textAlign: "left" }}
                  onClick={() => setView("studio")}
                >
                  FilmCraft Timeline (.fcproj)
                </button>
              </section>
            </aside>

            <main style={{ height: "100%", overflow: "hidden" }}>
              <StoryboardView projectPath={projectPath} onToast={showToast} />
            </main>
          </div>
        )}
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
