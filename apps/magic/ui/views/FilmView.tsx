import React, { useEffect, useRef, useState } from "react";
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

  const totalDuration = Math.max(1, timeline?.duration || 1);

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
        </div>
      </div>

      {/* FilmCraft Export & Interchange Bar */}
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
            onChange={(e) => setExportFormat(e.target.value)}
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
            onChange={(e) => setExportRange(e.target.value)}
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
            onChange={(e) => setBurnCaptions(e.target.checked)}
          />
          <span>Burn Captions</span>
        </label>

        <label style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <span>Sidecar</span>
          <select
            id="film-caption-sidecar"
            value={captionSidecar}
            onChange={(e) => setCaptionSidecar(e.target.value)}
          >
            <option value="">None</option>
            <option value="srt">.srt</option>
            <option value="vtt">.vtt</option>
          </select>
        </label>

        <button
          id="film-export"
          className="start"
          disabled={busy || !film?.timeline?.exists}
          onClick={() =>
            void runFilmAction("Export", () =>
              executeCommand("video.export", {
                format: exportFormat,
                range: exportRange,
                burnCaptions,
                ...(captionSidecar ? { captionSidecar } : {}),
              }),
            )
          }
        >
          Export Film
        </button>

        <div style={{ marginLeft: "auto", display: "flex", gap: "6px" }}>
          <button
            className="ghost"
            disabled={busy || !film?.timeline?.exists}
            onClick={() => void handleExportInterchange("fcpxml")}
            title="Export Final Cut Pro FCPXML via filmcraft_engine"
          >
            FCPXML
          </button>
          <button
            className="ghost"
            disabled={busy || !film?.timeline?.exists}
            onClick={() => void handleExportInterchange("otio")}
            title="Export OpenTimelineIO via filmcraft_engine"
          >
            OTIO
          </button>
          <button
            className="ghost"
            disabled={busy || !film?.timeline?.exists}
            onClick={() => void handleExportInterchange("edl")}
            title="Export CMX3600 EDL via filmcraft_engine"
          >
            EDL
          </button>
        </div>
      </div>

      {/* Video Program Monitor */}
      <div
        style={{
          background: "#0b0d11",
          borderRadius: "8px",
          padding: "12px",
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          minHeight: "340px",
        }}
      >
        <video
          ref={videoRef}
          id="film-player"
          className="film-player"
          controls
          preload="auto"
          src={videoSrc || undefined}
          onTimeUpdate={(e) => setPlayhead(e.currentTarget.currentTime)}
          style={{
            maxWidth: "100%",
            maxHeight: "48vh",
            borderRadius: "6px",
          }}
        />
      </div>

      {/* Interactive Multi-Track Timeline */}
      <div
        id="film-layers"
        aria-label="Film layers"
        style={{
          background: "var(--surface-2, #181c22)",
          borderRadius: "8px",
          padding: "12px",
          display: "flex",
          flexDirection: "column",
          gap: "6px",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", fontSize: "12px" }}>
          <strong>
            Timeline Tracks{" "}
            {film?.timeline?.path ? `(${film.timeline.path.split("/").pop()})` : ""}
          </strong>
          <span className="muted">
            Playhead: {playhead.toFixed(2)}s / {totalDuration.toFixed(2)}s
          </span>
        </div>

        {!timeline || timeline.tracks.length === 0 ? (
          <p className="muted" style={{ fontSize: "12px", margin: "4px 0" }}>
            No tracks loaded yet.
          </p>
        ) : (
          timeline.tracks.map((track, idx) => (
            <div
              key={track.id ?? idx}
              style={{
                display: "grid",
                gridTemplateColumns: "48px 1fr",
                alignItems: "center",
                gap: "8px",
                height: "30px",
              }}
            >
              <span
                className="muted"
                style={{ fontSize: "12px", fontWeight: 600 }}
              >
                {track.label}
              </span>
              <div
                style={{
                  position: "relative",
                  height: "26px",
                  background: "var(--surface-1, #12151a)",
                  borderRadius: "4px",
                  overflow: "hidden",
                }}
              >
                {track.clips.map((c, cIdx) => {
                  const leftPct = (c.start / totalDuration) * 100;
                  const widthPct = Math.max(
                    0.5,
                    ((c.end - c.start) / totalDuration) * 100,
                  );
                  return (
                    <button
                      key={cIdx}
                      title={`${c.name} (${c.start.toFixed(2)}s – ${c.end.toFixed(2)}s)`}
                      onClick={() => {
                        if (videoRef.current) {
                          videoRef.current.currentTime = c.start;
                        }
                        setPlayhead(c.start);
                      }}
                      style={{
                        position: "absolute",
                        left: `${leftPct}%`,
                        width: `${widthPct}%`,
                        top: "2px",
                        bottom: "2px",
                        background:
                          track.kind === "audio"
                            ? "var(--surface-3, #2a3340)"
                            : "var(--accent, #3fb8a8)",
                        opacity: c.enabled === false ? 0.35 : 0.85,
                        border: "none",
                        borderRadius: "3px",
                        color: "#fff",
                        fontSize: "11px",
                        padding: "0 6px",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                        cursor: "pointer",
                        textAlign: "left",
                      }}
                    >
                      {c.name}
                    </button>
                  );
                })}
                {/* Playhead indicator */}
                <div
                  style={{
                    position: "absolute",
                    left: `${Math.min(100, (playhead / totalDuration) * 100)}%`,
                    top: 0,
                    bottom: 0,
                    width: "2px",
                    background: "var(--warn, #ffb020)",
                    pointerEvents: "none",
                  }}
                />
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
