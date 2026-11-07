import React, { useEffect, useRef, useState } from "react";
import { StudioFilmstrips } from "../components/StudioFilmstrips";
import { FilmExportButton } from "../components/FilmExportButton";
import {
  executeCommand,
  fileUrl,
  FilmStatusResponse,
  TimelineResponse,
} from "../api";

interface FilmViewProps {
  projectPath: string;
  onToast: (msg: string) => void;
}

export const FilmView: React.FC<FilmViewProps> = ({ projectPath, onToast }) => {
  const [film, setFilm] = useState<FilmStatusResponse | null>(null);
  const [timeline, setTimeline] = useState<TimelineResponse | null>(null);
  const [statusText, setStatusText] = useState<string>("Loading film…");
  const [busy, setBusy] = useState<boolean>(false);
  const [exportFormat, setExportFormat] = useState<string>("h264");
  const [burnCaptions, setBurnCaptions] = useState<boolean>(false);
  const [captionSidecar, setCaptionSidecar] = useState<string>("");
  const [exportRange, setExportRange] = useState<string>("entire");
  const [exportSettingsEdited, setExportSettingsEdited] = useState(false);
  const [playhead, setPlayhead] = useState<number>(0);
  const videoRef = useRef<HTMLVideoElement | null>(null);

  const refreshFilm = async () => {
    try {
      const f = await executeCommand<FilmStatusResponse>("video.film");
      setFilm(f);
      if (f.preview?.exists) {
        setStatusText(
          f.preview_stale
            ? "Preview is older than timeline — click Preview to re-render"
            : "Preview ready",
        );
      } else if (f.timeline?.exists) {
        setStatusText("Timeline exists — click Preview or Export");
      } else {
        setStatusText("No film timeline yet — click Assemble to build .fcproj");
      }
      if (f.timeline?.exists) {
        const tl = await executeCommand<TimelineResponse>("video.timeline");
        setTimeline(tl);
      } else {
        setTimeline(null);
      }
    } catch (err) {
      setStatusText(String(err));
    }
  };

  useEffect(() => {
    setExportSettingsEdited(false);
    void refreshFilm();
  }, [projectPath]);

  const runFilmAction = async (label: string, fn: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    setStatusText(`${label}…`);
    try {
      await fn();
      await refreshFilm();
      setStatusText(`${label} complete`);
      onToast(`${label} complete`);
    } catch (err) {
      const msg = String(err);
      setStatusText(msg);
      onToast(msg);
    } finally {
      setBusy(false);
    }
  };

  const handleExportInterchange = async (fmt: string) => {
    if (!film?.timeline?.path) return;
    const outPath = film.timeline.path.replace(/\.fcproj$/, `.${fmt}`);
    await runFilmAction(`Export ${fmt.toUpperCase()}`, () =>
      executeCommand("video.exec", {
        id: "file.exportInterchange",
        params: { format: fmt, path: outPath },
        save: false,
      }),
    );
  };

  const videoSrc = film?.preview?.exists && !film.preview_stale
    ? `${fileUrl(film.preview.path)}?v=${film.preview.modified || 0}`
    : film?.export?.exists
      ? `${fileUrl(film.export.path)}?v=${film.export.modified || 0}`
      : "";


  return (
    <div
      id="view-studio"
      className="film-view"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "12px",
        padding: "16px",
        height: "100%",
        overflowY: "auto",
      }}
    >
      {/* Top Action Bar */}
      <div
        id="film-head"
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          flexWrap: "wrap",
          gap: "10px",
          background: "var(--surface-2, #181c22)",
          padding: "12px 16px",
          borderRadius: "8px",
        }}
      >
        <div>
          <strong style={{ fontSize: "16px" }}>FilmCraft Studio</strong>
          <p
            id="film-status"
            className="muted"
            role="status"
            style={{ margin: "2px 0 0", fontSize: "12px" }}
          >
            {statusText}
          </p>
        </div>

        <div style={{ display: "flex", gap: "8px", flexWrap: "wrap", alignItems: "center" }}>
          <button
            id="film-assemble"
            className="ghost"
            disabled={busy}
            onClick={() =>
              void runFilmAction("Assemble", () =>
                executeCommand("video.assemble", { export: false }),
              )
            }
          >
            Assemble
          </button>
          <button
            id="film-preview"
            className="start"
            disabled={busy}
            onClick={() =>
              void runFilmAction("Preview", () =>
                executeCommand("video.preview"),
              )
            }
          >
            Preview
          </button>
          <button
            id="film-frame"
            className="ghost"
            disabled={busy || !film?.timeline?.exists}
            onClick={() =>
              void runFilmAction(`Export Frame @ ${playhead.toFixed(1)}s`, () =>
                executeCommand("video.frame", { seconds: playhead }),
              )
            }
          >
            Grab Frame ({playhead.toFixed(1)}s)
          </button>
          <button
            id="film-open"
            className="ghost"
            disabled={busy || !film?.timeline?.exists}
            onClick={() =>
              void runFilmAction("Open in FilmCraft", () =>
                executeCommand("video.open"),
              )
            }
          >
            Open in FilmCraft
          </button>
          <FilmExportButton project={projectPath} disabled={busy || !film?.timeline?.exists} onBusy={setBusy} onToast={onToast}
            options={exportSettingsEdited ? {format:exportFormat,range:exportRange,burnCaptions,captionSidecar:captionSidecar || null} : {}} />
        </div>
      </div>

      {/* Optional settings stay collapsed; exporting itself opens no dialog. */}
      <details id="film-export-settings">
      <summary id="film-export-settings-toggle" aria-label="Export settings and interchange" title="Export settings and interchange">⚙</summary>
      <div
        style={{
          display: "flex",
          gap: "12px",
          alignItems: "center",
          flexWrap: "wrap",
          background: "var(--surface-2, #181c22)",
          padding: "10px 16px",
          borderRadius: "8px",
          fontSize: "12px",
        }}
      >
        <strong>Export Options:</strong>
        <label style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <span>Format</span>
          <select
            id="film-export-format"
            value={exportFormat}
            onChange={(e) => { setExportFormat(e.target.value); setExportSettingsEdited(true); }}
          >
            <option value="h264">H.264 (.mp4)</option>
            <option value="hevc">HEVC / H.265 (.mp4)</option>
            <option value="prores">Apple ProRes (.mov)</option>
            <option value="dnxhr">Avid DNxHR (.mov)</option>
            <option value="gif">Animated GIF (.gif)</option>
            <option value="wav">Audio Master (.wav)</option>
            <option value="flac">Lossless Audio (.flac)</option>
          </select>
        </label>

        <label style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <span>Range</span>
          <select
            id="film-export-range"
            value={exportRange}
            onChange={(e) => { setExportRange(e.target.value); setExportSettingsEdited(true); }}
          >
            <option value="entire">Entire Sequence</option>
            <option value="inOut">In / Out Range</option>
            <option value="workArea">Work Area</option>
          </select>
        </label>

        <label style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <input
            id="film-burn-captions"
            type="checkbox"
            checked={burnCaptions}
            onChange={(e) => { setBurnCaptions(e.target.checked); setExportSettingsEdited(true); }}
          />
          <span>Burn Captions</span>
        </label>

        <label style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <span>Sidecar</span>
          <select
            id="film-caption-sidecar"
            value={captionSidecar}
            onChange={(e) => { setCaptionSidecar(e.target.value); setExportSettingsEdited(true); }}
          >
            <option value="">None</option>
            <option value="srt">.srt</option>
            <option value="vtt">.vtt</option>
          </select>
        </label>

        <div style={{ marginLeft: "auto", display: "flex", gap: "6px" }}>
          <button
            className="ghost"
            disabled={busy || !film?.timeline?.exists}
            onClick={() => void handleExportInterchange("fcpxml")}
            id="film-export-fcpxml"
            title="Export Final Cut Pro FCPXML via filmcraft_engine"
          >
            FCPXML
          </button>
          <button
            className="ghost"
            disabled={busy || !film?.timeline?.exists}
            onClick={() => void handleExportInterchange("otio")}
            id="film-export-otio"
            title="Export OpenTimelineIO via filmcraft_engine"
          >
            OTIO
          </button>
          <button
            className="ghost"
            disabled={busy || !film?.timeline?.exists}
            onClick={() => void handleExportInterchange("edl")}
            id="film-export-edl"
            title="Export CMX3600 EDL via filmcraft_engine"
          >
            EDL
          </button>
        </div>
      </div>

      </details>

      <StudioFilmstrips project={projectPath} timeline={timeline} videoSrc={videoSrc}
        revision={film?.timeline?.modified || 0} playhead={playhead} onTime={setPlayhead} videoRef={videoRef} />
    </div>
  );
};
