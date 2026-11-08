import React, { useCallback, useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { executeCommand, pickFile } from "../api";
import type { ViewMode } from "../App";
import "../home.css";

interface QuickAction {
  label: string;
  view?: string;
  open_block?: string;
  enabled?: boolean;
}

interface Readiness {
  next_step?: string;
  audio_available?: boolean;
  transcript_valid?: boolean;
  editor_ready?: boolean;
  quick_actions?: QuickAction[];
}

interface CatalogProject {
  name: string;
  path: string;
  folder?: string;
  exists?: boolean;
  thumb?: string;
  open_state?: "attached" | "detached" | "not_observed_open" | null;
  readiness?: Readiness;
}

interface CatalogResponse {
  projects?: CatalogProject[];
}

interface HomeViewProps {
  onOpenProject: (path: string, initialView?: ViewMode) => Promise<void>;
  onToast: (msg: string) => void;
}

export const HomeView: React.FC<HomeViewProps> = ({ onOpenProject, onToast }) => {
  const [projects, setProjects] = useState<CatalogProject[]>([]);
  const [filter, setFilter] = useState<"all" | "production" | "render" | "character">("all");
  const [search, setSearch] = useState("");
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [newAudio, setNewAudio] = useState("");
  const [newRoot, setNewRoot] = useState("");
  const [busy, setBusy] = useState(false);
  const [audioDropStem, setAudioDropStem] = useState<string | null>(null);

  const loadCatalog = useCallback(async () => {
    try {
      const cat = await executeCommand<CatalogResponse>("project.catalog");
      setProjects(cat.projects || []);
      const rootInfo = await executeCommand<{ root?: string }>("project.root");
      if (rootInfo.root) setNewRoot(rootInfo.root);
    } catch (err) {
      onToast(String(err));
    }
  }, [onToast]);

  useEffect(() => {
    void loadCatalog();
  }, [loadCatalog]);

  const handleBrowseProject = async () => {
    const selected = await pickFile({
      title: "Open Magic Studio Project",
      extensions: ["magic"],
    });
    if (typeof selected === "string" && selected) {
      await onOpenProject(selected);
    }
  };

  const handlePickAudio = async () => {
    const selected = await pickFile({
      title: "Choose Song Audio",
      extensions: ["wav", "mp3", "flac", "m4a", "aac", "ogg"],
    });
    if (typeof selected === "string" && selected) {
      setNewAudio(selected);
      const stem = selected
        .split("/")
        .pop()
        ?.replace(/\.[^.]+$/, "")
        .replace(/[._-]+/g, " ")
        .trim();
      if (stem) {
        setNewName(stem);
        setAudioDropStem(stem);
      }
      setCreating(true);
    }
  };

  const handlePickRoot = async () => {
    const selected = await pickFile({
      title: "Choose Project Folder",
      directory: true,
    });
    if (typeof selected === "string" && selected) {
      setNewRoot(selected);
    }
  };

  const handleRevealProject = async (path: string) => {
    try {
      await revealItemInDir(path);
    } catch (err) {
      onToast(String(err));
    }
  };

  const handleForgetProject = async (path: string, name: string) => {
    try {
      await executeCommand("project.catalog", { forget: path });
      await loadCatalog();
      onToast(`Removed ${name} from recent projects`);
    } catch (err) {
      onToast(String(err));
    }
  };

  const handleCreateProject = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName.trim()) {
      onToast("Enter a project name first");
      return;
    }
    setBusy(true);
    try {
      // The engine makes <root>/<slug>/<name>.magic and refuses a missing folder.
      const created = await executeCommand<{ project: string }>("project.new", {
        name: newName.trim(),
        root: newRoot,
      });
      if (newAudio) {
        await executeCommand("workshop.set_audio", { path: newAudio });
      }
      setCreating(false);
      setNewAudio("");
      setAudioDropStem(null);
      await onOpenProject(created.project);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    const list = projects.filter((p) => {
      const match =
        q === "" ||
        p.name.toLowerCase().includes(q) ||
        (p.path || "").toLowerCase().includes(q);
      if (!match) return false;
      switch (filter) {
        case "production":
          return !!(
            p.readiness?.audio_available || p.readiness?.transcript_valid
          );
        case "render":
          return !!p.readiness?.editor_ready;
        case "character":
          return /character|rig/i.test(p.name || "") || /character|rig/i.test(p.path || "");
        default:
          return true;
      }
    });
    return { list, allCount: projects.length };
  }, [projects, filter, search]);

  const hashString = (str: string): number => {
    let h = 0;
    for (let i = 0; i < str.length; i++) {
      h = ((h << 5) - h + str.charCodeAt(i)) | 0;
    }
    return h;
  };

  const waveformSvg = useCallback(
    (name: string) => {
      const seed = hashString(name);
      const bars = 24;
      const svgW = 400;
      const svgH = 170;
      const gap = 6;
      const barW = (svgW - gap * (bars - 1)) / bars;
      const maxH = svgH * 0.6;
      const parts: string[] = [];
      for (let i = 0; i < bars; i++) {
        const h = 6 + Math.abs(Math.sin(seed * 0.1 + i * 0.7) * (maxH - 6));
        const x = i * (barW + gap);
        const y = (svgH - h) / 2;
        const color = i % 3 === 0 ? "#5bd69b" : i % 3 === 1 ? "#eabb62" : "#7fe0b3";
        parts.push(
          `<rect x="${Math.round(x * 100) / 100}" y="${Math.round(y * 100) / 100}" width="${Math.round(barW * 100) / 100}" height="${Math.round(h * 100) / 100}" fill="${color}" rx="2"/>`,
        );
      }
      return `<svg width="${svgW}" height="${svgH}" viewBox="0 0 ${svgW} ${svgH}" xmlns="http://www.w3.org/2000/svg" style="display:block;width:100%;height:100%">${parts.join("")}</svg>`;
    },
    [],
  );

  const statusPills = (p: CatalogProject) => {
    const r = p.readiness || {};
    const pills: { kind: "song" | "lyrics" | "studio" | "none"; label: string }[] = [];
    if (r.audio_available) pills.push({ kind: "song", label: "Song · audio" });
    if (r.transcript_valid) pills.push({ kind: "lyrics", label: "Lyrics · transcript" });
    if (r.editor_ready) pills.push({ kind: "studio", label: "Studio · editor" });
    if (pills.length === 0) pills.push({ kind: "none", label: "Empty" });
    return pills;
  };

  return (
    <section id="view-home" className="view active">
      <div className="home-shelf">
        {/* Top toolbar */}
        <div className="home-toolbar">
          <div className="home-brand">
            <svg className="logo" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
            </svg>
            <h2>Song Film Shelf</h2>
          </div>
          <div className="home-filters">
            <button
              id="home-filter-all"
              className={`home-filter-pill ${filter === "all" ? "active" : ""}`}
              onClick={() => setFilter("all")}
            >
              All Songs ({filtered.allCount})
            </button>
            <button
              id="home-filter-production"
              className={`home-filter-pill ${filter === "production" ? "active" : ""}`}
              onClick={() => setFilter("production")}
            >
              In Production
            </button>
            <button
              id="home-filter-render"
              className={`home-filter-pill ${filter === "render" ? "active" : ""}`}
              onClick={() => setFilter("render")}
            >
              Ready to Render
            </button>
            <button
              id="home-filter-character"
              className={`home-filter-pill ${filter === "character" ? "active" : ""}`}
              onClick={() => setFilter("character")}
            >
              Character Rigs
            </button>
          </div>
          <div className="home-search">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <circle cx="11" cy="11" r="8" />
              <path d="M21 21l-4.3-4.3" />
            </svg>
            <input
              id="home-search-input"
              type="text"
              placeholder="Filter songs or characters…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          <div className="home-actions">
            <button id="home-new" onClick={() => setCreating((v) => !v)}>
              + New Song Project
            </button>
            <button id="home-open" onClick={handleBrowseProject}>
              Open .magic
            </button>
          </div>
        </div>

        {/* New project form (inline drawer / modal) */}
        {creating && (
          <form id="new-form" onSubmit={handleCreateProject} aria-label="New Song Project">
            <h3 className="home-section-title">New Song Project</h3>
            <div className="new-form-field">
              <label htmlFor="new-name">Song / Project Name</label>
              <input
                id="new-name"
                autoFocus
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder="e.g. Storybook Moves"
                required
              />
            </div>
            <div className="new-form-field">
              <label htmlFor="new-audio">Audio Recording (optional)</label>
              <div className="new-form-row">
                <input
                  id="new-audio"
                  value={newAudio}
                  onChange={(e) => setNewAudio(e.target.value)}
                  placeholder="/path/to/song.wav"
                />
                <button id="new-audio-pick" type="button" onClick={handlePickAudio}>
                  Browse…
                </button>
              </div>
            </div>
            <div className="new-form-field">
              <label htmlFor="new-root">Projects Root Folder</label>
              <div className="new-form-row">
                <input
                  id="new-root"
                  value={newRoot}
                  onChange={(e) => setNewRoot(e.target.value)}
                />
                <button id="new-root-pick" type="button" onClick={handlePickRoot}>
                  Folder…
                </button>
              </div>
            </div>
            <div className="new-form-actions">
              <button
                id="new-cancel"
                type="button"
                onClick={() => {
                  setCreating(false);
                  setAudioDropStem(null);
                }}
              >
                Cancel
              </button>
              <button id="new-create" type="submit" className="open" disabled={busy}>
                {busy ? "Creating…" : "Create Project"}
              </button>
            </div>
          </form>
        )}

        {/* Projects Grid — Top-Left Slot is always the "+ New Project" card */}
        <h3 className="home-section-title">Projects &amp; Recent Productions</h3>
        <div id="recent">
          {/* Top-Left Slot: New Project Card */}
          <div className="project-card new-project-slot">
            <div
              id="home-new-slot-hero"
              className="new-slot-media"
              role="button"
              tabIndex={0}
              onClick={() => setCreating(true)}
              onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && setCreating(true)}
              aria-label="Create a new song project"
            >
              <div className="new-slot-plus">
                <svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" strokeWidth="2.4">
                  <path d="M12 5v14M5 12h14" />
                </svg>
              </div>
              <span className="new-slot-badge">Start Here</span>
            </div>
            <div className="card-body">
              <h3 className="card-title">New Song Project</h3>
              <div className="card-path">
                {audioDropStem
                  ? `Ready: “${audioDropStem}” — click Create above`
                  : "Drop an .mp3 / .wav track or start a blank song film"}
              </div>
              <div className="card-status">
                <span className="card-pill song">Audio · Lyrics · Storyboard · Film</span>
              </div>
              <div className="card-actions">
                <button
                  id="home-slot-audio"
                  onClick={() => void handlePickAudio()}
                >
                  Choose Audio…
                </button>
                <button
                  id="home-slot-create"
                  className="open"
                  onClick={() => setCreating(true)}
                >
                  + New Project
                </button>
              </div>
            </div>
          </div>

          {/* Existing Recent Project Cards */}
          {filtered.list.map((p, idx) => {
            const attached = p.open_state === "attached";
            const pills = statusPills(p);
            const quickActions = (p.readiness?.quick_actions || []).filter((qa) => qa.enabled !== false);
            return (
              <div
                key={p.path}
                id={`home-card-${idx}`}
                className={`project-card ${attached ? "attached" : ""}`}
                onClick={() => void onOpenProject(p.path, "project")}
                onMouseEnter={(e) => {
                  const m = e.currentTarget.querySelector(".card-menu");
                  if (m) m.classList.add("open");
                }}
                onMouseLeave={(e) => {
                  const m = e.currentTarget.querySelector(".card-menu");
                  if (m) m.classList.remove("open");
                }}
              >
                <div className="card-media">
                  {p.thumb ? (
                    <img
                      src={convertFileSrc(p.thumb)}
                      alt={`${p.name} thumbnail`}
                      onError={(e) => {
                        (e.target as HTMLImageElement).style.display = "none";
                      }}
                    />
                  ) : (
                    <div
                      dangerouslySetInnerHTML={{
                        __html: waveformSvg(p.name || "Untitled"),
                      }}
                    />
                  )}
                  <div className="card-menu">
                    <button
                      id={`home-reveal-${idx}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        void handleRevealProject(p.path);
                      }}
                    >
                      View in folder
                    </button>
                    <button
                      id={`home-forget-${idx}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        void handleForgetProject(p.path, p.name);
                      }}
                    >
                      Remove from list
                    </button>
                  </div>
                </div>
                <div className="card-body">
                  <h3 className="card-title">{p.name}</h3>
                  <div className="card-path" title={p.path || ""}>
                    {p.folder || p.path || ""}
                  </div>
                  <div className="card-status">
                    {pills.map((pil, i) => (
                      <span key={i} className={`card-pill ${pil.kind}`}>
                        <span
                          className={`card-pill-dot ${pil.kind}`}
                        />
                        {pil.label}
                      </span>
                    ))}
                  </div>
                  <div className="card-actions">
                    {quickActions.slice(0, 2).map((qa, i) => {
                      const targetView: ViewMode =
                        qa.open_block === "lyrics"
                          ? "design"
                          : (qa.view as ViewMode) || "project";
                      return (
                        <button
                          key={i}
                          id={`home-qa-${idx}-${i}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            void onOpenProject(p.path, targetView);
                          }}
                        >
                          {qa.label}
                        </button>
                      );
                    })}
                    <button
                      id={`home-open-card-${idx}`}
                      className="open"
                      onClick={(e) => {
                        e.stopPropagation();
                        void onOpenProject(p.path, "project");
                      }}
                    >
                      Open Project
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
};
