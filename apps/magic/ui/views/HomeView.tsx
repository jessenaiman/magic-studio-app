import React, { useCallback, useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { executeCommand, pickFile } from "../api";
import "../home.css";

interface Readiness {
  next_step?: string;
  audio_available?: boolean;
  transcript_valid?: boolean;
  editor_ready?: boolean;
}

interface QuickAction {
  label: string;
  command: string;
}

interface CatalogProject {
  name: string;
  path: string;
  folder?: string;
  exists?: boolean;
  thumb?: string;
  open_state?: "attached" | "detached" | null;
  readiness?: Readiness;
  quick_actions?: QuickAction[];
}

interface CatalogResponse {
  projects?: CatalogProject[];
}

interface HomeViewProps {
  onOpenProject: (path: string) => Promise<void>;
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

  const handleCreateProject = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName.trim()) {
      onToast("Enter a project name first");
      return;
    }
    setBusy(true);
    try {
      const slug = newName
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "");
      const dir = `${newRoot || "/tmp"}/${slug || "song"}`;
      const magicPath = `${dir}/${newName.trim()}.magic`;
      await executeCommand("project.new", {
        name: newName.trim(),
        path: magicPath,
      });
      if (newAudio) {
        await executeCommand("workshop.set_audio", { path: newAudio });
      }
      setCreating(false);
      setNewAudio("");
      setAudioDropStem(null);
      await onOpenProject(magicPath);
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
            <h2>Magic Studio</h2>
          </div>
          <div className="home-filters">
            <button
              className={`home-filter-pill ${filter === "all" ? "active" : ""}`}
              onClick={() => setFilter("all")}
            >
              All Songs ({filtered.allCount})
            </button>
            <button
              className={`home-filter-pill ${filter === "production" ? "active" : ""}`}
              onClick={() => setFilter("production")}
            >
              In Production
            </button>
            <button
              className={`home-filter-pill ${filter === "render" ? "active" : ""}`}
              onClick={() => setFilter("render")}
            >
              Ready to Render
            </button>
            <button
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

        {/* Quick-Drop / Audio Intake strip */}
        <div
          className="home-quickdrop"
          role="button"
          tabIndex={0}
          onClick={handlePickAudio}
          onKeyDown={(e) => e.key === "Enter" && handlePickAudio()}
          aria-label="Drop an audio track to start a new song film"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
          <p>Drop an .mp3 / .wav audio track anywhere to start a new song film</p>
          {audioDropStem && (
            <p className="stem-hint">Ready: “{audioDropStem}” — review name below and click Create</p>
          )}
        </div>

        {/* New project form (inline drawer / modal) */}
        {creating && (
          <form id="new-form" onSubmit={handleCreateProject} aria-label="New Song Project">
            <h3 className="home-section-title">New Song Project</h3>
            <div className="new-form-field">
              <label>Song / Project Name</label>
              <input
                id="new-name"
                autoFocus
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder="e.g. X Marks The Spot"
                required
              />
            </div>
            <div className="new-form-field">
              <label>Audio Recording (optional)</label>
              <div className="new-form-row">
                <input
                  id="new-audio"
                  value={newAudio}
                  onChange={(e) => setNewAudio(e.target.value)}
                  placeholder="/path/to/song.wav"
                />
                <button type="button" onClick={handlePickAudio}>
                  Browse…
                </button>
              </div>
            </div>
            <div className="new-form-field">
              <label>Projects Root Folder</label>
              <div className="new-form-row">
                <input
                  id="new-root"
                  value={newRoot}
                  onChange={(e) => setNewRoot(e.target.value)}
                />
                <button type="button" onClick={handlePickRoot}>
                  Folder…
                </button>
              </div>
            </div>
            <div className="new-form-actions">
              <button
                type="button"
                onClick={() => {
                  setCreating(false);
                  setAudioDropStem(null);
                }}
              >
                Cancel
              </button>
              <button type="submit" className="open" disabled={busy}>
                {busy ? "Creating…" : "Create Project"}
              </button>
            </div>
          </form>
        )}

        {/* Recent projects grid */}
        <h3 className="home-section-title">Recent Projects</h3>
        <div id="recent">
          {filtered.list.length === 0 ? (
            <div className="empty-state">
              <p>No projects match your filter yet.</p>
              <p style={{ fontSize: "12px", marginTop: "6px" }}>
                Click <strong>+ New Song Project</strong> or drop an audio track on the strip above to begin.
              </p>
            </div>
          ) : (
            filtered.list.map((p) => {
              const attached = p.open_state === "attached";
              const pills = statusPills(p);
              return (
                <div
                  key={p.path}
                  className={`project-card ${attached ? "attached" : ""}`}
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
                      <button onClick={(e) => { e.stopPropagation(); onToast(`View folder: ${p.name}`); }}>
                        View in folder
                      </button>
                      <button onClick={(e) => { e.stopPropagation(); onToast(`Duplicate: ${p.name}`); }}>
                        Duplicate
                      </button>
                      <button onClick={(e) => { e.stopPropagation(); onToast(`Remove: ${p.name}`); }}>
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
                            style={{
                              display: "inline-block",
                              width: "6px",
                              height: "6px",
                              borderRadius: "50%",
                              backgroundColor:
                                pil.kind === "song"
                                  ? "#5bd69b"
                                  : pil.kind === "lyrics"
                                    ? "#7fe0b3"
                                    : pil.kind === "studio"
                                      ? "#eabb62"
                                      : "#7a8594",
                            }}
                          />
                          {pil.label}
                        </span>
                      ))}
                    </div>
                    <div className="card-actions">
                      {p.quick_actions?.map((qa, i) => (
                        <button
                          key={i}
                          onClick={(e) => {
                            e.stopPropagation();
                            void executeCommand(qa.command).catch((err) => onToast(String(err)));
                          }}
                        >
                          {qa.label}
                        </button>
                      ))}
                      <button
                        className="open"
                        onClick={(e) => {
                          e.stopPropagation();
                          void onOpenProject(p.path);
                        }}
                      >
                        Open Project
                      </button>
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </section>
  );
};
