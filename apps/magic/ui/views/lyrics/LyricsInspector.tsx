import React, { useCallback, useEffect, useState } from "react";
import { executeCommand } from "../../api";
import { StyleClipboard } from "../../components/StyleClipboard";
import { Dial, Slider, Toggle } from "../../components/controls";
import {
  buildDefaultLineDesign,
  EasingPreset,
  effectiveOutlineColor,
  isDarkHex,
  LetteringPreset,
  LineDesign,
  MotionProperty,
  MovePreset,
  StrokePreset,
  TextPresetItem,
  WordItem,
  WordsAnimation,
  WordToken,
} from "./types";

const ROTATION = "transform/rotation";
const OPACITY = "transform/opacity";
const POSITION = "transform/position";
const SCALE = "transform/scale";

const FALLBACK_PROPERTIES: MotionProperty[] = [
  { path: POSITION, name: "Position" },
  { path: ROTATION, name: "Rotation" },
  { path: SCALE, name: "Scale" },
  { path: OPACITY, name: "Opacity" },
];

type Pair = [number, number];

interface MotionSpec {
  property: string;
  delay: number;
  duration: number;
  from: number | Pair;
  to: number | Pair;
  speed: number;
  easing: string;
  loopMode: "loop" | "play_once" | "single_frame";
  firstFrame: number;
}

const finite = (value: unknown, fallback: number): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;

const pair = (value: unknown, fallback: Pair): Pair =>
  Array.isArray(value) && value.length >= 2
    ? [finite(value[0], fallback[0]), finite(value[1], fallback[1])]
    : fallback;

const scalar = (value: unknown, fallback: number): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;

function defaultMotionSpec(property: string): MotionSpec {
  if (property === POSITION) {
    return { property, delay: 0, duration: 1.2, from: [0, -40], to: [0, 0], speed: 0, easing: "cubic_out", loopMode: "play_once", firstFrame: 0 };
  }
  if (property === SCALE) {
    return { property, delay: 0, duration: 0.9, from: [130, 130], to: [100, 100], speed: 0, easing: "cubic_out", loopMode: "play_once", firstFrame: 0 };
  }
  if (property === OPACITY) {
    return { property, delay: 0, duration: 0.8, from: 0, to: 100, speed: 0, easing: "quad_out", loopMode: "play_once", firstFrame: 0 };
  }
  return { property, delay: 0, duration: 1.2, from: -12, to: 0, speed: 90, easing: "cubic_out", loopMode: "loop", firstFrame: 0 };
}

interface StoryboardSlot {
  id?: string;
  label?: string;
  kind?: string;
  text?: string;
}

interface StoryboardPage {
  page: number;
  title?: string;
  archetype?: string;
  layout?: string;
  slots?: StoryboardSlot[];
}

export interface LyricsInspectorProps {
  mode: "design" | "motion";
  anim: WordsAnimation | undefined;
  lineId: string | null;
  lineWords: WordToken[];
  design: LineDesign;
  activeItemIndex: number;
  onSelectItemIndex: (index: number) => void;
  onUpdateDesign: (updater: (draft: LineDesign) => void) => void;
  onSaveAndSync: () => Promise<void>;
  moves: MovePreset[];
  letterings: LetteringPreset[];
  strokes: StrokePreset[];
  choreographies: { id: string; name: string; does: string }[];
  easings: EasingPreset[];
  textPresets: (string | TextPresetItem)[];
  drawMode: boolean;
  onToggleDrawMode: () => void;
  brushColor: string;
  onBrushColorChange: (color: string) => void;
  brushWidth: number;
  onBrushWidthChange: (width: number) => void;
  onToast: (msg: string) => void;
}

export const LyricsInspector: React.FC<LyricsInspectorProps> = ({
  mode,
  anim,
  lineId,
  lineWords,
  design,
  activeItemIndex,
  onSelectItemIndex,
  onUpdateDesign,
  onSaveAndSync,
  moves,
  letterings,
  strokes,
  choreographies,
  easings,
  textPresets,
  drawMode,
  onToggleDrawMode,
  brushColor,
  onBrushColorChange,
  brushWidth,
  onBrushWidthChange,
  onToast,
}) => {
  const item: WordItem | undefined = design.items[activeItemIndex] ?? design.items[0];
  const itemText = (item?.word_ids ?? [])
    .map((wid) => lineWords.find((w) => w.word_id === wid)?.text)
    .filter(Boolean)
    .join(" ");

  const patchItem = useCallback(
    (patch: Partial<WordItem>) => {
      onUpdateDesign((draft) => {
        const target = draft.items[activeItemIndex] ?? draft.items[0];
        if (!target) return;
        Object.assign(target, patch);
      });
    },
    [activeItemIndex, onUpdateDesign],
  );

  // Native EffectCraft keyframe curve state
  const [motionProps, setMotionProps] = useState<MotionProperty[]>(FALLBACK_PROPERTIES);
  const [compBindingLabel, setCompBindingLabel] = useState<string>("Lyrics · EffectCraft");
  const [motionSpec, setMotionSpec] = useState<MotionSpec>(() =>
    defaultMotionSpec(ROTATION),
  );
  const [motionBusy, setMotionBusy] = useState(false);

  const wordIdsJoined = (item?.word_ids ?? []).join("|");
  useEffect(() => {
    let alive = true;
    if (!anim || !lineId || !item?.word_ids?.length) return;
    executeCommand("words.motion.properties", {
      line_id: lineId,
      word_ids: item.word_ids,
    })
      .then((res) => {
        if (!alive) return;
        const r = res as {
          properties?: MotionProperty[];
          comp?: { name?: string };
          binding?: { layer?: { name?: string } };
        };
        if (Array.isArray(r.properties) && r.properties.length > 0) {
          setMotionProps(r.properties);
        }
        if (r.binding?.layer?.name) {
          setCompBindingLabel(`${r.binding.layer.name} · ${r.comp?.name ?? "Lyrics"}`);
        } else {
          setCompBindingLabel("Lyrics · EffectCraft");
        }
      })
      .catch(() => {
        if (!alive) return;
        setCompBindingLabel("Lyrics · EffectCraft");
      });
    return () => {
      alive = false;
    };
  }, [anim, lineId, wordIdsJoined, item?.word_ids]);

  const applyNativeKeyframes = async () => {
    if (!anim || !lineId || !item?.word_ids?.length) return;
    setMotionBusy(true);
    try {
      await onSaveAndSync();
      const values =
        motionSpec.property === POSITION || motionSpec.property === SCALE
          ? {
              from: pair(motionSpec.from, [0, 0]),
              to: pair(motionSpec.to, [0, 0]),
            }
          : {
              from: scalar(motionSpec.from, 0),
              to: scalar(motionSpec.to, 0),
            };
      const req: Record<string, unknown> = {
        animation_id: anim.animation_id,
        line_id: lineId,
        word_ids: item.word_ids,
        expected_source_revision: finite(anim.source_revision, 0),
        property: motionSpec.property,
        delay: motionSpec.delay,
        duration: motionSpec.duration,
        from: values.from,
        easing: motionSpec.easing,
        loop_mode: motionSpec.loopMode,
        first_frame: motionSpec.firstFrame,
      };
      if (motionSpec.property === ROTATION) {
        req.speed = motionSpec.speed;
      } else {
        req.to = values.to;
      }
      const reply = (await executeCommand("words.motion.set", req)) as {
        ok?: boolean;
        motion_keys?: [number, unknown][];
        reason?: string;
      };
      if (!reply.ok) {
        throw new Error(reply.reason ?? "Failed to write keyframes to lyrics.ecproj");
      }
      patchItem({
        motion_keys: reply.motion_keys,
      });
      onToast(
        `Wrote ${reply.motion_keys?.length ?? 2} keyframes to lettering/lyrics.ecproj`,
      );
    } catch (err) {
      onToast(`Motion keyframe error: ${String(err)}`);
    } finally {
      setMotionBusy(false);
    }
  };

  // Storyboard pages for Scene Context
  const [sbPages, setSbPages] = useState<StoryboardPage[]>([]);
  const [selectedSbPage, setSelectedSbPage] = useState<number>(0);

  useEffect(() => {
    let alive = true;
    executeCommand("storyboard.pages", { full: true })
      .then((res) => {
        if (!alive) return;
        const pages = (res as { pages?: StoryboardPage[] })?.pages ?? [];
        setSbPages(pages);
        const firstScene = pages.find((p) => p.archetype === "scene") ?? pages[0];
        if (firstScene) setSelectedSbPage(firstScene.page);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const currentSbPage = sbPages.find((p) => p.page === selectedSbPage) ?? sbPages[0];
  const textSlots = (currentSbPage?.slots ?? []).filter(
    (sl) => sl.kind === "text" || typeof sl.text === "string",
  );

  const rotateVal = finite(item?.rotate, 0);
  const skewVal = finite(item?.skew, 0);
  const skewAxisVal = finite(item?.skew_axis, 0);
  const scaleXVal = finite(item?.scale_x, 100);
  const scaleYVal = finite(item?.scale_y, 100);
  const sizeVal = finite(item?.size, 84);
  const trackingVal = finite(item?.tracking, 0);
  const strokeWidthVal = finite(item?.stroke_width, 6);
  const fillHex = item?.color || "#D95B38";
  const outlineHex = effectiveOutlineColor(item ?? { word_ids: [], x: 0, y: 0 }) || "#FFF8E9";
  const warpVal = item?.warp_path ?? "none";

  return (
    <aside
      id="words-effects-rail"
      className="animation-rail-page lyrics-inspector-rail"
      aria-label="Lyrics and Motion Inspector"
    >
      {/* 1. Word Group & Transform / Skew (Cutout Look) */}
      <details
        id="lyrics-transform-section"
        className="panel motion-section"
        data-look="cutout"
        open
      >
        <summary className="panel-head">
          <span className="motion-section-icon" aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <path d="M3 3h4v4H3zM9 3h4v4H9zM3 9h4v4H3zM9 9h4v4H9z" />
            </svg>
          </span>
          <span className="title">Transform &amp; Skew</span>
          <span className="meta">{itemText || "Phrase"}</span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          {/* Phrase group selector + Split / Group buttons */}
          <div className="lyrics-word-group-bar">
            <div className="lyrics-word-chips">
              {design.items.map((grp, idx) => {
                const text = grp.word_ids
                  .map((id) => lineWords.find((w) => w.word_id === id)?.text)
                  .filter(Boolean)
                  .join(" ");
                return (
                  <button
                    key={`${grp.word_ids.join("_")}_${idx}`}
                    id={`words-group-chip-${idx}`}
                    type="button"
                    className={`lyrics-chip ${idx === activeItemIndex ? "active" : ""}`}
                    onClick={() => onSelectItemIndex(idx)}
                  >
                    {text || `Group ${idx + 1}`}
                  </button>
                );
              })}
            </div>
            <div className="lyrics-group-actions">
              <button
                id="words-split-tokens"
                type="button"
                className="ghost"
                title="Split every word on this line into its own independent box"
                onClick={() => {
                  if (!lineWords.length) return;
                  onUpdateDesign((draft) => {
                    draft.items = lineWords.map((w, idx) => ({
                      word_ids: [w.word_id],
                      x: 180 + (idx % 4) * 390,
                      y: 280 + Math.floor(idx / 4) * 210,
                      w: 360,
                      h: 130,
                      lettering: idx === 0 ? "serif-italic" : "song-bold",
                      size: idx === 0 ? 96 : 74,
                      color: idx === 0 ? "#D95B38" : "#173F49",
                      outline: "#FFF8E9",
                      stroke_width: 6,
                      tracking: 0,
                      rotate: 0,
                      skew: 0,
                      skew_axis: 0,
                      scale_x: 100,
                      scale_y: 100,
                      warp_path: "none",
                      move: "pop",
                      dx: 0,
                      dy: -28,
                    }));
                  });
                  onSelectItemIndex(0);
                }}
              >
                Split Words
              </button>
              <button
                id="words-merge-line"
                type="button"
                className="ghost"
                title="Group words back into expressive phrase blocks"
                onClick={() => {
                  onUpdateDesign((draft) => {
                    const fresh = buildDefaultLineDesign(
                      lineWords,
                      draft.choreography || "cascade",
                    );
                    draft.items = fresh.items;
                  });
                  onSelectItemIndex(0);
                }}
              >
                Group Phrase
              </button>
            </div>
          </div>

          <StyleClipboard
            id="word-style-clipboard"
            title="Word style"
            design={design as unknown as { items: Record<string, unknown>[]; draws: unknown[] }}
            itemIndex={activeItemIndex}
            onChange={(next) =>
              onUpdateDesign((draft) => {
                draft.items = next.items as unknown as WordItem[];
              })
            }
            onToast={onToast}
          />

          <div className="lyrics-dial-grid">
            <div className="craft-number" data-control="dial">
              <label className="craft-number-title" htmlFor="words-item-rotate-dial">
                <span className="craft-control-label">Rotate (360°)</span>
                <output className="craft-number-readout">{Math.round(rotateVal)}°</output>
              </label>
              <span className="craft-number-body">
                <Dial
                  id="words-item-rotate"
                  label="Rotate"
                  value={rotateVal}
                  min={-360}
                  max={360}
                  step={1}
                  unit="°"
                  onChange={(v) => patchItem({ rotate: v })}
                />
              </span>
            </div>

            <div className="craft-number" data-control="dial">
              <label className="craft-number-title" htmlFor="words-item-skew-dial">
                <span className="craft-control-label">Skew Slant</span>
                <output className="craft-number-readout">{Math.round(skewVal)}°</output>
              </label>
              <span className="craft-number-body">
                <Dial
                  id="words-item-skew"
                  label="Skew"
                  value={skewVal}
                  min={-70}
                  max={70}
                  step={1}
                  unit="°"
                  onChange={(v) => patchItem({ skew: v })}
                />
              </span>
            </div>

            <div className="craft-number" data-control="dial">
              <label className="craft-number-title" htmlFor="words-item-skew-axis-dial">
                <span className="craft-control-label">Skew Axis</span>
                <output className="craft-number-readout">{Math.round(skewAxisVal)}°</output>
              </label>
              <span className="craft-number-body">
                <Dial
                  id="words-item-skew-axis"
                  label="Skew Axis"
                  value={skewAxisVal}
                  min={-180}
                  max={180}
                  step={1}
                  unit="°"
                  onChange={(v) => patchItem({ skew_axis: v })}
                />
              </span>
            </div>
          </div>

          {/* Quick angle presets */}
          <div className="lyrics-quick-angles">
            {[-90, -15, 0, 15, 90, 180].map((deg) => (
              <button
                key={deg}
                id={`words-angle-${deg}`}
                type="button"
                className={`ghost ${Math.round(rotateVal) === deg ? "active" : ""}`}
                onClick={() => patchItem({ rotate: deg })}
              >
                {deg > 0 ? `+${deg}°` : `${deg}°`}
              </button>
            ))}
          </div>

          <Slider
            id="words-item-scale-x"
            label="Stretch X"
            value={scaleXVal}
            min={25}
            max={350}
            step={1}
            unit="%"
            onChange={(v) => patchItem({ scale_x: v })}
          />
          <Slider
            id="words-item-scale-y"
            label="Stretch Y"
            value={scaleYVal}
            min={25}
            max={350}
            step={1}
            unit="%"
            onChange={(v) => patchItem({ scale_y: v })}
          />
          <Toggle
            id="words-item-3d"
            label="EffectCraft 3D Per-Character Layer"
            checked={Boolean(item?.per_char_3d)}
            onChange={(v) => patchItem({ per_char_3d: v })}
          />
        </div>
      </details>

      {/* 2. Lettering, Contrast & Warp Path (Chalk Look) */}
      <details
        id="lyrics-type-section"
        className="panel motion-section"
        data-look="chalk"
        open={mode === "design"}
      >
        <summary className="panel-head">
          <span className="motion-section-icon" aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <path d="M3 3h10v2H9v8H7V5H3V3z" />
            </svg>
          </span>
          <span className="title">Lettering &amp; Warp Path</span>
          <span className="meta">{item?.lettering ?? "serif-italic"}</span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div className="lyrics-lettering-pills">
            {letterings.map((letPreset) => (
              <button
                key={letPreset.id}
                id={`words-lettering-${letPreset.id}`}
                type="button"
                className={`lyrics-chip ${item?.lettering === letPreset.id ? "active" : ""}`}
                title={letPreset.does}
                onClick={() => patchItem({ lettering: letPreset.id })}
              >
                {letPreset.name}
              </button>
            ))}
          </div>

          <Slider
            id="words-item-size"
            label="Font Size"
            value={sizeVal}
            min={28}
            max={260}
            step={2}
            unit="px"
            onChange={(v) => patchItem({ size: v })}
          />

          <Slider
            id="words-item-tracking"
            label="Tracking"
            value={trackingVal}
            min={-80}
            max={400}
            step={5}
            onChange={(v) => patchItem({ tracking: v })}
          />

          <div className="lyrics-color-row">
            <label className="lyrics-color-picker">
              <span>Fill</span>
              <input
                id="words-item-color"
                type="color"
                value={fillHex.startsWith("#") ? fillHex : "#D95B38"}
                onChange={(e) => {
                  const nextColor = e.target.value;
                  const nextOutline =
                    isDarkHex(nextColor) && (!item?.outline || item.outline === "none")
                      ? "#FFF8E9"
                      : item?.outline;
                  patchItem({ color: nextColor, outline: nextOutline });
                }}
              />
            </label>
            <label className="lyrics-color-picker">
              <span>Outline</span>
              <input
                id="words-item-outline"
                type="color"
                value={outlineHex.startsWith("#") ? outlineHex : "#FFF8E9"}
                onChange={(e) => patchItem({ outline: e.target.value })}
              />
            </label>
            <button
              id="words-item-contrast-fix"
              type="button"
              className="ghost"
              title="Ensure high-contrast cream halo outline behind dark lettering"
              onClick={() =>
                patchItem({
                  outline: isDarkHex(fillHex) ? "#FFF8E9" : "#11222B",
                  stroke_width: Math.max(6, strokeWidthVal),
                })
              }
            >
              Auto Halo
            </button>
          </div>

          <Slider
            id="words-item-stroke-width"
            label="Outline Halo"
            value={strokeWidthVal}
            min={0}
            max={28}
            step={1}
            unit="px"
            onChange={(v) => patchItem({ stroke_width: v })}
          />

          <div className="lyrics-sublabel">EffectCraft Text-on-Path Warp</div>
          <div className="lyrics-warp-pills">
            {(
              [
                ["none", "Flat"],
                ["arc_up", "⤴ Arc Up"],
                ["arc_down", "⤵ Arc Down"],
                ["wave", "〰 Wave"],
                ["circle", "◯ Orbit"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                id={`words-warp-${id}`}
                type="button"
                className={`lyrics-chip ${warpVal === id ? "active" : ""}`}
                onClick={() => patchItem({ warp_path: id })}
              >
                {label}
              </button>
            ))}
          </div>

          <div className="lyrics-sublabel">Line Choreography Layout</div>
          <div className="lyrics-warp-pills">
            {choreographies.map((ch) => (
              <button
                key={ch.id}
                id={`words-choreo-${ch.id}`}
                type="button"
                className={`lyrics-chip ${design.choreography === ch.id ? "active" : ""}`}
                title={ch.does}
                onClick={() => {
                  onUpdateDesign((draft) => {
                    const built = buildDefaultLineDesign(lineWords, ch.id);
                    draft.choreography = ch.id;
                    draft.items = built.items;
                  });
                }}
              >
                {ch.name}
              </button>
            ))}
          </div>
        </div>
      </details>

      {/* 3. Motion Styles & EffectCraft Keyframes (Motion Look) */}
      <details
        id="lyrics-motion-section"
        className="panel motion-section"
        data-look="motion"
        open={mode === "motion" || true}
      >
        <summary className="panel-head">
          <span className="motion-section-icon" aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <path d="M2 12c3-6 5-6 8 0s5 0 4-6" />
            </svg>
          </span>
          <span className="title">Motion &amp; EffectCraft Presets</span>
          <span className="meta">{item?.move ?? "pop"}</span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div className="lyrics-moves-grid">
            {moves.map((mv) => (
              <button
                key={mv.id}
                id={`words-move-${mv.id}`}
                type="button"
                className={`lyrics-move-card ${item?.move === mv.id ? "active" : ""}`}
                title={mv.does}
                onClick={() => patchItem({ move: mv.id })}
              >
                <span className="move-title">{mv.name}</span>
                <span className="move-desc">{mv.does}</span>
              </button>
            ))}
          </div>

          <div className="lyrics-sublabel">EffectCraft Text Animator Presets</div>
          <div className="lyrics-warp-pills">
            {textPresets.slice(0, 8).map((raw) => {
              const id =
                typeof raw === "string"
                  ? raw
                  : raw.id || raw.match_name || raw.name || "typewriter";
              const label =
                typeof raw === "string"
                  ? raw.replace(/_/g, " ")
                  : raw.name || id;
              return (
                <button
                  key={id}
                  id={`words-ec-preset-${id}`}
                  type="button"
                  className={`lyrics-chip ${item?.text_preset === id ? "active" : ""}`}
                  onClick={() => patchItem({ text_preset: id })}
                >
                  ✦ {label}
                </button>
              );
            })}
          </div>

          {/* Native EffectCraft Keyframe Curve Controls */}
          <div className="lyrics-ecproj-box">
            <div className="lyrics-ecproj-head">
              <strong>EffectCraft Keyframes</strong>
              <span className="muted">{compBindingLabel}</span>
            </div>
            <label className="inspector-field">
              <span>Channel</span>
              <select
                id="word-motion-property"
                value={motionSpec.property}
                onChange={(e) => setMotionSpec(defaultMotionSpec(e.target.value))}
              >
                {motionProps.map((mp) => (
                  <option key={mp.path} value={mp.path}>
                    {mp.name ?? mp.path}
                  </option>
                ))}
              </select>
            </label>
            <label className="inspector-field">
              <span>AniCraft Easing</span>
              <select
                id="word-motion-easing"
                value={motionSpec.easing}
                onChange={(e) =>
                  setMotionSpec((s) => ({ ...s, easing: e.target.value }))
                }
              >
                {easings.map((ea) => (
                  <option key={ea.id} value={ea.id}>
                    {ea.name}
                    {ea.bezier
                      ? ` (${ea.bezier.map((n) => n.toFixed(2)).join(", ")})`
                      : ""}
                  </option>
                ))}
              </select>
            </label>
            <label className="inspector-field">
              <span>Symbol Timeline</span>
              <select
                id="word-motion-loop-mode"
                value={motionSpec.loopMode}
                onChange={(e) =>
                  setMotionSpec((s) => ({
                    ...s,
                    loopMode: e.target.value as MotionSpec["loopMode"],
                  }))
                }
              >
                <option value="loop">Graphic · Loop</option>
                <option value="play_once">Graphic · Play Once</option>
                <option value="single_frame">Graphic · Single Frame</option>
              </select>
            </label>
            <div className="lyrics-dial-grid">
              <div className="craft-number" data-control="dial">
                <label className="craft-number-title" htmlFor="word-motion-delay-dial">
                  <span className="craft-control-label">Delay</span>
                  <output className="craft-number-readout">
                    {motionSpec.delay.toFixed(2)}s
                  </output>
                </label>
                <span className="craft-number-body">
                  <Dial
                    id="word-motion-delay"
                    label="Delay"
                    value={motionSpec.delay}
                    min={0}
                    max={5}
                    step={0.05}
                    unit="s"
                    onChange={(v) => setMotionSpec((s) => ({ ...s, delay: v }))}
                  />
                </span>
              </div>
              <div className="craft-number" data-control="dial">
                <label className="craft-number-title" htmlFor="word-motion-duration-dial">
                  <span className="craft-control-label">Duration</span>
                  <output className="craft-number-readout">
                    {motionSpec.duration.toFixed(2)}s
                  </output>
                </label>
                <span className="craft-number-body">
                  <Dial
                    id="word-motion-duration"
                    label="Duration"
                    value={motionSpec.duration}
                    min={0.1}
                    max={6}
                    step={0.05}
                    unit="s"
                    onChange={(v) => setMotionSpec((s) => ({ ...s, duration: v }))}
                  />
                </span>
              </div>
              {motionSpec.property === ROTATION ? (
                <div className="craft-number" data-control="dial">
                  <label className="craft-number-title" htmlFor="word-motion-speed-dial">
                    <span className="craft-control-label">Speed</span>
                    <output className="craft-number-readout">
                      {Math.round(motionSpec.speed)}°/s
                    </output>
                  </label>
                  <span className="craft-number-body">
                    <Dial
                      id="word-motion-speed"
                      label="Speed"
                      value={motionSpec.speed}
                      min={-360}
                      max={360}
                      step={5}
                      unit="°/s"
                      onChange={(v) => setMotionSpec((s) => ({ ...s, speed: v }))}
                    />
                  </span>
                </div>
              ) : null}
            </div>
            <button
              id="word-motion-apply"
              type="button"
              className="primary"
              disabled={motionBusy}
              onClick={() => void applyNativeKeyframes()}
            >
              {motionBusy ? "Writing to lyrics.ecproj…" : "Apply Keyframes to lyrics.ecproj"}
            </button>
          </div>
        </div>
      </details>

      {/* 4. Drawn Marks & Brushes (Chalk Look) */}
      <details
        id="lyrics-draw-section"
        className="panel motion-section"
        data-look="chalk"
      >
        <summary className="panel-head">
          <span className="motion-section-icon" aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <path d="M12.1 2.3a1 1 0 0 1 1.4 0l.2.2a1 1 0 0 1 0 1.4L5.5 12.1 2 13l.9-3.5L11.1 1.3z" />
            </svg>
          </span>
          <span className="title">Drawn Marks &amp; Brushes</span>
          <span className="meta">{ design.draws.length } marks</span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div className="lyrics-warp-pills">
            <button
              id="words-inspector-draw-toggle"
              type="button"
              className={`lyrics-chip ${drawMode ? "active" : ""}`}
              onClick={onToggleDrawMode}
            >
              {drawMode ? "✎ Drawing on Stage (Active)" : "✎ Draw Freehand on Stage"}
            </button>
            {design.draws.length > 0 && (
              <button
                id="words-clear-draws"
                type="button"
                className="ghost"
                onClick={() =>
                  onUpdateDesign((draft) => {
                    draft.draws = [];
                  })
                }
              >
                Clear Marks ({design.draws.length})
              </button>
            )}
          </div>

          <div className="lyrics-color-row">
            <label className="lyrics-color-picker">
              <span>Ink</span>
              <input
                id="words-brush-color"
                type="color"
                value={brushColor}
                onChange={(e) => onBrushColorChange(e.target.value)}
              />
            </label>
            <div className="lyrics-flex-1">
              <Slider
                id="words-brush-width"
                label="Brush Width"
                value={brushWidth}
                min={2}
                max={28}
                step={1}
                unit="px"
                onChange={onBrushWidthChange}
              />
            </div>
          </div>

          <div className="lyrics-sublabel">Vector Trim-Path Accents</div>
          <div className="lyrics-warp-pills">
            {strokes.map((st) => (
              <button
                key={st.id}
                id={`words-stroke-preset-${st.id}`}
                type="button"
                className="lyrics-chip"
                title={st.does}
                onClick={() => {
                  onUpdateDesign((draft) => {
                    const target = draft.items[activeItemIndex] ?? draft.items[0];
                    const bx = target ? target.x : 540;
                    const by = target ? target.y + (target.h ?? 130) - 16 : 620;
                    draft.draws.push({
                      name: st.name,
                      view_box: st.view_box,
                      x: bx,
                      y: by,
                      w: st.w,
                      h: st.h,
                      paths: [
                        {
                          d: st.d,
                          color: brushColor,
                          width: brushWidth,
                          from: 0,
                          to: st.draw || 0.8,
                        },
                      ],
                    });
                  });
                  onToast(`Added ${st.name} vector trim-path stroke`);
                }}
              >
                + {st.name}
              </button>
            ))}
          </div>
        </div>
      </details>

      {/* 5. Storyboard · Scene Context & Text (Cutout Look) */}
      <details
        id="lyrics-storyboard-section"
        className="panel motion-section"
        data-look="cutout"
      >
        <summary className="panel-head">
          <span className="motion-section-icon" aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <rect x="2" y="3" width="12" height="10" rx="1.5" />
            </svg>
          </span>
          <span className="title">Storyboard · Scene Context</span>
          <span className="meta">P{(currentSbPage?.page ?? 0) + 1}</span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div className="lyrics-sb-bar">
            <select
              id="words-sb-page-select"
              value={selectedSbPage}
              onChange={(e) => setSelectedSbPage(Number(e.target.value))}
            >
              {sbPages.map((pg) => (
                <option key={pg.page} value={pg.page}>
                  P{pg.page + 1} · {pg.title || pg.archetype || "Scene"} ({pg.layout || "Spread"})
                </option>
              ))}
            </select>
            <button
              id="words-sb-open-dc"
              type="button"
              className="ghost"
              onClick={() =>
                void executeCommand("storyboard.open", {}).catch((e) =>
                  onToast(String(e)),
                )
              }
            >
              DesignCraft ↗
            </button>
          </div>
          {textSlots.slice(0, 6).map((slot, i) => (
            <label
              key={slot.id || slot.label || i}
              className="lyrics-sb-slot"
            >
              <span>{(slot.label || slot.id || `Slot ${i + 1}`).toUpperCase()}</span>
              <input
                type="text"
                defaultValue={slot.text ?? ""}
                onBlur={(e) => {
                  if (!currentSbPage || !slot.label) return;
                  void executeCommand("storyboard.slot.set", {
                    page: currentSbPage.page,
                    slot: slot.label,
                    text: e.target.value,
                  }).catch((err) => onToast(String(err)));
                }}
              />
            </label>
          ))}
        </div>
      </details>
    </aside>
  );
};
