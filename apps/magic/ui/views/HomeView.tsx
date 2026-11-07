import React, { useEffect, useState } from "react";
import { executeCommand, pickFile, ProjectSummary } from "../api";

interface CatalogResponse {
  projects?: Array<{
    name: string;
    path: string;
    folder?: string;
    exists?: boolean;
    readiness?: {
      next_step?: string;
      audio_available?: boolean;
    };
  }>;
}

interface HomeViewProps {
  onOpenProject: (path: string) => Promise<void>;
  onToast: (msg: string) => void;
}

export const HomeView: React.FC<HomeViewProps> = ({ onOpenProject, onToast }) => {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [newAudio, setNewAudio] = useState("");
  const [newRoot, setNewRoot] = useState("");
  const [busy, setBusy] = useState(false);

  const loadCatalog = async () => {
    try {
      const cat = await executeCommand<CatalogResponse>("project.catalog");
      const list: ProjectSummary[] = (cat.projects || []).map((p) => ({
        name: p.name,
        path: p.path,
        dir: p.folder,
        exists: p.exists,
      }));
      setProjects(list);
      const rootInfo = await executeCommand<{ root?: string }>("project.root");
      if (rootInfo.root) setNewRoot(rootInfo.root);
    } catch (err) {
      onToast(String(err));
    }
  };

  useEffect(() => {
    void loadCatalog();
  }, []);

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
      if (!newName.trim()) {
        const stem = selected
          .split("/")
          .pop()
          ?.replace(/\.[^.]+$/, "")
          .replace(/[._-]+/g, " ")
          .trim();
        if (stem) setNewName(stem);
      }
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
      await onOpenProject(magicPath);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section id="view-home" className="view active" style={{ padding: "24px" }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "20px",
        }}
      >
        <div>
          <h1 style={{ margin: 0, fontSize: "22px" }}>Magic Studio</h1>
          <p className="muted" style={{ margin: "4px 0 0" }}>
            Song-driven storyboard, spatial lyrics, character animation, and FilmCraft studio
          </p>
        </div>
        <div style={{ display: "flex", gap: "8px" }}>
          <button
            id="home-new"
            className="start"
            onClick={() => setCreating((v) => !v)}
          >
            New Song Project
          </button>
          <button id="home-open" className="ghost" onClick={handleBrowseProject}>
            Open .magic…
          </button>
        </div>
      </div>

      {creating && (
        <form
          id="new-form"
          onSubmit={handleCreateProject}
          style={{
            background: "var(--surface-2, #181c22)",
            padding: "16px",
            borderRadius: "8px",
            marginBottom: "20px",
            display: "grid",
            gap: "12px",
            maxWidth: "560px",
          }}
        >
          <strong>New Song Project</strong>
          <label style={{ display: "grid", gap: "4px" }}>
            <span>Song / Project Name</span>
            <input
              id="new-name"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="e.g. X Marks The Spot"
              required
            />
          </label>
          <label style={{ display: "grid", gap: "4px" }}>
            <span>Audio Recording (optional)</span>
            <div style={{ display: "flex", gap: "8px" }}>
              <input
                id="new-audio"
                value={newAudio}
                onChange={(e) => setNewAudio(e.target.value)}
                placeholder="/path/to/song.wav"
                style={{ flex: 1 }}
              />
              <button type="button" className="ghost" onClick={handlePickAudio}>
                Browse…
              </button>
            </div>
          </label>
          <label style={{ display: "grid", gap: "4px" }}>
            <span>Projects Root Folder</span>
            <div style={{ display: "flex", gap: "8px" }}>
              <input
                id="new-root"
                value={newRoot}
                onChange={(e) => setNewRoot(e.target.value)}
                style={{ flex: 1 }}
              />
              <button type="button" className="ghost" onClick={handlePickRoot}>
                Folder…
              </button>
            </div>
          </label>
          <div style={{ display: "flex", gap: "8px" }}>
            <button type="submit" className="start" disabled={busy}>
              {busy ? "Creating…" : "Create Project"}
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => setCreating(false)}
            >
              Cancel
            </button>
          </div>
        </form>
      )}

      <h2 style={{ fontSize: "14px", textTransform: "uppercase", opacity: 0.7 }}>
        Recent Projects
      </h2>
      {projects.length === 0 ? (
        <div className="panel-empty boxed">
          No recent projects yet. Click <strong>Open .magic…</strong> or{" "}
          <strong>New Song Project</strong>.
        </div>
      ) : (
        <div
          id="recent"
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
            gap: "12px",
          }}
        >
          {projects.map((p) => (
            <div
              key={p.path}
              className="project-card"
              style={{
                background: "var(--surface-2, #181c22)",
                border: "1px solid var(--border, #2a313c)",
                borderRadius: "8px",
                padding: "14px",
                display: "flex",
                flexDirection: "column",
                justifyContent: "space-between",
                gap: "10px",
              }}
            >
              <div>
                <strong style={{ fontSize: "15px", display: "block" }}>
                  {p.name}
                </strong>
                <span
                  className="muted"
                  style={{
                    fontSize: "12px",
                    wordBreak: "break-all",
                    display: "block",
                    marginTop: "4px",
                  }}
                >
                  {p.path}
                </span>
              </div>
              <div style={{ display: "flex", gap: "8px" }}>
                <button
                  className="start"
                  onClick={() => void onOpenProject(p.path)}
                >
                  Open Project
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
};
