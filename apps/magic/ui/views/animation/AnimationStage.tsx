import React from "react";
import { AnimationStyle, CharacterPart, CharacterSpriteSummary, executeCommand, fileUrl } from "../../api";

export type SpriteAction = CharacterSpriteSummary;

export interface BoneNode {
  id: string;
  label: string;
  x: number;
  y: number;
  parent: string | null;
  outline?: Array<[number, number]>;
}

interface AnimationStageProps {
  selectedCharId: string;
  selectedView: string;
  animationStyle: AnimationStyle;
  selectedSprite: SpriteAction | null;
  activeActionName: string;
  sprites: SpriteAction[];
  onSelectSprite: (sprite: SpriteAction | null) => void;
  showMarks: boolean;
  onToggleMarks: () => void;
  penActive: boolean;
  onTogglePen: () => void;
  onAddPathPoint: (point: [number, number]) => void;
  showPath: boolean;
  onTogglePath: () => void;
  showGrid: boolean;
  onToggleGrid: () => void;
  show3D: boolean;
  onToggle3D: () => void;
  zoomLevel: number;
  onChangeZoom: (zoom: number) => void;
  framePath: string;
  drawingSize: [number, number];
  parts: CharacterPart[];
  bones: BoneNode[];
  selectedBone: string;
  onSelectBone: (boneId: string) => void;
  pathPoints: Array<[number, number]>;
  pathPointIdx: number;
  onSelectPathPoint: (idx: number) => void;
  pathX: number;
  onChangePathX: (x: number) => void;
  pathY: number;
  onChangePathY: (y: number) => void;
  pathDuration: number;
  onChangePathDuration: (dur: number) => void;
  sizeStart: number;
  onChangeSizeStart: (val: number) => void;
  sizeEnd: number;
  onChangeSizeEnd: (val: number) => void;
  onClearPath: () => void;
  onUndoPath: () => void;
  onApplyPath: () => void;
  isPlaying: boolean;
  onTogglePlay: () => void;
  currentTime: number;
  duration: number;
  onSeek: (time: number) => void;
  onOpenFullReview: () => void;
}

export const AnimationStage: React.FC<AnimationStageProps> = ({
  selectedCharId,
  selectedView,
  animationStyle,
  selectedSprite,
  activeActionName,
  showMarks,
  onToggleMarks,
  penActive,
  onTogglePen,
  onAddPathPoint,
  showPath,
  onTogglePath,
  showGrid,
  onToggleGrid,
  show3D,
  onToggle3D,
  zoomLevel,
  onChangeZoom,
  framePath,
  drawingSize,
  parts,
  bones,
  selectedBone,
  onSelectBone,
  pathPoints,
  pathPointIdx,
  onSelectPathPoint,
  pathX,
  onChangePathX,
  pathY,
  onChangePathY,
  pathDuration,
  onChangePathDuration,
  sizeStart,
  onChangeSizeStart,
  sizeEnd,
  onChangeSizeEnd,
  onClearPath,
  onUndoPath,
  onApplyPath,
  isPlaying,
  onTogglePlay,
  currentTime,
  duration,
  onSeek,
  onOpenFullReview,
}) => {
  const formatTimecode = (secs: number) => {
    const mins = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    const ms = Math.floor((secs % 1) * 1000);
    return `${mins}:${String(s).padStart(2, "0")}.${String(ms).padStart(3, "0")}`;
  };

  // Resolve the active image for the stage:
  // 1. If a sprite cycle is selected and has PNG frames, pick the frame corresponding to currentTime
  //    when paused/scrubbing, or the animated GIF preview when playing (or cycle frames directly).
  // 2. Otherwise show the rigged view's frame or initial cutout drawing.
  let stageImageSrc = "";
  if (selectedSprite) {
    if (selectedSprite.frames && selectedSprite.frames.length > 0) {
      const cycDur = selectedSprite.duration || duration || 1.0;
      const norm = cycDur > 0 ? ((currentTime % cycDur) + cycDur) % cycDur / cycDur : 0;
      const idx = Math.min(
        selectedSprite.frames.length - 1,
        Math.floor(norm * selectedSprite.frames.length)
      );
      stageImageSrc = fileUrl(selectedSprite.frames[idx]);
    } else if (selectedSprite.preview) {
      stageImageSrc = fileUrl(selectedSprite.preview);
    }
  }
  if (!stageImageSrc && framePath) {
    stageImageSrc = fileUrl(framePath.split("?")[0]);
  }

  const [docW, docH] = drawingSize[0] > 0 && drawingSize[1] > 0 ? drawingSize : [520, 520];
  const captionLabel = selectedSprite?.name || activeActionName || "Rest";
  const styleBadge =
    animationStyle === "sprite_2d"
      ? "2D Sprite"
      : animationStyle === "rig_3d"
        ? "3D Rig"
        : "2.5D Rig";

  return (
    <div className="motion-main">
      <div id="motion-stagebar" className="panel-head" data-look="bar">
        <span id="motion-caption" className="title">
          {selectedCharId} · {styleBadge} · {captionLabel}
        </span>

        <div className="panel-actions">
          <button
            id="motion-full-review"
            type="button"
            className="ghost"
            title="Inspect drawings, sprite sheets and motion clips"
            onClick={onOpenFullReview}
          >
            Drawing / motion review
          </button>
          <button
            id="motion-toggle-3d"
            type="button"
            className="ghost"
            aria-pressed={show3D}
            title="Toggle 3D viewport"
            onClick={onToggle3D}
          >
            3D View
          </button>
          <button
            id="motion-marks"
            type="button"
            className="icon"
            aria-pressed={showMarks}
            title="Show / edit part outlines & joints"
            aria-label="Edit part marks"
            onClick={onToggleMarks}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <circle cx="12" cy="5" r="2.5" />
              <circle cx="5" cy="19" r="2.5" />
              <circle cx="19" cy="19" r="2.5" />
              <path d="M12 7.5v5M12 12.5l-5.5 4.5M12 12.5l5.5 4.5" />
            </svg>
          </button>
          <button
            id="motion-pen"
            type="button"
            className="icon"
            aria-pressed={penActive}
            title="Pen · draw a motion path"
            aria-label="Pen · draw a motion path"
            onClick={onTogglePen}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M12 20h9" />
              <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
            </svg>
          </button>
          <button
            id="motion-path-visible"
            type="button"
            className="icon"
            aria-pressed={showPath}
            title="Toggle motion path overlay"
            aria-label="Show motion path"
            onClick={onTogglePath}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          </button>
          <button
            id="motion-grid"
            type="button"
            className="icon"
            aria-pressed={showGrid}
            title="Show stage grid"
            aria-label="Show stage grid"
            onClick={onToggleGrid}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <path d="M3 9h18M3 15h18M9 3v18M15 3v18" />
            </svg>
          </button>
          <button
            id="motion-fit"
            type="button"
            className="icon"
            title="Fit to stage"
            aria-label="Fit to stage"
            onClick={() => {
              const canvas = document.getElementById("motion-canvas");
              const subject = canvas?.querySelector<HTMLElement>(".anim-stage-subject");
              if (!canvas || !subject?.offsetWidth || !subject.offsetHeight) return;
              const fit = Math.min(canvas.clientWidth / subject.offsetWidth, canvas.clientHeight / subject.offsetHeight);
              onChangeZoom(Math.max(0.4, Math.min(2.5, Number(fit.toFixed(2)))));
            }}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
            </svg>
          </button>
          <button
            id="motion-actual"
            type="button"
            className="ghost"
            title="Actual size (1:1)"
            onClick={() => onChangeZoom(1)}
          >
            1:1
          </button>
          <button
            id="motion-minus"
            type="button"
            className="icon"
            title="Zoom out"
            aria-label="Zoom out"
            onClick={() => onChangeZoom(Math.max(0.4, Number((zoomLevel - 0.15).toFixed(2))))}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M5 12h14" />
            </svg>
          </button>
          <button
            id="motion-plus"
            type="button"
            className="icon"
            title="Zoom in"
            aria-label="Zoom in"
            onClick={() => onChangeZoom(Math.min(2.5, Number((zoomLevel + 0.15).toFixed(2))))}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12 5v14M5 12h14" />
            </svg>
          </button>
          <button
            id="btn-generate-all-sheets"
            type="button"
            className="icon"
            title="Open in EffectCraft"
            aria-label="Open in EffectCraft"
            onClick={() =>
              void executeCommand("character.open", {
                character: selectedCharId,
                view: selectedView,
                app: "effectcraft",
              })
            }
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="m12 2 9 5-9 5-9-5 9-5z" />
              <path d="m3 12 9 5 9-5" />
              <path d="m3 17 9 5 9-5" />
            </svg>
          </button>
        </div>
      </div>

      <div id="motion-stage" className="motion-stage">
        <div
          id="motion-canvas"
          className={`seq-canvas ${showGrid ? "grid" : ""}`}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <div
            className="anim-stage-subject"
            style={{ "--stage-zoom": zoomLevel } as React.CSSProperties}
          >
            {stageImageSrc ? (
              <img
                src={stageImageSrc}
                alt={`${selectedCharId} ${captionLabel}`}
                style={{
                  position: "static",
                  transform: "none",
                  maxHeight: "74vh",
                  height: "390px",
                  width: "auto",
                  objectFit: "contain",
                  cursor: showMarks ? "crosshair" : "default",
                }}
              />
            ) : (
              <div className="panel-empty">
                <p className="muted">Select an action or character asset to preview on stage.</p>
              </div>
            )}

            {(showMarks || showPath) && (
              <svg
                className="anim-skeleton-svg"
                viewBox={`0 0 ${docW} ${docH}`}
                preserveAspectRatio="xMidYMid meet"
                style={{ cursor: penActive ? "crosshair" : undefined }}
                onClick={(event) => {
                  if (!penActive) return;
                  const toDrawing = event.currentTarget.getScreenCTM()?.inverse();
                  if (!toDrawing) return;
                  const point = new DOMPoint(event.clientX, event.clientY).matrixTransform(toDrawing);
                  onAddPathPoint([Math.round(point.x), Math.round(point.y)]);
                }}
              >
                {showPath && pathPoints.length > 0 && (
                  <g>
                    <polyline
                      fill="none"
                      stroke="var(--depth-gold)"
                      strokeWidth={2.5}
                      strokeDasharray="6 4"
                      points={pathPoints.map((pt) => `${pt[0]},${pt[1]}`).join(" ")}
                    />
                    {pathPoints.map((pt, idx) => {
                      const isSelPt = pathPointIdx === idx;
                      return (
                        <g
                          key={idx}
                          style={{ cursor: "pointer" }}
                          onClick={(event) => {
                            event.stopPropagation();
                            onSelectPathPoint(idx);
                          }}
                        >
                          <circle
                            cx={pt[0]}
                            cy={pt[1]}
                            r={isSelPt ? 7 : 5}
                            fill={isSelPt ? "var(--amber)" : "var(--depth-gold)"}
                            stroke="var(--bg)"
                            strokeWidth={2}
                          />
                          <text
                            x={pt[0] + 10}
                            y={pt[1] + 4}
                            fill={isSelPt ? "var(--amber)" : "var(--muted)"}
                            fontSize="10"
                            fontFamily="ui-monospace, monospace"
                            fontWeight={isSelPt ? "bold" : "normal"}
                          >
                            Pt {idx + 1}
                          </text>
                        </g>
                      );
                    })}
                  </g>
                )}

                {showMarks &&
                  parts.map((part) => {
                    if (!part.outline || part.outline.length < 3) return null;
                    const isSel = selectedBone === part.id;
                    return (
                      <polygon
                        key={`outline-${part.id}`}
                        points={part.outline.map((pt) => `${pt[0]},${pt[1]}`).join(" ")}
                        fill={isSel ? "color-mix(in srgb, var(--amber) 14%, transparent)" : "none"}
                        stroke={isSel ? "var(--amber)" : "var(--tone-animate)"}
                        strokeWidth={isSel ? 2.5 : 1.5}
                        strokeDasharray={isSel ? "none" : "4 3"}
                        onClick={() => onSelectBone(part.id)}
                        style={{ cursor: "pointer" }}
                      />
                    );
                  })}

                {showMarks &&
                  bones.map((b) => {
                    if (!b.parent) return null;
                    const p = bones.find((x) => x.id === b.parent);
                    if (!p) return null;
                    const isSel = selectedBone === b.id;
                    return (
                      <line
                        key={`bone-${p.id}-${b.id}`}
                        x1={p.x}
                        y1={p.y}
                        x2={b.x}
                        y2={b.y}
                        stroke={isSel ? "var(--amber)" : "var(--accent)"}
                        strokeWidth={isSel ? 3 : 2}
                      />
                    );
                  })}

                {showMarks &&
                  bones.map((b) => {
                    const isSel = selectedBone === b.id;
                    return (
                      <g
                        key={`joint-${b.id}`}
                        className="anim-joint-node"
                        onClick={() => onSelectBone(b.id)}
                      >
                        <circle
                          cx={b.x}
                          cy={b.y}
                          r={isSel ? 7 : 5}
                          fill={isSel ? "var(--amber)" : "var(--surface-2)"}
                          stroke={isSel ? "var(--text)" : "var(--accent)"}
                          strokeWidth={2}
                        />
                        <text
                          x={b.x + 9}
                          y={b.y + 4}
                          fill={isSel ? "var(--text)" : "var(--muted)"}
                          fontSize="11"
                          fontFamily="ui-monospace, monospace"
                          fontWeight={isSel ? "bold" : "normal"}
                        >
                          {b.label}
                        </text>
                      </g>
                    );
                  })}
              </svg>
            )}
          </div>
        </div>

        {/* Floating Direct-Manipulation Spline Path & Timing HUD Pill Bar */}
        {showPath && (
          <div id="motion-path-hud" className="motion-path-hud" role="toolbar" aria-label="Spline path and timing">
            <select
              id="hud-path-point"
              className="motion-hud-select"
              value={pathPointIdx}
              aria-label="Active path point"
              onChange={(e) => onSelectPathPoint(Number(e.target.value))}
            >
              <option value={0}>Pt 1 (Start)</option>
              <option value={1}>Pt 2 (Mid)</option>
              <option value={2}>Pt 3 (End)</option>
            </select>

            <label className="motion-hud-field">
              <span>X</span>
              <input
                id="hud-path-x"
                type="number"
                value={pathX}
                onChange={(e) => onChangePathX(Number(e.target.value))}
              />
            </label>
            <label className="motion-hud-field">
              <span>Y</span>
              <input
                id="hud-path-y"
                type="number"
                value={pathY}
                onChange={(e) => onChangePathY(Number(e.target.value))}
              />
            </label>

            <span className="motion-hud-sep" aria-hidden="true" />

            <label className="motion-hud-field">
              <span>Dur</span>
              <input
                id="hud-path-duration"
                type="number"
                step={0.1}
                min={0.2}
                max={12}
                value={pathDuration}
                onChange={(e) => onChangePathDuration(Number(e.target.value))}
              />
              <small>s</small>
            </label>

            <label className="motion-hud-field">
              <span>Scale</span>
              <input
                id="hud-size-start"
                type="number"
                step={5}
                min={20}
                max={220}
                value={sizeStart}
                onChange={(e) => onChangeSizeStart(Number(e.target.value))}
              />
              <small>→</small>
              <input
                id="hud-size-end"
                type="number"
                step={5}
                min={20}
                max={220}
                value={sizeEnd}
                onChange={(e) => onChangeSizeEnd(Number(e.target.value))}
              />
              <small>%</small>
            </label>

            <span className="motion-hud-sep" aria-hidden="true" />

            <button
              id="hud-path-undo"
              type="button"
              className="ghost motion-hud-btn"
              onClick={onUndoPath}
            >
              Undo
            </button>
            <button
              id="hud-path-clear"
              type="button"
              className="ghost motion-hud-btn"
              onClick={onClearPath}
            >
              Clear
            </button>
            <button
              id="hud-path-apply"
              type="button"
              className="start motion-hud-btn"
              onClick={onApplyPath}
            >
              Apply Path
            </button>
          </div>
        )}

        <div id="motion-canvas-3d" className="tripo-3d-viewport" hidden={!show3D}>
          {show3D && (
            <div className="panel-empty" style={{ height: "100%" }}>
              <strong>3D Model Viewport ({selectedCharId})</strong>
              <p className="muted">
                Use Part artwork → Skeleton and motion to inspect or rig GLB/FBX clips, or open in
                Blender.
              </p>
            </div>
          )}
        </div>
      </div>

      <div className="motion-transport">
        <button
          id="motion-play"
          type="button"
          className="icon"
          title={isPlaying ? "Pause" : "Play"}
          aria-label={isPlaying ? "Pause" : "Play"}
          onClick={onTogglePlay}
        >
          {isPlaying ? (
            <svg viewBox="0 0 24 24" fill="currentColor">
              <rect x="6" y="5" width="4" height="14" rx="1" />
              <rect x="14" y="5" width="4" height="14" rx="1" />
            </svg>
          ) : (
            <svg viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          )}
        </button>
        <button
          id="motion-rewind"
          type="button"
          className="icon"
          title="Rewind to start"
          aria-label="Rewind"
          onClick={() => onSeek(0)}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
          </svg>
        </button>
        <button
          id="motion-step-back"
          type="button"
          className="icon"
          title="Previous frame"
          aria-label="Previous frame"
          onClick={() => onSeek(Math.max(0, Number((currentTime - 1 / 24).toFixed(3))))}
        >
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M6 6h2v12H6zm3.5 6 8.5 6V6z" />
          </svg>
        </button>
        <button
          id="motion-step"
          type="button"
          className="icon"
          title="Next frame"
          aria-label="Next frame"
          onClick={() => onSeek(Math.min(duration, Number((currentTime + 1 / 24).toFixed(3))))}
        >
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z" />
          </svg>
        </button>
        <span id="motion-time" className="motion-time">
          {formatTimecode(currentTime)}
        </span>
        <input
          id="motion-scrub"
          type="range"
          min={0}
          max={Math.max(0.1, duration)}
          step={0.01}
          value={Math.min(currentTime, Math.max(0.1, duration))}
          aria-label="Scrub motion"
          onChange={(e) => onSeek(Number(e.target.value))}
        />
      </div>
    </div>
  );
};
