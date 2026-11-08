import React, { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  executeCommand,
  fileUrl,
  pickFile,
  TrunkFile,
  WorkshopData,
} from "../api";
import type { ViewMode } from "../App";
import "../project.css";

interface SongLyricsResponse {
  source?: string;
  line_count?: number;
  sections?: Array<{
    label?: string;
    title?: string;
    lines?: Array<{ text: string }>;
  }>;
  metadata?: {
    title?: string;
    album?: string;
    track_number?: number;
    notes?: string;
  };
}

interface ProjectViewProps {
  projectPath: string;
  data: WorkshopData | null;
  onRefresh: () => Promise<void>;
  onNavigate: (view: ViewMode, characterId?: string) => void;
  onToast: (msg: string) => void;
}

const GROUPS_ORDER: Array<[string, string]> = [
  ["filmcraft", "FilmCraft"],
  ["design_pdf", "DesignCraft & PDF"],
  ["photo_vector", "Photo & Vector"],
  ["effectcraft", "EffectCraft"],
  ["video", "Video & exports"],
  ["texture_3d", "3D & Texture"],
  ["audio_words", "Audio & Lyrics"],
  ["other_assets", "Other assets"],
  ["project_files", "Project & supporting files"],
];

function matchesType(file: TrunkFile, type: string): boolean {
  if (type === "all") return true;
  if (type === "assets") return (file.assets?.length || 0) > 0;
  if (type === "craft") return Boolean(file.is_craft_project);
  if (type === "filmcraft") return file.badge === "FC";
  if (type === "effectcraft") return file.badge === "EC";
  if (type === "photocraft") return file.badge === "PC";
  if (type === "vectorcraft") return file.badge === "VC" || file.badge === "SVG";
  if (type === "video") return /\.(mp4|mov|webm|mkv)$/i.test(file.path || "");
  return file.group === type;
}

function renderFamilyIcon(family: string): React.ReactNode {
  switch (family) {
    case "film":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4M10 9l5 3-5 3z" />
        </svg>
      );
    case "motion":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <path d="m12 3 9 9-9 9-9-9z" />
          <path d="m9 9 6 3-6 3z" />
        </svg>
      );
    case "design":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M10 4v16M3 9h7M14 9h3M14 13h3" />
        </svg>
      );
    case "vector":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <path d="m12 3 8 8-8 10-8-10zM12 3v10" />
          <circle cx="12" cy="14" r="2" />
        </svg>
      );
    case "image":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <circle cx="8" cy="9" r="1.5" />
          <path d="m3 17 6-5 4 3 4-6 4 8" />
        </svg>
      );
    case "model":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <path d="m12 3 9 5v8l-9 5-9-5V8zM3 8l9 5 9-5M12 13v8" />
        </svg>
      );
    case "audio":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <path d="M9 17V5l11-2v12M9 8l11-2" />
          <ellipse cx="6" cy="18" rx="3" ry="2" />
          <ellipse cx="17" cy="16" rx="3" ry="2" />
        </svg>
      );
    case "project":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <path d="m12 3 9 5-9 5-9-5zM3 12l9 5 9-5M3 16l9 5 9-5" />
        </svg>
      );
    default:
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.6">
          <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
          <path d="M14 3v6h6M8 13h8M8 17h5" />
        </svg>
      );
  }
}

export const ProjectView: React.FC<ProjectViewProps> = ({
  projectPath,
  data,
  onRefresh,
  onNavigate,
  onToast,
}) => {
  const [railCollapsed, setRailCollapsed] = useState(false);
  const [songOpen, setSongOpen] = useState(true);
  const [lyrics, setLyrics] = useState<SongLyricsResponse | null>(null);
  const [pfFilter, setPfFilter] = useState("all");
  const [pfSort, setPfSort] = useState("type");
  const [pfGroup, setPfGroup] = useState(true);
  const [pfSearch, setPfSearch] = useState("");
  const [previewPath, setPreviewPath] = useState<string | null>(null);
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState(false);

  const inventory = data?.trunk || null;
  const allFiles = inventory?.files || [];

  const loadExtras = useCallback(async () => {
    try {
      const lyr = await executeCommand<SongLyricsResponse>("song.lyrics");
      setLyrics(lyr);
    } catch {
      setLyrics(null);
    }
  }, []);

  useEffect(() => {
    setPreviewPath(null);
    void loadExtras();
  }, [projectPath, loadExtras]);

  const songStage = data?.status?.stages?.find((s) => s.id === "song");
  const transcriptStage = data?.status?.stages?.find((s) => s.id === "transcript");
  const audioInput = songStage?.inputs?.[0];
  const transcriptWords = transcriptStage?.document?.words || [];

  const counts = useMemo(() => {
    return {
      all: allFiles.length,
      assets: allFiles.filter((f) => matchesType(f, "assets")).length,
      craft: allFiles.filter((f) => f.is_craft_project).length,
      filmcraft: allFiles.filter((f) => matchesType(f, "filmcraft")).length,
      effectcraft: allFiles.filter((f) => matchesType(f, "effectcraft")).length,
      photocraft: allFiles.filter((f) => matchesType(f, "photocraft")).length,
      vectorcraft: allFiles.filter((f) => matchesType(f, "vectorcraft")).length,
      video: allFiles.filter((f) => matchesType(f, "video")).length,
      design_pdf: allFiles.filter((f) => f.group === "design_pdf").length,
      effect_film: allFiles.filter((f) => f.group === "effect_film").length,
      photo_vector: allFiles.filter((f) => f.group === "photo_vector").length,
      texture_3d: allFiles.filter((f) => f.group === "texture_3d").length,
      audio_words: allFiles.filter((f) => f.group === "audio_words").length,
      project_files: allFiles.filter((f) => f.group === "project_files").length,
    };
  }, [allFiles]);

  const sorted = useMemo(() => {
    const q = pfSearch.trim().toLowerCase();
    const filtered = allFiles.filter((f) => {
      if (!matchesType(f, pfFilter)) return false;
      if (!q) return true;
      const assetText = (f.assets || []).map((a) => `${a.name} ${a.id}`).join(" ");
      return `${f.name} ${f.rel_dir} ${f.badge} ${f.group_label} ${assetText}`
        .toLowerCase()
        .includes(q);
    });
    return [...filtered].sort((a, b) => {
      if (pfSort === "name") return String(a.name).localeCompare(String(b.name));
      if (pfSort === "modified") return (b.modified_ms || 0) - (a.modified_ms || 0);
      if (pfSort === "size") return (b.size_bytes || 0) - (a.size_bytes || 0);
      return (
        Number(Boolean(b.current_timeline)) - Number(Boolean(a.current_timeline)) ||
        a.group_order - b.group_order ||
        a.craft_priority - b.craft_priority ||
        String(a.name).localeCompare(String(b.name))
      );
    });
  }, [allFiles, pfFilter, pfSearch, pfSort]);

  const previewFile = useMemo(
    () => sorted.find((f) => f.path === previewPath && f.is_image) || null,
    [sorted, previewPath],
  );

  const runAction = async (fn: () => Promise<void>, refreshAfter = false) => {
    setBusy(true);
    try {
      await fn();
      if (refreshAfter) {
        await onRefresh();
        await loadExtras();
      }
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleOpenFile = (path: string) =>
    void runAction(async () => {
      await executeCommand("workspace.open", { path });
    });

  const handleRevealFile = (path: string) =>
    void runAction(async () => {
      await revealItemInDir(path);
    });

  const handleAnimateFile = (file: TrunkFile) => {
    const folderMatch = (file.rel_dir || "").match(/^assets\/characters\/([^/]+)/);
    const asset = file.assets?.[0];
    const rawId =
      folderMatch?.[1] || asset?.character || asset?.id || file.name.replace(/\.[^.]+$/, "");
    const charId = rawId.replace(/[^a-zA-Z0-9]+/g, "_").replace(/^_+|_+$/g, "");
    onNavigate("animation", charId);
  };

  const handleSetAudio = () =>
    void runAction(async () => {
      const selected = await pickFile({
        title: "Choose Song Audio",
        extensions: ["wav", "mp3", "flac", "m4a", "aac", "ogg"],
      });
      if (typeof selected === "string" && selected) {
        await executeCommand("workshop.set_audio", { path: selected });
        onToast("Audio linked to project");
      }
    }, true);

  const handleImportTranscript = () =>
    void runAction(async () => {
      const selected = await pickFile({
        title: "Import Timed Lyrics / Transcript",
        extensions: ["json", "srt", "vtt", "txt"],
      });
      if (typeof selected === "string" && selected) {
        await executeCommand("workshop.import_transcript", { path: selected });
        onToast("Imported timed transcript");
      }
    }, true);

  const handleTranscribe = () =>
    void runAction(async () => {
      await executeCommand("audio.analyze", { analyze_beats: false });
      onToast("Transcription finished");
    }, true);

  const handlePasteArtwork = () =>
    void runAction(async () => {
      await executeCommand("clipboard.import_image", { expected_project: projectPath });
      onToast("Clipboard artwork saved to project");
    }, true);

  const handleAddAsset = () =>
    void runAction(async () => {
      const selected = await pickFile({
        title: "Add Artwork Asset",
        multiple: true,
        extensions: ["png", "jpg", "jpeg", "webp", "svg", "glb"],
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      for (const p of paths) {
        const name = (p.split(/[/\\]/).pop() || p).replace(/\.[^.]+$/, "");
        await executeCommand("project.add_asset", { path: p, name, image: name, load: false });
      }
      onToast(`Added ${paths.length} asset${paths.length === 1 ? "" : "s"}`);
    }, true);

  const handleImportFiles = () =>
    void runAction(async () => {
      const selected = await pickFile({
        title: "Import Studio File",
        multiple: true,
        extensions: [
          "designcraft",
          "idml",
          "pdf",
          "ecproj",
          "fcproj",
          "mp4",
          "mov",
          "webm",
          "pcraft",
          "psd",
          "vectorcraft",
          "vcraft",
          "svg",
          "tcraft",
          "glb",
          "gltf",
          "png",
          "jpg",
          "jpeg",
          "webp",
          "mp3",
          "wav",
          "flac",
          "m4a",
          "srt",
          "txt",
          "md",
        ],
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      await executeCommand("project.import_file", { paths });
      onToast(`Imported ${paths.length} file${paths.length === 1 ? "" : "s"}`);
    }, true);

  const handleCloseOpenedApps = () =>
    void runAction(async () => {
      const report = await invoke<Array<{ closed: boolean; error?: string; document?: { file?: string } }>>(
        "close_documents",
      );
      if (!report.length) {
        onToast("No Craft apps opened by Magic are currently open");
        return;
      }
      const refused = report.filter((r) => !r.closed);
      if (refused.length) {
        onToast(
          `Still open: ${refused
            .map((r) => `${r.document?.file?.split("/").pop() || "app"} (${r.error || "busy"})`)
            .join("; ")}`,
        );
      } else {
        onToast(`Saved and closed ${report.length} open Craft document${report.length === 1 ? "" : "s"}`);
      }
    });

  const renderRowActions = (file: TrunkFile, rowKey: string) => {
    const asset = file.assets?.[0];
    return (
      <div className="pf-row-actions">
        <button
          id={`pf-open-${rowKey}`}
          className="pf-open-btn"
          title={`${file.open_label} (${file.name})`}
          disabled={!file.path || busy}
          onClick={() => handleOpenFile(file.path)}
        >
          {file.open_label || "Open"}
        </button>
        {file.can_animate && (
          <button
            id={`pf-animate-${rowKey}-${asset?.id || "char"}`}
            className="pf-open-btn animate"
            title={`Animate ${asset?.name || asset?.id || file.name}`}
            onClick={() => handleAnimateFile(file)}
          >
            Animate →
          </button>
        )}
        <button
          id={`pf-reveal-${rowKey}`}
          className="pf-reveal-btn"
          title={`Reveal ${file.name} in folder`}
          aria-label={`Reveal ${file.name} in folder`}
          disabled={!file.path}
          onClick={() => handleRevealFile(file.path)}
        >
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.8">
            <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
          </svg>
        </button>
      </div>
    );
  };

  const renderRow = (file: TrunkFile, rowKey: string) => {
    const family = file.visual_kind || "document";
    const isPreviewing = previewPath === file.path;
    const displayName = file.assets?.[0]?.name || file.name;
    return (
      <div
        key={file.path}
        className={`pf-row pf-family-${family}${isPreviewing ? " previewing" : ""}`}
        data-badge={file.badge}
      >
        {file.is_image ? (
          <button
            id={`pf-preview-${rowKey}`}
            className="pf-file-icon pf-preview-trigger"
            title={`Preview ${file.name}`}
            aria-label={`Preview ${file.name}`}
            aria-expanded={isPreviewing}
            onClick={() => setPreviewPath(isPreviewing ? null : file.path)}
          >
            <img src={fileUrl(file.path)} alt="" loading="lazy" />
          </button>
        ) : (
          <span className="pf-file-icon">{renderFamilyIcon(family)}</span>
        )}
        <span className="pf-file-label" title={file.path}>
          {file.is_image ? (
            <button
              className="pf-name pf-preview-name"
              title={`Preview ${file.name}`}
              onClick={() => setPreviewPath(isPreviewing ? null : file.path)}
            >
              {displayName}
            </button>
          ) : (
            <span className="pf-name">{displayName}</span>
          )}
          <span className="pf-file-location" title={`${file.rel_dir}${file.name}`}>
            {file.assets?.length ? (
              <span className="pf-asset-mark" title="Registered asset" aria-label="Registered asset" />
            ) : null}
            {`${file.rel_dir || "./"}${file.assets?.length ? file.name : ""}`}
          </span>
        </span>
        <span className="pf-type" title={file.group_label}>
          {file.badge || "FILE"}
        </span>
        <span className="pf-meta">{file.detail}</span>
        {renderRowActions(file, rowKey)}
      </div>
    );
  };

  const lyricExcerpt = useMemo(() => {
    const lines = (lyrics?.sections || []).flatMap((s) => (s.lines || []).map((l) => l.text));
    if (lines.length > 0) return lines.slice(0, 10).join("\n");
    if (transcriptStage?.document?.text) return transcriptStage.document.text;
    if (transcriptWords.length > 0) return transcriptWords.map((w) => w.word).join(" ");
    return "";
  }, [lyrics, transcriptStage, transcriptWords]);

  return (
    <section id="view-project" aria-label="Project workspace">
      <div className="project-body">
        {/* Left collapsible Song workflow rail */}
        <aside
          id="blocks"
          className={`blocks${railCollapsed ? " collapsed" : ""}`}
          aria-label="Song workflow"
        >
          <div className="project-rail-top">
            <button
              id="project-rail-toggle"
              className={`icon rail-toggle${railCollapsed ? "" : " open"}`}
              title={railCollapsed ? "Expand song workflow rail" : "Collapse song workflow rail"}
              aria-label={railCollapsed ? "Expand song workflow rail" : "Collapse song workflow rail"}
              onClick={() => setRailCollapsed((v) => !v)}
            >
              <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="3" y="4" width="18" height="16" rx="2" />
                <path d="M9 4v16" />
              </svg>
            </button>
            {!railCollapsed && <span className="muted">Song &amp; Stages</span>}
          </div>

          {!railCollapsed && (
            <>
              <section
                id="block-song"
                className={`panel block${songOpen ? " open" : ""}`}
                data-look="song"
              >
                <button
                  className="project-block-head panel-head"
                  aria-expanded={songOpen}
                  onClick={() => setSongOpen((v) => !v)}
                >
                  <span
                    className={`project-status-dot ${audioInput ? "ready" : "needs-attention"}`}
                  />
                  <span className="title">Song</span>
                  <span className="meta">
                    {audioInput ? audioInput.name || audioInput.path.split("/").pop() : "No audio"}
                  </span>
                  <span className="chev">▸</span>
                </button>
                {songOpen && (
                  <div className="block-inner">
                    {lyrics?.metadata?.title && <strong>{lyrics.metadata.title}</strong>}
                    {lyrics?.metadata?.notes && (
                      <p className="song-note muted">{lyrics.metadata.notes}</p>
                    )}
                    <button id="song-upload" className="ghost" disabled={busy} onClick={handleSetAudio}>
                      {audioInput ? "Change audio…" : "Upload audio…"}
                    </button>

                    {/* Lyrics & notes */}
                    <div className="song-section" data-section="lyrics">
                      <div className="panel-head">
                        <span
                          className={`project-status-dot ${lyricExcerpt ? "ready" : "needs-attention"}`}
                        />
                        <span className="title">Lyrics &amp; notes</span>
                        <span className="meta">
                          {lyrics?.line_count ? `${lyrics.line_count} lines` : ""}
                        </span>
                      </div>
                      {lyricExcerpt ? (
                        <pre className="project-lyric-preview">{lyricExcerpt}</pre>
                      ) : (
                        <span className="why">No artist lyrics linked yet.</span>
                      )}
                      <button
                        id="project-open-lyrics"
                        className="ghost"
                        onClick={() => onNavigate("design")}
                      >
                        Open Lyrics &amp; Motion →
                      </button>
                    </div>

                    {/* Transcript */}
                    <div className="song-section" data-section="transcript">
                      <div className="panel-head">
                        <span
                          className={`project-status-dot ${transcriptWords.length ? "ready" : "needs-attention"}`}
                        />
                        <span className="title">Transcript</span>
                        <span className="meta">
                          {transcriptWords.length ? `${transcriptWords.length} timed words` : "Untimed"}
                        </span>
                      </div>
                      <button
                        id="song-import-transcript"
                        className="ghost"
                        disabled={busy}
                        onClick={handleImportTranscript}
                      >
                        Import timed lyrics…
                      </button>
                      {!transcriptWords.length && audioInput && (
                        <button
                          id="song-transcribe"
                          className="start"
                          disabled={busy}
                          onClick={handleTranscribe}
                        >
                          Transcribe this recording
                        </button>
                      )}
                    </div>
                  </div>
                )}
              </section>

              <div className="project-stage-jumps">
                <button
                  id="project-jump-storyboard"
                  className="ghost"
                  onClick={() => onNavigate("storyboard")}
                >
                  Storyboard →
                </button>
                <button
                  id="project-jump-animation"
                  className="ghost"
                  onClick={() => onNavigate("animation")}
                >
                  Animation →
                </button>
                <button
                  id="project-jump-lyrics"
                  className="ghost"
                  onClick={() => onNavigate("design")}
                >
                  Lyrics →
                </button>
                <button
                  id="project-jump-studio"
                  className="ghost"
                  onClick={() => onNavigate("studio")}
                >
                  Studio →
                </button>
              </div>

              <button
                id="project-close-apps"
                className="ghost"
                disabled={busy}
                onClick={handleCloseOpenedApps}
              >
                Close apps opened by Magic
              </button>
            </>
          )}
        </aside>

        {/* Main Project Files & Assets Trunk */}
        <section id="project-contents" className="project-contents" aria-label="Project contents">
          <div id="project-workspace-stack" className="project-workspace-stack">
            <section
              id="project-files-panel"
              className="pf-panel"
              aria-label="Project Files & Assets"
            >
              <div className="pf-header">
                <div className="pf-heading">
                  <strong className="pf-title">Project Files &amp; Assets</strong>
                  <span className="pf-count">
                    {allFiles.length} files · {inventory?.registered_asset_count || 0} registered assets
                  </span>
                </div>
                <div className="pf-toolbar">
                  <select
                    id="pf-type-filter"
                    className="pf-select"
                    aria-label="Filter project files by type"
                    value={pfFilter}
                    onChange={(e) => setPfFilter(e.target.value)}
                  >
                    <option value="all">Type: All Files ({counts.all})</option>
                    <option value="assets">Type: Registered Assets ({counts.assets})</option>
                    <option value="craft">Type: Craft Projects ({counts.craft})</option>
                    <option value="filmcraft">Type: FilmCraft ({counts.filmcraft})</option>
                    <option value="effectcraft">Type: EffectCraft ({counts.effectcraft})</option>
                    <option value="photocraft">Type: PhotoCraft ({counts.photocraft})</option>
                    <option value="vectorcraft">Type: VectorCraft ({counts.vectorcraft})</option>
                    <option value="video">Type: Video &amp; exports ({counts.video})</option>
                    <option value="design_pdf">Type: Design &amp; PDF ({counts.design_pdf})</option>
                    <option value="effect_film">Type: Effect &amp; Film ({counts.effect_film})</option>
                    <option value="photo_vector">Type: Photo &amp; Vector ({counts.photo_vector})</option>
                    <option value="texture_3d">Type: 3D &amp; Texture ({counts.texture_3d})</option>
                    <option value="audio_words">Type: Audio &amp; Lyrics ({counts.audio_words})</option>
                    <option value="project_files">
                      Type: Project &amp; supporting files ({counts.project_files})
                    </option>
                  </select>

                  <select
                    id="pf-sort"
                    className="pf-select"
                    aria-label="Sort project files"
                    value={pfSort}
                    onChange={(e) => setPfSort(e.target.value)}
                  >
                    <option value="type">Sort: Type</option>
                    <option value="name">Sort: Name</option>
                    <option value="modified">Sort: Modified</option>
                    <option value="size">Sort: Size</option>
                  </select>

                  <label className="pf-group-label" htmlFor="pf-group-toggle">
                    <input
                      id="pf-group-toggle"
                      type="checkbox"
                      checked={pfGroup}
                      onChange={(e) => setPfGroup(e.target.checked)}
                      aria-label="Group files by Craft type"
                    />
                    <span>Group</span>
                  </label>

                  <div className="pf-search-wrap">
                    <input
                      id="pf-search"
                      type="search"
                      className="pf-search"
                      placeholder="Filter by name..."
                      aria-label="Filter files by name"
                      value={pfSearch}
                      onChange={(e) => setPfSearch(e.target.value)}
                    />
                  </div>

                  <button id="pf-paste" className="pf-btn" disabled={busy} onClick={handlePasteArtwork}>
                    Paste artwork
                  </button>
                  <button
                    id="pf-add-asset"
                    className="pf-btn pf-btn-primary"
                    disabled={busy}
                    onClick={handleAddAsset}
                  >
                    + Add Asset
                  </button>
                  <button
                    id="pf-import"
                    className="pf-btn pf-btn-primary"
                    disabled={busy}
                    onClick={handleImportFiles}
                  >
                    + Import File
                  </button>
                </div>
              </div>

              {inventory?.error && (
                <div className="pf-inventory-warning" role="alert">
                  Unable to read project files: {inventory.error}
                </div>
              )}

              {Boolean(inventory?.external_assets?.length || inventory?.missing_assets?.length) && (
                <div className="pf-inventory-warning" role="status">
                  {inventory?.external_assets?.length || 0} external asset links ·{" "}
                  {inventory?.missing_assets?.length || 0} missing assets.
                </div>
              )}

              {previewFile && (
                <section
                  id="pf-preview-panel"
                  className="pf-preview"
                  aria-label={`Preview ${previewFile.name}`}
                >
                  <img src={fileUrl(previewFile.path)} alt={previewFile.name} decoding="async" />
                  <div className="pf-preview-detail">
                    <strong>{previewFile.assets?.[0]?.name || previewFile.name}</strong>
                    <span className="muted">
                      {previewFile.rel_dir}
                      {previewFile.name}
                    </span>
                    {renderRowActions(previewFile, "preview")}
                  </div>
                  <button
                    id="pf-preview-close"
                    className="pf-reveal-btn"
                    aria-label="Close preview"
                    title="Close preview"
                    onClick={() => setPreviewPath(null)}
                  >
                    ×
                  </button>
                </section>
              )}

              <div id="pf-list" className="pf-list">
                {!sorted.length ? (
                  <div className="pf-empty">No project files match the active filter.</div>
                ) : pfGroup ? (
                  GROUPS_ORDER.map(([gKey, gLabel]) => {
                    const items = sorted.filter((f) => matchesType(f, gKey));
                    if (!items.length) return null;
                    const collapsed = Boolean(collapsedGroups[gKey]);
                    return (
                      <div key={gKey} className={`pf-group${collapsed ? " collapsed" : ""}`}>
                        <button
                          id={`pf-group-${gKey}`}
                          className="pf-group-head"
                          aria-expanded={!collapsed}
                          onClick={() =>
                            setCollapsedGroups((prev) => ({ ...prev, [gKey]: !prev[gKey] }))
                          }
                        >
                          <span className="pf-group-arrow">{collapsed ? "▸" : "▾"}</span>{" "}
                          {gLabel} ({items.length})
                        </button>
                        {!collapsed && (
                          <div className="pf-group-rows">
                            {items.map((file, idx) => renderRow(file, `${gKey}-${idx}`))}
                          </div>
                        )}
                      </div>
                    );
                  })
                ) : (
                  <div className="pf-group-rows">
                    {sorted.map((file, idx) => renderRow(file, `flat-${idx}`))}
                  </div>
                )}
              </div>
            </section>
          </div>
        </section>
      </div>
    </section>
  );
};
