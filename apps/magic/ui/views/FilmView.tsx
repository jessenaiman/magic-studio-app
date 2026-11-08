import React, { useEffect, useState } from "react";
import { FilmExportButton } from "../components/FilmExportButton";
import {
  executeCommand,
  FilmStatusResponse,
} from "../api";
import "../film.css";

interface FilmViewProps {
  projectPath: string;
  onToast: (msg: string) => void;
}

export const FilmView: React.FC<FilmViewProps> = ({ projectPath, onToast }) => {
  const [film, setFilm] = useState<FilmStatusResponse | null>(null);
  const [statusText, setStatusText] = useState<string>("Loading film…");
  const [busy, setBusy] = useState<boolean>(false);
  const [exportFormat, setExportFormat] = useState<string>("h264");
  const [burnCaptions, setBurnCaptions] = useState<boolean>(false);
  const [captionSidecar, setCaptionSidecar] = useState<string>("");
  const [exportRange, setExportRange] = useState<string>("entire");
  const [exportSettingsEdited, setExportSettingsEdited] = useState(false);

  const refreshFilm = async () => {
    try {
      const f = await executeCommand<FilmStatusResponse>("video.film");
      setFilm(f);
      setStatusText(
        f.storyboard_stale
          ? "The storyboard changed after this timeline was built"
          : f.timeline?.exists
          ? "Timeline ready — open it in FilmCraft"
          : "No film timeline yet — click Assemble to build .fcproj",
      );
    } catch (err) {
      setStatusText(String(err));
    }
  };

  useEffect(() => {
    setExportSettingsEdited(false);
    void refreshFilm();
  }, [projectPath]);

  useEffect(() => {
    if (film?.timeline?.exists) void executeCommand("video.open").catch((err) => setStatusText(String(err)));
  }, [projectPath, film?.timeline?.exists]);

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

  return (
    <div
      id="view-studio"
      className="film-view film-view-layout"
      
    >
      {/* Top Action Bar */}
      <div
        id="film-head"
        className="film-head"
      >
        <div>
          <strong className="film-title">FilmCraft Studio</strong>
          <p
            id="film-status"
            className="muted film-status"
            role="status"
            
          >
            {statusText}
          </p>
        </div>

        <div className="film-actions">
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
        className="film-export-settings"
      >
        <strong>Export Options:</strong>
        <label className="film-inline-field">
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

        <label className="film-inline-field">
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

        <label className="film-inline-field">
          <input
            id="film-burn-captions"
            type="checkbox"
            checked={burnCaptions}
            onChange={(e) => { setBurnCaptions(e.target.checked); setExportSettingsEdited(true); }}
          />
          <span>Burn Captions</span>
        </label>

        <label className="film-inline-field">
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

        <div className="film-export-actions">
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
    </div>
  );
};
