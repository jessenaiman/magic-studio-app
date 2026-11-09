import React, { useEffect, useRef, useState } from "react";
import { StudioFilmstrips } from "../components/StudioFilmstrips";
import { FilmExportButton } from "../components/FilmExportButton";
import {
  executeCommand,
  fileUrl,
  FilmStatusResponse,
  TimelineResponse,
} from "../api";
import "../film.css";

interface FilmViewProps {
  projectPath: string;
  onToast: (msg: string) => void;
}

export const FilmView: React.FC<FilmViewProps> = ({ projectPath, onToast }) => {
  const [film, setFilm] = useState<FilmStatusResponse | null>(null);
  const [timeline, setTimeline] = useState<TimelineResponse | null>(null);
  const [statusText, setStatusText] = useState<string>("Loading film…");
  const [busy, setBusy] = useState<boolean>(false);
  const [exportPreset, setExportPreset] = useState<string>("");
  const [exportPresets, setExportPresets] = useState<Array<{ id: string; name?: string }>>([
    { id: "youtube_1080p", name: "YouTube 1080p (H.264)" },
    { id: "h265_web", name: "Web 4K / HEVC" },
    { id: "prores_master", name: "Apple ProRes 422 HQ Master" },
    { id: "dnxhr_hq", name: "Avid DNxHR HQ Master" },
    { id: "audio_wav", name: "Broadcast WAV Master" },
  ]);
  const [exportFormat, setExportFormat] = useState<string>("h264");
  const [exportScale, setExportScale] = useState<number>(1);
  const [exportQuality, setExportQuality] = useState<number>(85);
  const [gpuRendering, setGpuRendering] = useState<string>("auto");
  const [includeAudio, setIncludeAudio] = useState<boolean>(true);
  const [allowOffline, setAllowOffline] = useState<boolean>(false);
  const [burnCaptions, setBurnCaptions] = useState<boolean>(false);
  const [captionSidecar, setCaptionSidecar] = useState<string>("");
  const [exportRange, setExportRange] = useState<string>("entire");
  const [customStart, setCustomStart] = useState<number>(0);
  const [customEnd, setCustomEnd] = useState<number>(30);
  const [queueExport, setQueueExport] = useState<boolean>(false);
  const [exportSettingsEdited, setExportSettingsEdited] = useState(false);
  const [playhead, setPlayhead] = useState<number>(0);
  const videoRef = useRef<HTMLVideoElement | null>(null);

  const refreshFilm = async () => {
    try {
      const f = await executeCommand<FilmStatusResponse>("video.film");
      setFilm(f);
      setStatusText(
        f.preview?.exists
          ? f.preview_stale
            ? "Preview loaded (timeline updated since last render — click Preview to refresh)"
            : "Preview ready"
          : f.export?.exists
          ? "Export ready"
          : f.timeline?.exists
          ? "Timeline ready — scrubbing live sequence frames (click Preview for full audio/video draft)"
          : "No film timeline yet — click Assemble to build .fcproj",
      );
      if (f.timeline?.exists) {
        const tl = await executeCommand<TimelineResponse>("video.timeline");
        setTimeline(tl);
        try {
          const presetsRes = await executeCommand<{ result?: Array<{ id?: string; name?: string }> }>("video.exec", {
            id: "export.presets.list",
            params: {},
            save: false,
          });
          if (Array.isArray(presetsRes?.result) && presetsRes.result.length > 0) {
            setExportPresets(
              presetsRes.result
                .filter((item) => typeof item?.id === "string" && item.id.length > 0)
                .map((item) => ({ id: String(item.id), name: item.name ? String(item.name) : String(item.id) })),
            );
          }
        } catch {
          // Keep default preset list if sequence preset query is unavailable.
        }
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

  const videoSrc = film?.preview?.exists
    ? `${fileUrl(film.preview.path)}?v=${film.preview.modified || 0}`
    : film?.export?.exists
      ? `${fileUrl(film.export.path)}?v=${film.export.modified || 0}`
      : "";

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
            options={exportSettingsEdited ? {
              ...(exportPreset ? { preset: exportPreset } : {}),
              format: exportFormat,
              range: exportRange === "custom" ? "entire" : exportRange,
              ...(exportRange === "custom" ? { startSeconds: customStart, endSeconds: customEnd } : {}),
              scale: exportScale,
              quality: exportQuality,
              gpuRendering,
              audio: includeAudio,
              allowOffline,
              burnCaptions,
              queue: queueExport,
              captionSidecar: captionSidecar || null,
            } : {}} />
        </div>
      </div>

      {/* Optional settings stay collapsed; exporting itself opens no dialog. */}
      <details id="film-export-settings">
        <summary id="film-export-settings-toggle" aria-label="Export settings and interchange" title="Export settings and interchange">⚙</summary>
        <div className="film-export-settings">
          <strong>Export Options:</strong>
          <label className="film-inline-field">
            <span>Preset</span>
            <select
              id="film-export-preset"
              value={exportPreset}
              onChange={(e) => { setExportPreset(e.target.value); setExportSettingsEdited(true); }}
            >
              <option value="">Custom Settings</option>
              {exportPresets.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.name || preset.id}
                </option>
              ))}
            </select>
          </label>

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
              <option value="apv">APV (.mov)</option>
              <option value="mjpeg">Motion JPEG (.mov)</option>
              <option value="mxf-op1a">MXF OP1a (.mxf)</option>
              <option value="mxf-opatom">MXF OP-Atom (.mxf)</option>
              <option value="png">PNG Sequence (.png)</option>
              <option value="tiff">TIFF Sequence (.tiff)</option>
              <option value="bmp">BMP Sequence (.bmp)</option>
              <option value="gif">Animated GIF (.gif)</option>
              <option value="wav">Audio Master (.wav)</option>
              <option value="aiff">Audio AIFF (.aiff)</option>
              <option value="flac">Lossless Audio (.flac)</option>
              <option value="aac">AAC Audio (.aac)</option>
              <option value="mp3">MP3 Audio (.mp3)</option>
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
              <option value="custom">Custom Seconds (--start / --end)</option>
            </select>
          </label>

          {exportRange === "custom" && (
            <>
              <label className="film-inline-field">
                <span>Start (s)</span>
                <input
                  id="film-export-start"
                  type="number"
                  min={0}
                  step={0.1}
                  value={customStart}
                  onChange={(e) => { setCustomStart(Number(e.target.value)); setExportSettingsEdited(true); }}
                  style={{ width: "4.5rem" }}
                />
              </label>
              <label className="film-inline-field">
                <span>End (s)</span>
                <input
                  id="film-export-end"
                  type="number"
                  min={0.1}
                  step={0.1}
                  value={customEnd}
                  onChange={(e) => { setCustomEnd(Number(e.target.value)); setExportSettingsEdited(true); }}
                  style={{ width: "4.5rem" }}
                />
              </label>
            </>
          )}

          <label className="film-inline-field">
            <span>Scale</span>
            <select
              id="film-export-scale"
              value={String(exportScale)}
              onChange={(e) => { setExportScale(Number(e.target.value)); setExportSettingsEdited(true); }}
            >
              <option value="1">100% (Full)</option>
              <option value="0.5">50% (Half)</option>
              <option value="0.25">25% (Quarter)</option>
            </select>
          </label>

          <label className="film-inline-field">
            <span>Quality</span>
            <input
              id="film-export-quality"
              type="number"
              min={0}
              max={100}
              value={exportQuality}
              onChange={(e) => { setExportQuality(Number(e.target.value)); setExportSettingsEdited(true); }}
              style={{ width: "4.5rem" }}
            />
          </label>

          <label className="film-inline-field">
            <span>GPU</span>
            <select
              id="film-export-gpu"
              value={gpuRendering}
              onChange={(e) => { setGpuRendering(e.target.value); setExportSettingsEdited(true); }}
            >
              <option value="auto">Auto (wgpu)</option>
              <option value="off">CPU Reference</option>
            </select>
          </label>

          <label className="film-inline-field">
            <input
              id="film-export-audio"
              type="checkbox"
              checked={includeAudio}
              onChange={(e) => { setIncludeAudio(e.target.checked); setExportSettingsEdited(true); }}
            />
            <span>Audio</span>
          </label>

          <label className="film-inline-field">
            <input
              id="film-allow-offline"
              type="checkbox"
              checked={allowOffline}
              onChange={(e) => { setAllowOffline(e.target.checked); setExportSettingsEdited(true); }}
            />
            <span>Allow Offline</span>
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

          <label className="film-inline-field" title="Add to FilmCraft export queue instead of immediate export">
            <input
              id="film-export-queue"
              type="checkbox"
              checked={queueExport}
              onChange={(e) => { setQueueExport(e.target.checked); setExportSettingsEdited(true); }}
            />
            <span>Queue</span>
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
              onClick={() => void handleExportInterchange("xml")}
              id="film-export-xml"
              title="Export Final Cut Pro 7 XML (xmeml) via filmcraft_engine"
            >
              FCP7 XML
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
            <button
              className="ghost"
              disabled={busy || !film?.timeline?.exists}
              onClick={() => void handleExportInterchange("aaf")}
              id="film-export-aaf"
              title="Export AAF via filmcraft_engine"
            >
              AAF
            </button>
            <button
              className="ghost"
              disabled={busy || !film?.timeline?.exists}
              onClick={() => void handleExportInterchange("omf")}
              id="film-export-omf"
              title="Export OMF 2.0 Audio via filmcraft_engine"
            >
              OMF
            </button>
          </div>
        </div>
      </details>

      <StudioFilmstrips
        project={projectPath}
        timeline={timeline}
        videoSrc={videoSrc}
        revision={film?.timeline?.modified || 0}
        playhead={playhead}
        onTime={setPlayhead}
        videoRef={videoRef}
      />
    </div>
  );
};
