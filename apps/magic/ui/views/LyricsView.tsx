import React, { useEffect, useRef, useState } from "react";
import {
  executeCommand,
  fileUrl,
  WordsAnimation,
  WordsLineItem,
  WordsReadResponse,
} from "../api";

interface LyricsViewProps {
  projectPath: string;
  audioPath?: string;
  onToast: (msg: string) => void;
}

export const LyricsView: React.FC<LyricsViewProps> = ({
  projectPath,
  audioPath,
  onToast,
}) => {
  const [animations, setAnimations] = useState<WordsAnimation[]>([]);
  const [selectedAnimId, setSelectedAnimId] = useState<string>("");
  const [selectedLineIdx, setSelectedLineIdx] = useState<number>(0);
  const [fontSize, setFontSize] = useState<number>(84);
  const [fontColor, setFontColor] = useState<string>("#ffffff");
  const [fontFamily, setFontFamily] = useState<string>("Inter");
  const [textAlign, setTextAlign] = useState<string>("center");
  const [layout, setLayout] = useState<string>("center-stack");
  const [entrance, setEntrance] = useState<string>("per-word-rise");
  const [posX, setPosX] = useState<number>(50);
  const [posY, setPosY] = useState<number>(50);
  const [currentTime, setCurrentTime] = useState<number>(0);
  const [busy, setBusy] = useState<boolean>(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const loadWords = async () => {
    try {
      const res = await executeCommand<WordsReadResponse>("words.read");
      const list = res.animations || [];
      setAnimations(list);
      if (list.length > 0) {
        const target =
          list.find((a) => a.animation_id === selectedAnimId) || list[0];
        selectAnimation(target);
      }
    } catch (err) {
      onToast(String(err));
    }
  };

  const selectAnimation = (anim: WordsAnimation) => {
    setSelectedAnimId(anim.animation_id);
    setSelectedLineIdx(0);
    setFontSize(anim.style?.size ?? 84);
    setFontColor(anim.style?.color ?? "#ffffff");
    setFontFamily(anim.style?.font ?? "Inter");
    setTextAlign(anim.style?.align ?? "center");
    setLayout(anim.style?.layout ?? "center-stack");
    setEntrance(anim.style?.entrance ?? "per-word-rise");
    setPosX(anim.style?.x ?? 50);
    setPosY(anim.style?.y ?? 50);
  };

  useEffect(() => {
    void loadWords();
  }, [projectPath]);

  const currentAnim = animations.find((a) => a.animation_id === selectedAnimId);
  const lines: WordsLineItem[] = currentAnim?.lines || [];
  const activeLine = lines[selectedLineIdx] || lines[0];

  const handleSaveDesign = async () => {
    if (!currentAnim) return;
    setBusy(true);
    try {
      const style = {
        size: fontSize,
        color: fontColor,
        font: fontFamily,
        align: textAlign,
        layout,
        entrance,
        x: posX,
        y: posY,
      };
      await executeCommand("words.save", {
        animation_id: currentAnim.animation_id,
        expected_source_revision: currentAnim.source_revision ?? 0,
        style,
        kinetic: {
          ...(currentAnim.kinetic || {}),
          style,
          layout,
          entrance,
        },
      });
      await executeCommand("words.apply", {
        animation_id: currentAnim.animation_id,
        use: true,
      }).catch(() => {});
      await loadWords();
      onToast(`Saved lyrics design for ${currentAnim.animation_id}`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleAutoDesignAll = async () => {
    setBusy(true);
    try {
      await executeCommand("words.design", {
        apply: true,
        layout,
        entrance,
      });
      await loadWords();
      onToast("Designed all sung lines");
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRenderLettering = async () => {
    setBusy(true);
    try {
      await executeCommand("words.letter", { render: true });
      onToast("Rendered EffectCraft lettering/lyrics.mov with transparency");
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const seekToLine = (line: WordsLineItem, idx: number) => {
    setSelectedLineIdx(idx);
    if (typeof line.start === "number" && audioRef.current) {
      audioRef.current.currentTime = line.start;
      setCurrentTime(line.start);
    }
  };

  return (
    <div
      id="view-design"
      style={{
        display: "grid",
        gridTemplateColumns: "280px 1fr 300px",
        gap: "12px",
        height: "100%",
        padding: "12px",
      }}
    >
      {/* Left Rail: Sections & Timed Lines */}
      <aside
        style={{
          background: "var(--surface-2, #181c22)",
          borderRadius: "8px",
          padding: "12px",
          display: "flex",
          flexDirection: "column",
          gap: "10px",
          overflowY: "auto",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong>Lyrics Sections</strong>
          <button
            id="words-auto-design"
            className="ghost"
            disabled={busy}
            onClick={handleAutoDesignAll}
          >
            Auto-Design
          </button>
        </div>

        {animations.length === 0 ? (
          <p className="muted" style={{ fontSize: "12px" }}>
            No lyric animations yet. Click <strong>Auto-Design</strong> to build
            sections from the song transcript.
          </p>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
            {animations.map((anim) => (
              <button
                key={anim.animation_id}
                id={`words-sec-${anim.animation_id}`}
                className={
                  anim.animation_id === selectedAnimId ? "start" : "ghost"
                }
                style={{ textAlign: "left", justifyContent: "space-between" }}
                onClick={() => selectAnimation(anim)}
              >
                <span>{anim.title || anim.section || anim.animation_id}</span>
                {anim.used_in_video && (
                  <span style={{ fontSize: "11px", opacity: 0.8 }}>● Live</span>
                )}
              </button>
            ))}
          </div>
        )}

        {lines.length > 0 && (
          <>
            <strong style={{ fontSize: "12px", marginTop: "8px" }}>
              Lines in Section ({lines.length})
            </strong>
            <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
              {lines.map((line, idx) => (
                <button
                  key={line.line_id || idx}
                  id={`words-line-${idx}`}
                  className={idx === selectedLineIdx ? "start" : "ghost"}
                  style={{
                    textAlign: "left",
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "flex-start",
                    gap: "2px",
                  }}
                  onClick={() => seekToLine(line, idx)}
                >
                  <span style={{ fontSize: "13px" }}>{line.text}</span>
                  {typeof line.start === "number" && (
                    <span className="muted" style={{ fontSize: "10px" }}>
                      {line.start.toFixed(2)}s – {(line.end ?? line.start).toFixed(2)}s
                    </span>
                  )}
                </button>
              ))}
            </div>
          </>
        )}
      </aside>

      {/* Center: 16:9 Spatial Stage & Audio Preview */}
      <main
        style={{
          background: "var(--surface-1, #12151a)",
          borderRadius: "8px",
          padding: "16px",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "12px",
        }}
      >
        <div
          style={{
            width: "100%",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <strong>
            Spatial Lyrics Stage{" "}
            {currentAnim ? `· ${currentAnim.animation_id}` : ""}
          </strong>
          <div style={{ display: "flex", gap: "8px" }}>
            <button
              id="words-render-lettering"
              className="ghost"
              disabled={busy}
              onClick={handleRenderLettering}
              title="Render lettering/lyrics.mov via EffectCraft"
            >
              Render Lettering (.mov)
            </button>
            <button
              id="words-save-btn"
              className="start"
              disabled={busy || !currentAnim}
              onClick={handleSaveDesign}
            >
              {busy ? "Saving…" : "Save Design"}
            </button>
          </div>
        </div>

        {/* 16:9 Interactive Spatial Canvas */}
        <div
          id="words-spatial-stage"
          onClick={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            const nx = Math.round(((e.clientX - rect.left) / rect.width) * 100);
            const ny = Math.round(((e.clientY - rect.top) / rect.height) * 100);
            setPosX(Math.max(5, Math.min(95, nx)));
            setPosY(Math.max(5, Math.min(95, ny)));
          }}
          style={{
            width: "100%",
            maxWidth: "860px",
            aspectRatio: "16 / 9",
            background:
              "radial-gradient(circle at 50% 40%, #1f2836 0%, #0d1016 100%)",
            border: "1px solid var(--border, #2a313c)",
            borderRadius: "8px",
            position: "relative",
            overflow: "hidden",
            cursor: "crosshair",
            userSelect: "none",
          }}
          title="Click anywhere on the 16:9 stage to position the selected lyric line"
        >
          <div
            style={{
              position: "absolute",
              left: `${posX}%`,
              top: `${posY}%`,
              transform: "translate(-50%, -50%)",
              textAlign: textAlign as React.CSSProperties["textAlign"],
              color: fontColor,
              fontFamily,
              fontSize: `${Math.round(fontSize * 0.42)}px`,
              fontWeight: 700,
              lineHeight: 1.2,
              textShadow: "0 4px 18px rgba(0,0,0,0.75)",
              padding: "8px 14px",
              borderRadius: "6px",
              border: "1px dashed rgba(63, 184, 168, 0.55)",
              maxWidth: "88%",
            }}
          >
            {activeLine?.text ||
              currentAnim?.words?.map((w) => w.text).join(" ") ||
              "Select or design a lyric section"}
          </div>
          <span
            className="muted"
            style={{
              position: "absolute",
              bottom: "8px",
              right: "12px",
              fontSize: "11px",
            }}
          >
            1920×1080 · ({posX}%, {posY}%) · {layout} · {entrance}
          </span>
        </div>

        {/* Synchronized Audio Player */}
        <div
          style={{
            width: "100%",
            display: "flex",
            alignItems: "center",
            gap: "12px",
          }}
        >
          {audioPath ? (
            <audio
              ref={audioRef}
              id="words-audio-player"
              controls
              src={fileUrl(audioPath)}
              onTimeUpdate={(e) => {
                const t = e.currentTarget.currentTime;
                setCurrentTime(t);
                const matchIdx = lines.findIndex(
                  (l) =>
                    typeof l.start === "number" &&
                    typeof l.end === "number" &&
                    t >= l.start &&
                    t <= l.end,
                );
                if (matchIdx >= 0 && matchIdx !== selectedLineIdx) {
                  setSelectedLineIdx(matchIdx);
                }
              }}
              style={{ flex: 1, height: "36px" }}
            />
          ) : (
            <span className="muted" style={{ fontSize: "12px" }}>
              No song audio linked yet.
            </span>
          )}
          <span className="muted" style={{ fontSize: "12px", minWidth: "60px" }}>
            {currentTime.toFixed(2)}s
          </span>
        </div>
      </main>

      {/* Right Inspector: Typography, Spatial Position & Kinetic Motion */}
      <aside
        id="words-inspector"
        style={{
          background: "var(--surface-2, #181c22)",
          borderRadius: "8px",
          padding: "12px",
          display: "flex",
          flexDirection: "column",
          gap: "12px",
          overflowY: "auto",
        }}
      >
        <strong>Typography & Motion Inspector</strong>

        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Font Family</span>
          <select
            id="words-font"
            value={fontFamily}
            onChange={(e) => setFontFamily(e.target.value)}
          >
            <option value="Inter">Inter</option>
            <option value="Montserrat">Montserrat</option>
            <option value="Playfair Display">Playfair Display</option>
            <option value="Space Grotesk">Space Grotesk</option>
            <option value="JetBrains Mono">JetBrains Mono</option>
          </select>
        </label>

        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Font Size ({fontSize}px)</span>
          <input
            id="words-size"
            type="range"
            min={32}
            max={180}
            value={fontSize}
            onChange={(e) => setFontSize(Number(e.target.value))}
          />
        </label>

        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Text Color</span>
          <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
            <input
              id="words-color"
              type="color"
              value={fontColor}
              onChange={(e) => setFontColor(e.target.value)}
            />
            <code>{fontColor}</code>
          </div>
        </label>

        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Alignment</span>
          <select
            id="words-align"
            value={textAlign}
            onChange={(e) => setTextAlign(e.target.value)}
          >
            <option value="left">Left</option>
            <option value="center">Center</option>
            <option value="right">Right</option>
          </select>
        </label>

        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Kinetic Layout</span>
          <select
            id="words-layout"
            value={layout}
            onChange={(e) => setLayout(e.target.value)}
          >
            <option value="center-stack">Center Stack</option>
            <option value="lower-third">Lower Third</option>
            <option value="split-callout">Split Callout</option>
            <option value="diagonal-burst">Diagonal Burst</option>
          </select>
        </label>

        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Entrance Effect</span>
          <select
            id="words-entrance"
            value={entrance}
            onChange={(e) => setEntrance(e.target.value)}
          >
            <option value="per-word-rise">Per-Word Rise</option>
            <option value="typewriter">Typewriter</option>
            <option value="pop-scale">Pop Scale</option>
            <option value="fade-slide">Fade Slide</option>
          </select>
        </label>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
          <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
            <span>Stage X ({posX}%)</span>
            <input
              id="words-pos-x"
              type="number"
              min={0}
              max={100}
              value={posX}
              onChange={(e) => setPosX(Number(e.target.value))}
            />
          </label>
          <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
            <span>Stage Y ({posY}%)</span>
            <input
              id="words-pos-y"
              type="number"
              min={0}
              max={100}
              value={posY}
              onChange={(e) => setPosY(Number(e.target.value))}
            />
          </label>
        </div>

        <button
          id="words-inspector-save"
          className="start"
          disabled={busy || !currentAnim}
          onClick={handleSaveDesign}
        >
          {busy ? "Saving…" : "Save & Apply to Video"}
        </button>
      </aside>
    </div>
  );
};
