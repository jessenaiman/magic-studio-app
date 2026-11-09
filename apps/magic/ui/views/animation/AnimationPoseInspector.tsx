import React, { useState } from "react";
import {
  AnimationStyle,
  Character3DClip,
  CharacterMetaResponse,
  CharacterPart,
  CharacterSpriteSummary,
  CharacterViewSummary,
  executeCommand,
  fileUrl,
} from "../../api";
import { Dial, Slider, Toggle } from "../../components/controls";

interface AnimationPoseInspectorProps {
  selectedCharId: string;
  selectedView: string;
  animationStyle: AnimationStyle;
  onSelectView: (view: string) => void;
  onChangeAnimationStyle: (style: AnimationStyle) => void;
  selectedBone: string;
  onSelectBone: (bone: string) => void;
  meta: CharacterMetaResponse | null;
  sprites: CharacterSpriteSummary[];
  selectedSprite: CharacterSpriteSummary | null;
  onSelectSprite: (sprite: CharacterSpriteSummary | null) => void;
  activeActionName: string;
  onTriggerAction: (actionName: string) => void;
  showMarks: boolean;
  onToggleMarks: () => void;
  onRefreshCharacter: () => Promise<void>;
  pathPointIdx: number;
  onSelectPathPoint: (idx: number) => void;
  pathX: number;
  onChangePathX: (val: number) => void;
  pathY: number;
  onChangePathY: (val: number) => void;
  pathDuration: number;
  onChangePathDuration: (val: number) => void;
  sizeStart: number;
  onChangeSizeStart: (val: number) => void;
  sizeEnd: number;
  onChangeSizeEnd: (val: number) => void;
  isLooping: boolean;
  onToggleLoop: (val: boolean) => void;
  playbackSpeed: number;
  onChangePlaybackSpeed: (val: number) => void;
  motionAmount: number;
  onChangeMotionAmount: (val: number) => void;
  rotation: number;
  onChangeRotation: (val: number) => void;
  onCommitRotation: (val: number) => void;
  scale: number;
  onChangeScale: (val: number) => void;
  onCommitScale: (val: number) => void;
  opacity: number;
  onChangeOpacity: (val: number) => void;
  onCommitOpacity: (val: number) => void;
  onKeyTransform: () => void;
  ikActive: boolean;
  onChangeIkActive: (val: boolean) => void;
  limbChain: string;
  onChangeLimbChain: (val: string) => void;
  poleBendClockwise: boolean;
  onChangePoleBend: (val: boolean) => void;
  torsoSecondaryFollow: boolean;
  onChangeTorsoFollow: (val: boolean) => void;
  ikTargetAngle: number;
  onChangeIkTargetAngle: (val: number) => void;
  onCommitIkTargetAngle: (val: number) => void;
  onSolveIK: () => void;
  meshDensity: number;
  onChangeMeshDensity: (val: number) => void;
  meshExpansion: number;
  onChangeMeshExpansion: (val: number) => void;
  pinMode: "deform" | "starch" | "overlap";
  onChangePinMode: (mode: "deform" | "starch" | "overlap") => void;
  pinStiffness: number;
  onChangePinStiffness: (val: number) => void;
  pinExtent: number;
  onChangePinExtent: (val: number) => void;
  followDelay: number;
  onChangeFollowDelay: (val: number) => void;
  handHold: boolean;
  onToggleHandHold: () => void;
  faceHold: boolean;
  onToggleFaceHold: () => void;
  selectedGesture: string;
  onSelectGesture: (gesture: string) => void;
  gestureAngle: number;
  onChangeGestureAngle: (angle: number) => void;
  onCommitGestureAngle: (angle: number) => void;
  onRenderClip: () => void;
  placeTrack: string;
  onChangePlaceTrack: (val: string) => void;
  placeTime: number;
  onChangePlaceTime: (val: number) => void;
  lastRenderedClip: string;
  onPlaceOnTimeline: () => void;
  busy: boolean;
  onToast: (msg: string) => void;
}

const STANDARD_ACTIONS = [
  { id: "walk_left", btnId: "motion-action-left", label: "Left" },
  { id: "walk_right", btnId: "motion-action-right", label: "Right" },
  { id: "walk_up", btnId: "motion-action-up", label: "Up" },
  { id: "walk_down", btnId: "motion-action-down", label: "Down" },
  { id: "idle", btnId: "motion-action-idle", label: "Idle" },
  { id: "walk_down", btnId: "motion-action-walk", label: "Walk" },
  { id: "jump", btnId: "motion-action-jump", label: "Jump" },
  { id: "turn", btnId: "motion-action-sway", label: "Turn" },
  { id: "wave", btnId: "motion-action-wave", label: "Wave" },
  { id: "rest", btnId: "motion-action-rest", label: "Rest" },
] as const;

export const AnimationPoseInspector: React.FC<AnimationPoseInspectorProps> = ({
  selectedCharId,
  selectedView,
  animationStyle,
  onSelectView,
  onChangeAnimationStyle,
  selectedBone,
  onSelectBone,
  meta,
  sprites,
  selectedSprite,
  onSelectSprite,
  activeActionName,
  onTriggerAction,
  onRefreshCharacter,
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
  isLooping,
  onToggleLoop,
  playbackSpeed,
  onChangePlaybackSpeed,
  motionAmount,
  onChangeMotionAmount,
  rotation,
  onChangeRotation,
  onCommitRotation,
  scale,
  onChangeScale,
  onCommitScale,
  opacity,
  onChangeOpacity,
  onCommitOpacity,
  onKeyTransform,
  ikActive,
  onChangeIkActive,
  limbChain,
  onChangeLimbChain,
  poleBendClockwise,
  onChangePoleBend,
  torsoSecondaryFollow,
  onChangeTorsoFollow,
  ikTargetAngle,
  onChangeIkTargetAngle,
  onCommitIkTargetAngle,
  onSolveIK,
  meshDensity,
  onChangeMeshDensity,
  meshExpansion,
  onChangeMeshExpansion,
  pinMode,
  onChangePinMode,
  pinStiffness,
  onChangePinStiffness,
  pinExtent,
  onChangePinExtent,
  followDelay,
  onChangeFollowDelay,
  handHold,
  onToggleHandHold,
  faceHold,
  onToggleFaceHold,
  selectedGesture,
  onSelectGesture,
  gestureAngle,
  onChangeGestureAngle,
  onCommitGestureAngle,
  onRenderClip,
  placeTrack,
  onChangePlaceTrack,
  placeTime,
  onChangePlaceTime,
  lastRenderedClip,
  onPlaceOnTimeline,
  busy,
  onToast,
}) => {
  // Layers state
  const [layerDepth, setLayerDepth] = useState<number>(0);
  const [layerReason, setLayerReason] = useState<string>("");
  const [hiddenParts, setHiddenParts] = useState<Record<string, boolean>>({});

  // Part artwork drawer tab
  const [partsTab, setPartsTab] = useState<"parts" | "compare">("parts");

  // Part coordinate state
  const [partPosX, setPartPosX] = useState<number>(0);
  const [partPosY, setPartPosY] = useState<number>(0);
  const [partScaleX, setPartScaleX] = useState<number>(100);
  const [partScaleY, setPartScaleY] = useState<number>(100);
  const [partPivotX, setPartPivotX] = useState<number>(0);
  const [partPivotY, setPartPivotY] = useState<number>(0);

  // Action authoring & scene placement
  const [actionLabel, setActionLabel] = useState<string>(
    selectedSprite?.name || activeActionName || "idle"
  );
  const [actionDuration, setActionDuration] = useState<number>(2.0);
  const [placeRate, setPlaceRate] = useState<number>(1.0);
  const [placeX, setPlaceX] = useState<number>(960);
  const [placeY, setPlaceY] = useState<number>(540);
  const [placeW, setPlaceW] = useState<number>(520);
  const [placeH, setPlaceH] = useState<number>(520);
  const [poseRef, setPoseRef] = useState<string>("rest");

  const activeViewSummary: CharacterViewSummary | undefined =
    meta?.views?.find((v) => v.view === selectedView) || meta?.views?.[0];
  const parts: CharacterPart[] = activeViewSummary?.parts || meta?.parts || [];
  const activePart = parts.find((p) => p.id === selectedBone) || parts[0];

  const isMarked = Boolean(activeViewSummary?.marked || parts.length > 0);
  const isCut = Boolean(activeViewSummary?.cut);
  const isRigged = Boolean(activeViewSummary?.rigged || activeViewSummary?.ecproj);
  const hasSpritesOr3D = Boolean(
    sprites.length > 0 || (meta?.tripo_3d && (meta.tripo_3d as { available?: boolean }).available)
  );
  const passedChecks = [isMarked, isCut, isRigged, hasSpritesOr3D].filter(Boolean).length;

  const clips3D: Character3DClip[] = meta?.tripo_3d?.clips || [];
  const modelClipNames: string[] = meta?.tripo_3d?.model_clips || [];
  const drawnViews = (meta?.views || []).filter((v) => Boolean(v.drawing || v.parts?.length || v.ecproj));
  const hasMultipleDrawnViews = drawnViews.length > 1;
  const sprite2DCycles = sprites.filter((s) => s.animation_style === "sprite_2d");
  const rig25DCycles = sprites.filter((s) => s.animation_style !== "sprite_2d");

  const runPrepareStep = async (cmd: string, label: string) => {
    try {
      onToast(`${label} (${selectedCharId} · ${selectedView})…`);
      await executeCommand(cmd, { character: selectedCharId, view: selectedView });
      await onRefreshCharacter();
      onToast(`${label} complete`);
    } catch (err) {
      onToast(String(err));
    }
  };

  const runAssembleBlender = async (openGui: boolean) => {
    try {
      onToast(
        `${openGui ? "Opening 2.5D/3D rig in Blender" : "Assembling 2.5D/3D pieces in Blender"} (${selectedCharId})…`
      );
      await executeCommand("scribbles.blender", {
        character: selectedCharId,
        view: selectedView,
        open: openGui,
      });
      await onRefreshCharacter();
      onToast(
        openGui
          ? `Opened ${selectedCharId} 2.5D/3D rig in Blender`
          : `Assembled ${selectedCharId} 2.5D/3D rig (.blend + .glb)`
      );
    } catch (err) {
      onToast(String(err));
    }
  };

  return (
    <aside
      id="motion-inspector"
      className="motion-inspector"
      data-ui-config="motion-inspector"
      data-animation-style={animationStyle}
      aria-label="Animation controls"
    >
      {/* 0. ANIMATION STYLE BAR (2D Sprite vs 2.5D Layered Rig vs 3D Model Animations) */}
      <div
        id="anim-style-bar"
        className="panel"
        style={{ padding: "var(--space-2) var(--space-3)", marginBottom: "var(--space-2)" }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: "6px",
          }}
        >
          <span className="title" style={{ fontSize: "var(--text-xs)", fontWeight: 600 }}>
            Animation Style
          </span>
          <span id="anim-style-badge" className="meta">
            {animationStyle === "sprite_2d"
              ? "2D Sprite Sheet"
              : animationStyle === "rig_3d"
                ? "3D Model Animations"
                : "2.5D Cutout Rig"}
          </span>
        </div>
        <div className="segmented" role="tablist" aria-label="Character animation style">
          <button
            id="style-tab-sprite-2d"
            type="button"
            role="tab"
            aria-selected={animationStyle === "sprite_2d"}
            className={animationStyle === "sprite_2d" ? "active" : ""}
            onClick={() => onChangeAnimationStyle("sprite_2d")}
          >
            2D Sprite
          </button>
          <button
            id="style-tab-cutout-2-5d"
            type="button"
            role="tab"
            aria-selected={animationStyle === "cutout_2_5d" || animationStyle === "cutout"}
            className={
              animationStyle === "cutout_2_5d" || animationStyle === "cutout" ? "active" : ""
            }
            onClick={() => onChangeAnimationStyle("cutout_2_5d")}
          >
            2.5D Rig
          </button>
          <button
            id="style-tab-rig-3d"
            type="button"
            role="tab"
            aria-selected={animationStyle === "rig_3d"}
            className={animationStyle === "rig_3d" ? "active" : ""}
            onClick={() => onChangeAnimationStyle("rig_3d")}
          >
            3D Anim
          </button>
        </div>
      </div>

      {/* 1. ACTION (closed drawer by default per A3) */}
      <details id="motion-action" className="panel motion-section" data-look="motion">
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          </span>
          <span className="title">Action</span>
          <span id="motion-action-meta" className="meta">
            {selectedSprite?.name || activeActionName || "Rest"}
          </span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div className="motion-actions-grid" role="group" aria-label="Character actions">
            {STANDARD_ACTIONS.map((act) => {
              const spriteMatch = sprites.find((s) => s.name === act.id);
              const isPressed =
                act.id === "rest"
                  ? !selectedSprite && activeActionName === "rest"
                  : selectedSprite?.name === act.id || activeActionName === act.id;
              return (
                <button
                  key={act.btnId}
                  id={act.btnId}
                  type="button"
                  className="motion-action"
                  aria-pressed={isPressed}
                  onClick={() => {
                    if (act.id === "rest") {
                      onSelectSprite(null);
                      onTriggerAction("rest");
                    } else if (spriteMatch) {
                      onSelectSprite(spriteMatch);
                    } else {
                      onSelectSprite(null);
                      onTriggerAction(act.id);
                    }
                  }}
                >
                  <span>{act.label}</span>
                </button>
              );
            })}
            {sprites
              .filter((s) => !STANDARD_ACTIONS.some((a) => a.id === s.name))
              .map((s) => {
                const isPressed = selectedSprite?.name === s.name;
                return (
                  <button
                    key={`sprite-${s.name}`}
                    id={`anim-sprite-${s.name}`}
                    type="button"
                    className="motion-action"
                    aria-pressed={isPressed}
                    onClick={() => onSelectSprite(s)}
                  >
                    <span>{s.name}</span>
                  </button>
                );
              })}
          </div>

          <div id="char-magic-panel" className="char-magic-panel">
            {animationStyle === "rig_3d" ? (
              <>
                <div className="char-magic-head">
                  <strong className="char-subhead">3D Model Animations</strong>
                  <span id="char-magic-badge" className="char-magic-badge">
                    {clips3D.length || modelClipNames.length} clips
                  </span>
                </div>
                <p className="motion-help" style={{ margin: "4px 0 8px" }}>
                  3D models play skeletal animation clips (.glb / .blend) instead of 2D sprite sheets.
                </p>
                <div className="char-sheets-row">
                  {clips3D.map((clip) => {
                    const active = activeActionName === clip.id;
                    return (
                      <button
                        key={clip.id}
                        id={`anim-3d-clip-${clip.id}`}
                        type="button"
                        className={`char-sheet-card ${active ? "is-active" : ""}`}
                        onClick={() => {
                          onSelectSprite(null);
                          onTriggerAction(clip.id);
                        }}
                      >
                        <span>{clip.name || clip.id}</span>
                      </button>
                    );
                  })}
                  {clips3D.length === 0 &&
                    modelClipNames.map((name) => {
                      const active = activeActionName === name;
                      return (
                        <button
                          key={name}
                          type="button"
                          className={`char-sheet-card ${active ? "is-active" : ""}`}
                          onClick={() => {
                            onSelectSprite(null);
                            onTriggerAction(name);
                          }}
                        >
                          <span>{name}</span>
                        </button>
                      );
                    })}
                </div>
                <div className="motion-path-actions" style={{ marginTop: "8px" }}>
                  <button
                    id="btn-assemble-3d-blender"
                    type="button"
                    className="ghost"
                    disabled={busy}
                    onClick={() => void runAssembleBlender(false)}
                  >
                    Assemble 2.5D/3D Rig
                  </button>
                  <button
                    id="btn-open-3d-blender"
                    type="button"
                    className="start"
                    disabled={busy}
                    onClick={() => void runAssembleBlender(true)}
                  >
                    Open in Blender…
                  </button>
                </div>
              </>
            ) : animationStyle === "sprite_2d" ? (
              <>
                <div className="char-magic-head">
                  <strong className="char-subhead">2D Directional Sprite Sheets</strong>
                  <span id="char-magic-badge" className="char-magic-badge">
                    {sprite2DCycles.length || sprites.length} sheets
                  </span>
                </div>
                <p className="motion-help" style={{ margin: "4px 0 8px" }}>
                  2D sprite characters play directional frame sheets (no 3D turnaround views).
                </p>
                {(sprite2DCycles.length > 0 ? sprite2DCycles : sprites).length > 0 && (
                  <div className="char-sheets-row">
                    {(sprite2DCycles.length > 0 ? sprite2DCycles : sprites).map((sp) => {
                      const thumb = sp.frames?.[0] || sp.preview;
                      const active = selectedSprite?.name === sp.name;
                      return (
                        <button
                          key={sp.name}
                          type="button"
                          className={`char-sheet-card ${active ? "is-active" : ""}`}
                          onClick={() => onSelectSprite(sp)}
                        >
                          {thumb ? <img src={fileUrl(thumb)} alt={sp.name} loading="lazy" /> : null}
                          <span>{sp.name}</span>
                        </button>
                      );
                    })}
                  </div>
                )}
              </>
            ) : (
              <>
                <div className="char-magic-head">
                  <strong className="char-subhead">2.5D Layered Rig &amp; Cycles</strong>
                  <span id="char-magic-badge" className="char-magic-badge">
                    {parts.length} layers · {sprites.length} cycles
                  </span>
                </div>
                {hasMultipleDrawnViews && (
                  <div id="char-angle-grid" className="char-angle-grid">
                    {drawnViews.map((vObj) => {
                      const v = vObj.view;
                      const thumb = vObj.drawing;
                      return (
                        <button
                          key={v}
                          type="button"
                          className={`char-thumb-card ${selectedView === v ? "is-active" : ""}`}
                          onClick={() => onSelectView(v)}
                        >
                          {thumb ? <img src={fileUrl(thumb)} alt={v} loading="lazy" /> : null}
                          <span>{v}</span>
                        </button>
                      );
                    })}
                  </div>
                )}
                {(rig25DCycles.length > 0 ? rig25DCycles : sprites).length > 0 && (
                  <div className="char-sheets-row">
                    {sprites.map((sp) => {
                      const thumb = sp.frames?.[0] || sp.preview;
                      const active = selectedSprite?.name === sp.name;
                      return (
                        <button
                          key={sp.name}
                          type="button"
                          className={`char-sheet-card ${active ? "is-active" : ""}`}
                          onClick={() => onSelectSprite(sp)}
                        >
                          {thumb ? <img src={fileUrl(thumb)} alt={sp.name} loading="lazy" /> : null}
                          <span>{sp.name}</span>
                        </button>
                      );
                    })}
                  </div>
                )}
                <div className="motion-path-actions" style={{ marginTop: "8px" }}>
                  <button
                    id="btn-assemble-2-5d-blender"
                    type="button"
                    className="ghost"
                    disabled={busy}
                    onClick={() => void runAssembleBlender(false)}
                  >
                    Assemble 2.5D/3D Planes
                  </button>
                  <button
                    id="btn-open-2-5d-blender"
                    type="button"
                    className="ghost"
                    disabled={busy}
                    onClick={() => void runAssembleBlender(true)}
                  >
                    Open in Blender…
                  </button>
                </div>
              </>
            )}
          </div>

          <button
            id="btn-generate-missing-poses"
            type="button"
            className="ghost"
            onClick={() =>
              void executeCommand("character.open", {
                character: selectedCharId,
                view: selectedView,
                app: "effectcraft",
              })
            }
          >
            Edit in EffectCraft…
          </button>

          <div className="seq-side-block">
            <label className="seq-field">
              <span>Action name</span>
              <input
                id="authoring-label"
                type="text"
                value={actionLabel}
                onChange={(e) => setActionLabel(e.target.value)}
              />
            </label>
            <Slider
              id="authoring-duration"
              label="Cycle duration"
              value={actionDuration}
              min={0.25}
              max={10}
              step={0.25}
              unit="s"
              onChange={setActionDuration}
            />
            <Toggle
              id="authoring-loop"
              label="Loop action cycle"
              checked={isLooping}
              onChange={onToggleLoop}
            />
            <button
              id="authoring-action-apply"
              type="button"
              className="ghost"
              disabled={busy}
              onClick={() => onTriggerAction(actionLabel || "idle")}
            >
              Save action settings
            </button>
          </div>

          <details className="seq-side-block">
            <summary className="muted" style={{ cursor: "pointer", fontSize: "var(--text-xs)" }}>
              Scene placement
            </summary>
            <div className="motion-section-body" style={{ padding: "var(--space-2) 0 0" }}>
              <label className="seq-field">
                <span>Track</span>
                <select
                  id="authoring-place-scene"
                  value={placeTrack}
                  onChange={(e) => onChangePlaceTrack(e.target.value)}
                >
                  <option value="V2">V2 (Characters)</option>
                  <option value="V1">V1 (Background)</option>
                  <option value="V3">V3 (Overlays)</option>
                </select>
              </label>
              <div className="motion-coordinate-row">
                <label className="anim-field-label">
                  <span>Start (s)</span>
                  <input
                    id="authoring-place-start"
                    type="number"
                    step={0.5}
                    value={placeTime}
                    onChange={(e) => onChangePlaceTime(Number(e.target.value))}
                  />
                </label>
                <label className="anim-field-label">
                  <span>Duration (s)</span>
                  <input
                    id="authoring-place-duration"
                    type="number"
                    step={0.5}
                    value={actionDuration}
                    onChange={(e) => setActionDuration(Number(e.target.value))}
                  />
                </label>
              </div>
              <Slider
                id="authoring-place-rate"
                label="Clip rate"
                control="dial"
                value={placeRate}
                min={0.25}
                max={2.0}
                step={0.05}
                unit="×"
                onChange={setPlaceRate}
              />
              <div className="motion-coordinate-row">
                <label className="anim-field-label">
                  <span>X</span>
                  <input
                    id="authoring-place-x"
                    type="number"
                    value={placeX}
                    onChange={(e) => setPlaceX(Number(e.target.value))}
                  />
                </label>
                <label className="anim-field-label">
                  <span>Y</span>
                  <input
                    id="authoring-place-y"
                    type="number"
                    value={placeY}
                    onChange={(e) => setPlaceY(Number(e.target.value))}
                  />
                </label>
                <label className="anim-field-label">
                  <span>W</span>
                  <input
                    id="authoring-place-w"
                    type="number"
                    value={placeW}
                    onChange={(e) => setPlaceW(Number(e.target.value))}
                  />
                </label>
                <label className="anim-field-label">
                  <span>H</span>
                  <input
                    id="authoring-place-h"
                    type="number"
                    value={placeH}
                    onChange={(e) => setPlaceH(Number(e.target.value))}
                  />
                </label>
              </div>
              <button
                id="authoring-place-video"
                type="button"
                className="start"
                disabled={busy}
                onClick={onPlaceOnTimeline}
              >
                Place in Studio
              </button>
              <span id="authoring-place-status" className="muted" role="status">
                {lastRenderedClip ? `Ready: ${lastRenderedClip.split("/").pop()}` : "Render a clip first"}
              </span>
            </div>
          </details>
        </div>
      </details>

      {/* 2. PATH & DEPTH */}
      <details id="motion-path-section" className="panel motion-section" data-look="depth">
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M12 20h9" />
              <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
            </svg>
          </span>
          <span className="title">Path &amp; depth</span>
          <span id="motion-path-meta" className="meta">
            {sizeStart}%→{sizeEnd}%
          </span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <p className="motion-help">
            Draw a walk path on the stage or adjust start/end depth scale for perspective travel.
          </p>
          <label className="seq-field">
            <span>Path root</span>
            <select
              id="motion-path-layer"
              value={selectedBone}
              onChange={(e) => onSelectBone(e.target.value)}
            >
              <option value="root">root</option>
              {parts.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.id}
                </option>
              ))}
            </select>
          </label>
          <label className="seq-field">
            <span>Edit point</span>
            <select
              id="motion-path-point"
              value={pathPointIdx}
              onChange={(e) => onSelectPathPoint(Number(e.target.value))}
            >
              <option value={0}>Point 1 (Start)</option>
              <option value={1}>Point 2 (Mid)</option>
              <option value={2}>Point 3 (End)</option>
            </select>
          </label>
          <div className="craft-axis-controls">
            <Slider
              id="motion-path-x"
              label="Point X"
              value={pathX}
              min={0}
              max={1920}
              step={1}
              unit="px"
              onChange={onChangePathX}
            />
            <Slider
              id="motion-path-y"
              label="Point Y"
              control="vertical"
              value={pathY}
              min={0}
              max={1080}
              step={1}
              unit="px"
              onChange={onChangePathY}
            />
          </div>
          <Slider
            id="motion-path-duration"
            label="Travel duration"
            value={pathDuration}
            min={0.2}
            max={12}
            step={0.1}
            unit="s"
            onChange={onChangePathDuration}
          />
          <Slider
            id="motion-size-start"
            label="Start size"
            value={sizeStart}
            min={20}
            max={220}
            step={5}
            unit="%"
            onChange={onChangeSizeStart}
          />
          <Slider
            id="motion-size-end"
            label="End size"
            value={sizeEnd}
            min={20}
            max={220}
            step={5}
            unit="%"
            onChange={onChangeSizeEnd}
          />
          <div className="motion-path-actions">
            <button
              id="motion-path-clear"
              type="button"
              className="ghost"
              onClick={() => {
                onChangeSizeStart(100);
                onChangeSizeEnd(100);
                onToast("Cleared motion path");
              }}
            >
              Clear
            </button>
            <button
              id="motion-path-undo"
              type="button"
              className="ghost"
              onClick={() => onToast("Reverted last path point")}
            >
              Undo point
            </button>
            <button
              id="motion-path-apply"
              type="button"
              className="start"
              onClick={() =>
                onToast(`Applied path (${pathX}, ${pathY}) · ${sizeStart}% → ${sizeEnd}%`)
              }
            >
              Apply path
            </button>
          </div>
        </div>
      </details>

      {/* 3. MOTION & TIMING */}
      <details id="motion-timing" className="panel motion-section" data-look="motion">
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          </span>
          <span className="title">Motion &amp; timing</span>
          <span id="motion-timing-meta" className="meta">
            {playbackSpeed.toFixed(2)}×
          </span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <Toggle
            id="motion-preview-loop"
            label="Loop preview"
            checked={isLooping}
            onChange={onToggleLoop}
          />
          <Slider
            id="motion-speed"
            label="Playback speed"
            value={playbackSpeed}
            min={0.25}
            max={2.25}
            step={0.05}
            unit="×"
            onChange={onChangePlaybackSpeed}
          />
          <Slider
            id="motion-amount"
            label="Motion amount"
            value={motionAmount}
            min={0}
            max={100}
            step={5}
            unit="%"
            onChange={onChangeMotionAmount}
          />
        </div>
      </details>

      {/* 4. LAYERS (open by default per A3) */}
      <details id="motion-layers" className="panel motion-section" data-look="depth" open>
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="m12 2 9 5-9 5-9-5 9-5z" />
              <path d="m3 12 9 5 9-5" />
              <path d="m3 17 9 5 9-5" />
            </svg>
          </span>
          <span className="title">Layers</span>
          <span id="motion-layers-meta" className="meta">
            {parts.length}
          </span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div className="motion-coordinate-row">
            <label className="anim-field-label">
              <span>Layer</span>
              <select
                id="authoring-layer-picker"
                value={selectedBone}
                onChange={(e) => onSelectBone(e.target.value)}
              >
                {parts.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.id}
                  </option>
                ))}
              </select>
            </label>
            <label className="anim-field-label">
              <span>Reason</span>
              <input
                id="authoring-layer-reason"
                type="text"
                placeholder="e.g. arm in front"
                value={layerReason}
                onChange={(e) => setLayerReason(e.target.value)}
              />
            </label>
          </div>
          <Slider
            id="authoring-layer-depth"
            label="Depth offset"
            control="horizontal"
            value={layerDepth}
            min={-10}
            max={10}
            step={1}
            onChange={setLayerDepth}
          />
          <div className="motion-path-actions">
            <button
              id="authoring-layer-back"
              type="button"
              className="ghost"
              onClick={() => {
                setLayerDepth((d) => d - 1);
                onToast(`Sent ${selectedBone} backward`);
              }}
            >
              Send back
            </button>
            <button
              id="authoring-layer-front"
              type="button"
              className="ghost"
              onClick={() => {
                setLayerDepth((d) => d + 1);
                onToast(`Brought ${selectedBone} forward`);
              }}
            >
              Bring forward
            </button>
            <button
              id="authoring-layer-apply"
              type="button"
              className="start"
              onClick={() => onToast(`Applied layer order (${selectedBone}: ${layerDepth})`)}
            >
              Apply order
            </button>
          </div>

          <div className="motion-layers">
            {parts.map((p) => {
              const isSel = selectedBone === p.id;
              const isHidden = Boolean(hiddenParts[p.id]);
              return (
                <div
                  key={p.id}
                  id={`motion-layer-${p.id}`}
                  className={`motion-layer ${isSel ? "selected" : ""} ${isHidden ? "hidden-layer" : ""}`}
                >
                  {p.file ? (
                    <img src={fileUrl(p.file)} alt={p.id} loading="lazy" />
                  ) : (
                    <span className="magic-asset-placeholder">PT</span>
                  )}
                  <button
                    type="button"
                    className="motion-layer-name"
                    aria-pressed={isSel}
                    onClick={() => onSelectBone(p.id)}
                  >
                    {p.name || p.id}
                  </button>
                  <button
                    id={`motion-visible-${p.id}`}
                    type="button"
                    className="icon"
                    aria-pressed={!isHidden}
                    title={isHidden ? "Show layer" : "Hide layer"}
                    onClick={() =>
                      setHiddenParts((prev) => ({ ...prev, [p.id]: !prev[p.id] }))
                    }
                  >
                    👁
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      </details>

      {/* 5. PART ARTWORK */}
      <details id="magic-parts-pane" className="panel motion-section" data-look="artwork">
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="m12 2 9 5-9 5-9-5 9-5z" />
              <path d="m3 12 9 5 9-5" />
              <path d="m3 17 9 5 9-5" />
            </svg>
          </span>
          <span className="title">Part artwork</span>
          <span id="magic-parts-meta" className="meta">
            {isRigged ? "Rigged" : isCut ? "Cut" : isMarked ? "Marked" : "Cutout"}
          </span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div id="magic-parts-slot" className="magic-parts-slot">
            <div id="character-prepare" className="seq-side-block" style={{ borderTop: 0, paddingTop: 0 }}>
              {hasMultipleDrawnViews && (
                <label className="seq-field">
                  <span>View</span>
                  <select
                    id="character-prepare-view"
                    value={selectedView}
                    onChange={(e) => onSelectView(e.target.value)}
                  >
                    {drawnViews.map((vObj) => (
                      <option key={vObj.view} value={vObj.view}>
                        {vObj.view}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              <p id="character-prepare-status" className="motion-help">
                {activeViewSummary?.status || "Mark parts, cut layers in PhotoCraft, and rig in EffectCraft."}
              </p>
              <div className="motion-path-actions">
                <button
                  id="character-mark-run"
                  type="button"
                  className="ghost"
                  disabled={busy}
                  onClick={() => void runPrepareStep("character.mark", "1. Mark parts")}
                >
                  1. Mark parts (Scribbles)
                </button>
                <button
                  id="character-cut-run"
                  type="button"
                  className="ghost"
                  disabled={busy}
                  onClick={() => void runPrepareStep("character.cut", "2. Cut layers")}
                >
                  2. Cut layers (PhotoCraft)
                </button>
                <button
                  id="character-rig-run"
                  type="button"
                  className="start"
                  disabled={busy}
                  onClick={() => void runPrepareStep("character.rig", "3. Rig & animate")}
                >
                  3. Rig &amp; animate (EffectCraft)
                </button>
              </div>
              <div className="motion-path-actions">
                <button
                  id="character-open-photocraft"
                  type="button"
                  className="ghost"
                  onClick={() =>
                    void executeCommand("character.open", {
                      character: selectedCharId,
                      view: selectedView,
                      app: "photocraft",
                    })
                  }
                >
                  Edit in PhotoCraft
                </button>
                <button
                  id="character-open-effectcraft"
                  type="button"
                  className="ghost"
                  onClick={() =>
                    void executeCommand("character.open", {
                      character: selectedCharId,
                      view: selectedView,
                      app: "effectcraft",
                    })
                  }
                >
                  Edit in EffectCraft
                </button>
              </div>
            </div>

            <div className="magic-parts-drawer" style={{ marginTop: "var(--space-2)" }}>
              <div className="segmented" role="tablist" aria-label="Part artwork view">
                <button
                  id="magic-parts-tab-parts"
                  type="button"
                  role="tab"
                  aria-selected={partsTab === "parts"}
                  className={partsTab === "parts" ? "active" : ""}
                  onClick={() => setPartsTab("parts")}
                >
                  Parts ({parts.length})
                </button>
                <button
                  id="magic-parts-tab-compare"
                  type="button"
                  role="tab"
                  aria-selected={partsTab === "compare"}
                  className={partsTab === "compare" ? "active" : ""}
                  onClick={() => setPartsTab("compare")}
                >
                  Compare
                </button>
              </div>

              {partsTab === "parts" ? (
                <div className="magic-part-cards" style={{ marginTop: "var(--space-2)" }}>
                  {parts.map((p) => {
                    const isSel = selectedBone === p.id;
                    return (
                      <div
                        key={p.id}
                        className={`magic-part-card ${isSel ? "active" : ""}`}
                        onClick={() => onSelectBone(p.id)}
                      >
                        <div className="magic-part-card-thumb">
                          {p.file ? (
                            <img src={fileUrl(p.file)} alt={p.id} loading="lazy" />
                          ) : (
                            <span className="magic-asset-placeholder">PT</span>
                          )}
                        </div>
                        <div className="magic-part-card-pick">
                          <span className="magic-part-card-name">{p.name || p.id}</span>
                          <span className="dot ok" />
                        </div>
                        <div className="magic-part-card-actions">
                          <span className="magic-part-card-id">{p.parent || "root"}</span>
                          <button
                            type="button"
                            className="ghost magic-part-card-open"
                            onClick={(e) => {
                              e.stopPropagation();
                              void executeCommand("character.open", {
                                character: selectedCharId,
                                view: selectedView,
                                app: "photocraft",
                              });
                            }}
                          >
                            PSD
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              ) : (
                <div id="magic-part-compare" className="magic-part-compare" style={{ marginTop: "var(--space-2)" }}>
                  <div className="magic-parts-strip">
                    {parts.map((p) => (
                      <button
                        key={p.id}
                        type="button"
                        className={`magic-part-thumb ${selectedBone === p.id ? "active" : ""}`}
                        onClick={() => onSelectBone(p.id)}
                      >
                        {p.file ? <img src={fileUrl(p.file)} alt={p.id} /> : null}
                        <span>{p.id}</span>
                      </button>
                    ))}
                  </div>
                  <div className="magic-compare-grid">
                    <figure className="magic-compare-frame">
                      {activeViewSummary?.drawing ? (
                        <img src={fileUrl(activeViewSummary.drawing)} alt="Original drawing" />
                      ) : (
                        <span className="magic-asset-placeholder">SRC</span>
                      )}
                      <figcaption>Source drawing</figcaption>
                    </figure>
                    <figure className="magic-compare-frame">
                      {activePart?.file ? (
                        <img src={fileUrl(activePart.file)} alt={activePart.id} />
                      ) : (
                        <span className="magic-asset-placeholder">CUT</span>
                      )}
                      <figcaption>{activePart?.id || "Cut layer"}</figcaption>
                    </figure>
                  </div>
                  <div className="magic-part-actionbar">
                    <button
                      id="part-open-photocraft"
                      type="button"
                      className="ghost"
                      onClick={() =>
                        void executeCommand("character.open", {
                          character: selectedCharId,
                          view: selectedView,
                          app: "photocraft",
                        })
                      }
                    >
                      Open in PhotoCraft
                    </button>
                    <button
                      id="part-copy-path"
                      type="button"
                      className="ghost"
                      onClick={() => {
                        if (activePart?.file) {
                          void navigator.clipboard?.writeText(activePart.file);
                          onToast(`Copied path: ${activePart.file}`);
                        }
                      }}
                    >
                      Copy path
                    </button>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </details>

      {/* 6. EXPRESSION & POSE (includes original Part Transform + tucked Pose / 2-Bone IK / Puppet Mesh / Hand & Facial Sprites) */}
      <details id="motion-pose" className="panel motion-section" data-look="pose" open>
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          </span>
          <span className="title">Expression &amp; pose</span>
          <span id="motion-pose-meta" className="meta">
            {selectedBone}
          </span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div id="anim-selected-part-inspector" className="anim-part-inspector">
            <div className="anim-bone-banner">
              <span className="anim-subhead">
                Selected Part · {selectedBone.toUpperCase()}
              </span>
              <span className="anim-engine-badge">• EffectCraft</span>
            </div>
            <div className="motion-path-actions">
              <button
                id="motion-bar-visible"
                type="button"
                className="ghost"
                onClick={() =>
                  setHiddenParts((prev) => ({
                    ...prev,
                    [selectedBone]: !prev[selectedBone],
                  }))
                }
              >
                {hiddenParts[selectedBone] ? "Show part" : "Hide part"}
              </button>
              <button
                id="motion-show-all"
                type="button"
                className="ghost"
                onClick={() => setHiddenParts({})}
              >
                Show all parts
              </button>
            </div>
          </div>

          <div className="motion-coordinate-row" style={{ gridTemplateColumns: "repeat(4, minmax(0, 1fr))" }}>
            <label className="anim-field-label">
              <span>Pos X</span>
              <input
                id="authoring-part-x"
                type="number"
                value={partPosX}
                onChange={(e) => setPartPosX(Number(e.target.value))}
              />
            </label>
            <label className="anim-field-label">
              <span>Pos Y</span>
              <input
                id="authoring-part-y"
                type="number"
                value={partPosY}
                onChange={(e) => setPartPosY(Number(e.target.value))}
              />
            </label>
            <label className="anim-field-label">
              <span>Piv X</span>
              <input
                id="authoring-pivot-x"
                type="number"
                value={partPivotX}
                onChange={(e) => setPartPivotX(Number(e.target.value))}
              />
            </label>
            <label className="anim-field-label">
              <span>Piv Y</span>
              <input
                id="authoring-pivot-y"
                type="number"
                value={partPivotY}
                onChange={(e) => setPartPivotY(Number(e.target.value))}
              />
            </label>
          </div>
          <div className="motion-coordinate-row">
            <label className="anim-field-label">
              <span>Scale X (%)</span>
              <input
                id="authoring-scale-x"
                type="number"
                value={partScaleX}
                onChange={(e) => setPartScaleX(Number(e.target.value))}
              />
            </label>
            <label className="anim-field-label">
              <span>Scale Y (%)</span>
              <input
                id="authoring-scale-y"
                type="number"
                value={partScaleY}
                onChange={(e) => setPartScaleY(Number(e.target.value))}
              />
            </label>
          </div>

          <Slider
            id="bone-rotation"
            label="Rotation"
            control="dial"
            value={rotation}
            min={-180}
            max={180}
            step={1}
            unit="°"
            onChange={onChangeRotation}
            onCommit={onCommitRotation}
          />

          <Slider
            id="bone-scale"
            label="Scale"
            value={scale}
            min={10}
            max={200}
            step={1}
            unit="%"
            onChange={onChangeScale}
            onCommit={onCommitScale}
          />

          <Slider
            id="bone-opacity"
            label="Opacity"
            value={opacity}
            min={0}
            max={100}
            step={1}
            unit="%"
            onChange={onChangeOpacity}
            onCommit={onCommitOpacity}
          />

          <div className="motion-path-actions">
            <button
              id="authoring-part-reset"
              type="button"
              className="ghost"
              onClick={() => {
                onChangeRotation(0);
                onChangeScale(100);
                onChangeOpacity(100);
                setPartPosX(0);
                setPartPosY(0);
                setPartScaleX(100);
                setPartScaleY(100);
                onToast(`Reset ${selectedBone} to rest pose`);
              }}
            >
              Reset part
            </button>
            <button
              id="authoring-part-apply"
              type="button"
              className="start"
              disabled={busy}
              onClick={onKeyTransform}
            >
              Key {selectedBone} Transform
            </button>
          </div>

          {/* Tucked Extra Controls: Hand Sprites & Facial States */}
          <div className="seq-side-block">
            <div className="anim-section-bar">
              <span className="anim-subhead">Hand Sprites &amp; Gesture Swap</span>
              <button
                type="button"
                className={`anim-hold-pill ${handHold ? "active" : ""}`}
                onClick={onToggleHandHold}
              >
                • Hold
              </button>
            </div>
            <div className="anim-gesture-row">
              <div className="anim-gesture-buttons" style={{ flex: 1 }}>
                {(["wave", "point", "fist", "open"] as const).map((g) => (
                  <button
                    key={g}
                    type="button"
                    className={`anim-pill-btn ${selectedGesture === g ? "active" : ""}`}
                    onClick={() => {
                      onSelectGesture(g);
                      onToast(`Swapped hand sprite to: ${g}`);
                    }}
                  >
                    {g}
                  </button>
                ))}
              </div>
              <Dial
                id="gesture-angle"
                label="Gesture Angle"
                value={gestureAngle}
                min={-180}
                max={180}
                step={5}
                unit="°"
                onChange={onChangeGestureAngle}
                onCommit={onCommitGestureAngle}
              />
            </div>

            <div className="anim-section-bar" style={{ marginTop: "var(--size-4)" }}>
              <span className="anim-subhead">Facial States</span>
              <button
                type="button"
                className={`anim-hold-pill ${faceHold ? "active" : ""}`}
                onClick={onToggleFaceHold}
              >
                • Hold
              </button>
            </div>
            <div className="anim-four-col">
              {(["Neutral", "Happy", "Blink", "Sing"] as const).map((label) => (
                <button
                  key={label}
                  type="button"
                  className="anim-pill-btn"
                  onClick={() => onToast(`Selected Facial State: ${label}`)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          {/* Tucked Extra Controls: 2-Bone IK / FK Limb Solver */}
          <div className="seq-side-block">
            <span className="anim-subhead">2-Bone IK / FK Limb Solver</span>
            <Toggle
              id="ik-solver-active"
              label="2-Bone IK Solver Active"
              checked={ikActive}
              onChange={onChangeIkActive}
            />
            <label className="anim-field-label">
              <span>Limb Chain</span>
              <select value={limbChain} onChange={(e) => onChangeLimbChain(e.target.value)}>
                <option value="Right Leg (torso → leg_R)">Right Leg (torso → leg_R)</option>
                <option value="Left Leg (torso → leg_L)">Left Leg (torso → leg_L)</option>
                <option value="Right Arm (torso → arm_R)">Right Arm (torso → arm_R)</option>
                <option value="Left Arm (torso → arm_L)">Left Arm (torso → arm_L)</option>
              </select>
            </label>
            <Toggle
              id="pole-bend-clockwise"
              label="Pole Bend Clockwise"
              checked={poleBendClockwise}
              onChange={onChangePoleBend}
            />
            <Toggle
              id="torso-secondary-follow"
              label="Torso Secondary Follow"
              checked={torsoSecondaryFollow}
              onChange={onChangeTorsoFollow}
            />
            <Slider
              id="ik-target-angle"
              label="IK Target Angle"
              control="dial"
              value={ikTargetAngle}
              min={-180}
              max={180}
              step={1}
              unit="°"
              onChange={onChangeIkTargetAngle}
              onCommit={onCommitIkTargetAngle}
            />
            <button
              id="anim-solve-ik"
              type="button"
              className="start anim-full-btn"
              disabled={busy}
              onClick={onSolveIK}
            >
              Solve &amp; Key 2-Bone IK
            </button>
          </div>

          {/* Tucked Extra Controls: Puppet Mesh & Pin Stiffness (collapsed 1-line pill drawer by default) */}
          <details className="seq-side-block">
            <summary className="muted" style={{ cursor: "pointer", fontSize: "var(--text-xs)" }}>
              Puppet Mesh &amp; Pin Stiffness ({pinMode} · {pinStiffness}%)
            </summary>
            <div className="motion-section-body" style={{ padding: "var(--space-2) 0 0" }}>
              <Slider
                id="mesh-density"
                label="Mesh Density"
                value={meshDensity}
                min={12}
                max={256}
                step={4}
                unit=" tris"
                onChange={onChangeMeshDensity}
              />
              <Slider
                id="mesh-expansion"
                label="Mesh Expansion"
                value={meshExpansion}
                min={0}
                max={32}
                step={1}
                unit=" px"
                onChange={onChangeMeshExpansion}
              />
              <div className="anim-three-col">
                {(["deform", "starch", "overlap"] as const).map((mode) => (
                  <button
                    key={mode}
                    type="button"
                    className={`anim-pill-btn ${pinMode === mode ? "active" : ""}`}
                    onClick={() => onChangePinMode(mode)}
                  >
                    {mode}
                  </button>
                ))}
              </div>
              <Slider
                id="pin-stiffness"
                label="Pin Stiffness"
                value={pinStiffness}
                min={0}
                max={100}
                step={1}
                unit="%"
                onChange={onChangePinStiffness}
              />
              <Slider
                id="pin-extent"
                label="Pin Extent"
                value={pinExtent}
                min={0}
                max={100}
                step={1}
                unit=" px"
                onChange={onChangePinExtent}
              />
              <Slider
                id="follow-delay-dial"
                label="Follow Delay"
                control="dial"
                value={followDelay}
                min={0}
                max={1}
                step={0.01}
                unit="s"
                onChange={onChangeFollowDelay}
              />
            </div>
          </details>

          {hasMultipleDrawnViews && (
            <button
              id="btn-generate-turnaround-sheet"
              type="button"
              className="ghost"
              onClick={() => {
                const idx = drawnViews.findIndex((v) => v.view === selectedView);
                const nextView = drawnViews[(idx + 1) % drawnViews.length]?.view || "front";
                onSelectView(nextView);
              }}
            >
              Switch drawn view ({selectedView})…
            </button>
          )}
        </div>
      </details>

      {/* 7. QUALITY CHECKS */}
      <details id="motion-quality" className="panel motion-section" data-look="check">
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M20 6 9 17l-5-5" />
            </svg>
          </span>
          <span className="title">Quality checks</span>
          <span id="motion-quality-meta" className="meta">
            {passedChecks}/4
          </span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <p id="motion-quality-summary" className="motion-help">
            {passedChecks}/4 asset structure checks passed
          </p>
          <ul id="motion-checks" className="companion-state">
            <li>{isMarked ? "✓" : "✗"} Parts and joints marked (Scribbles)</li>
            <li>{isCut ? "✓" : "✗"} Parts cut into layers (PhotoCraft)</li>
            <li>{isRigged ? "✓" : "✗"} Puppet rigged (EffectCraft)</li>
            <li>{hasSpritesOr3D ? "✓" : "✗"} Action cycles / 3D rig ready</li>
          </ul>
        </div>
      </details>

      {/* 8. EXPORT & INTEGRATIONS */}
      <details id="motion-export-section" className="panel motion-section" data-look="export">
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
          </span>
          <span className="title">Export &amp; integrations</span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <button
            id="motion-export"
            type="button"
            className="start anim-full-btn"
            disabled={busy}
            onClick={onRenderClip}
          >
            {busy ? "Rendering…" : "Save animation (.mov)"}
          </button>
          <span id="motion-export-status" className="muted" role="status">
            {lastRenderedClip
              ? `Saved: ${lastRenderedClip.split("/").pop()}`
              : "Renders ProRes 4444 with alpha via EffectCraft."}
          </span>

          <div className="anim-two-col">
            <label className="anim-field-label">
              <span>Track</span>
              <select value={placeTrack} onChange={(e) => onChangePlaceTrack(e.target.value)}>
                <option value="V2">V2 (Characters)</option>
                <option value="V1">V1 (Background)</option>
                <option value="V3">V3 (Overlays)</option>
              </select>
            </label>
            <label className="anim-field-label">
              <span>At Seconds</span>
              <input
                type="number"
                min={0}
                step={0.5}
                value={placeTime}
                onChange={(e) => onChangePlaceTime(Number(e.target.value))}
              />
            </label>
          </div>

          <button
            id="anim-place-clip-btn"
            type="button"
            className="ghost anim-full-btn"
            disabled={busy || !lastRenderedClip}
            onClick={onPlaceOnTimeline}
          >
            Place Clip on Film Timeline
          </button>

          <div className="seq-side-block">
            <label className="seq-field">
              <span>Pose reference</span>
              <input
                id="authoring-reference"
                type="text"
                value={poseRef}
                onChange={(e) => setPoseRef(e.target.value)}
              />
            </label>
            <button
              id="authoring-pose-export"
              type="button"
              className="ghost anim-full-btn"
              onClick={() => onToast(`Captured pose frame (${poseRef})`)}
            >
              Capture pending pose
            </button>
          </div>
        </div>
      </details>

      <div id="motion-source-context" className="motion-source-context" />
    </aside>
  );
};
