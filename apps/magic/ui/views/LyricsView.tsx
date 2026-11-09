import { convertFileSrc } from "@tauri-apps/api/core";
import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { executeCommand } from "../api";
import { takeAuthoringTime } from "../authoring-navigation";
import { LyricsInspector } from "./lyrics/LyricsInspector";
import {
  BackdropImage,
  buildDefaultLineDesign,
  effectiveOutlineColor,
  fontStackForLettering,
  LetteringPreset,
  LineDesign,
  LyricLineGroup,
  MovePreset,
  SceneClip,
  STAGE_H,
  STAGE_W,
  StrokePreset,
  TextPresetItem,
  warpSvgPathD,
  WordItem,
  WordsAnimation,
  WordsReadReply,
  WordToken,
} from "./lyrics/types";

export interface LyricsViewProps {
  projectPath: string;
  mode: "design" | "motion";
  onModeChange: (nextMode: "design" | "motion") => void;
  onNavigateStudio: () => void;
  onToast: (message: string) => void;
}

const DEFAULT_MOVES: MovePreset[] = [
  { id: "quiet", name: "Quiet", does: "Fades in and holds" },
  { id: "pop", name: "Bounce / Pop", does: "Swells on the beat" },
  { id: "loop", name: "Wave / Loop", does: "Continuous wave" },
  { id: "fall", name: "Kinetic Tilt", does: "Drops down from above" },
  { id: "rise", name: "Fire / Impact", does: "Arcs up from below" },
  { id: "east", name: "Neon Drift", does: "Slides right" },
  { id: "west", name: "Slide West", does: "Slides left" },
  { id: "settle", name: "Sparkle Settle", does: "Drops and settles into place" },
  { id: "sleep", name: "Sleep", does: "Letters sink one by one" },
];

const DEFAULT_LETTERINGS: LetteringPreset[] = [
  { id: "serif-italic", name: "Serif Italic", does: "Expressive hero word", font: "serif", weight: 400, italic: true },
  { id: "serif-bold", name: "Serif Bold", does: "Strong hero word", font: "serif", weight: 700, italic: false },
  { id: "song-bold", name: "Song Bold", does: "Heavy display sans", font: "song", weight: 700, italic: false },
  { id: "song", name: "Song Regular", does: "Clean lyric lettering", font: "song", weight: 400, italic: false },
  { id: "sans", name: "Sans Quiet", does: "Calm context words", font: "sans", weight: 400, italic: false },
];

const DEFAULT_CHOREOGRAPHIES = [
  { id: "cascade", name: "Cascade", does: "Diagonal staircase" },
  { id: "scatter", name: "Spiral / Scatter", does: "Tilted dynamic spread" },
  { id: "poster", name: "Zoom Blast", does: "Hero word giant & tilted" },
  { id: "stack", name: "Stack", does: "Vertical stepped column" },
  { id: "plain", name: "Plain", does: "Clean horizontal row" },
];

const formatTime = (sec: number): string => {
  if (!Number.isFinite(sec) || sec < 0) return "0:00.0";
  const m = Math.floor(sec / 60);
  const s = (sec % 60).toFixed(1).padStart(4, "0");
  return `${m}:${s}`;
};

export const LyricsView: React.FC<LyricsViewProps> = ({
  projectPath,
  mode,
  onModeChange,
  onNavigateStudio,
  onToast,
}) => {
  const [readData, setReadData] = useState<WordsReadReply | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [selectedLineKey, setSelectedLineKey] = useState<string | null>(null);
  const [activeItemIndex, setActiveItemIndex] = useState<number>(0);
  const [stageBgMode, setStageBgMode] = useState<"scene" | "paper">("scene");
  const [ecFrameUrl, setEcFrameUrl] = useState<string | null>(null);
  const [renderingEcFrame, setRenderingEcFrame] = useState<boolean>(false);
  const [publishingStudio, setPublishingStudio] = useState<boolean>(false);
  const [drawMode, setDrawMode] = useState<boolean>(false);
  const [brushColor, setBrushColor] = useState<string>("#FF9E3D");
  const [brushWidth, setBrushWidth] = useState<number>(8);
  const [playheadSec, setPlayheadSec] = useState<number>(0.8);
  const [waveformPeaks, setWaveformPeaks] = useState<number[]>([]);
  const [timelineMarkers, setTimelineMarkers] = useState<
    {
      id?: number | null;
      ec_index?: number;
      index: number;
      name: string;
      comment: string;
      kind: string;
      color: string;
      time_sec: number;
      duration_sec: number;
      source: string;
    }[]
  >([]);
  const [sceneTrackClips, setSceneTrackClips] = useState<
    {
      clip: number;
      name: string;
      start_sec: number;
      end_sec: number;
      label?: string;
    }[]
  >([]);

  // Local draft map keyed by `${animId}::${lineId}`
  const [draftDesigns, setDraftDesigns] = useState<Record<string, LineDesign>>({});

  // Refs for tether computation & interactive stage dragging
  const workspaceRef = useRef<HTMLDivElement | null>(null);
  const stageRef = useRef<HTMLDivElement | null>(null);
  const anchorRefs = useRef<Map<string, HTMLButtonElement>>(new Map());
  const boxRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const [tetherCoords, setTetherCoords] = useState<{
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    cx1: number;
    cy1: number;
    cx2: number;
    cy2: number;
  } | null>(null);

  const loadWords = useCallback(async () => {
    setLoading(true);
    try {
      const res = (await executeCommand("words.read", {})) as WordsReadReply;
      setReadData(res);
      const nextDrafts: Record<string, LineDesign> = {};
      let firstKey: string | null = null;
      let firstTime = 0.8;
      for (const anim of res.document?.animations ?? []) {
        const byLine = new Map<string, WordToken[]>();
        for (const w of anim.words ?? []) {
          const lid = w.line_id || "line-1";
          const arr = byLine.get(lid) ?? [];
          arr.push(w);
          byLine.set(lid, arr);
        }
        for (const [lid, lwords] of byLine.entries()) {
          const key = `${anim.animation_id}::${lid}`;
          if (!firstKey) {
            firstKey = key;
            firstTime = lwords[0]?.start ?? 0.8;
          }
          const saved = anim.design?.[lid];
          if (saved && Array.isArray(saved.items) && saved.items.length > 0) {
            nextDrafts[key] = structuredClone(saved);
          } else {
            nextDrafts[key] = buildDefaultLineDesign(lwords, "cascade");
          }
        }
      }
      setDraftDesigns(nextDrafts);
      const jumpSec = takeAuthoringTime(projectPath);
      if (typeof jumpSec === "number" && Number.isFinite(jumpSec)) {
        setPlayheadSec(jumpSec);
      } else {
        setPlayheadSec(firstTime);
      }
      setSelectedLineKey((prev) => prev ?? firstKey);
    } catch (err) {
      onToast(`Could not load lyrics: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  }, [onToast, projectPath]);

  const refreshSyncedTimeline = useCallback(() => {
    executeCommand("video.timeline", {})
      .then((res) => {
        const r = res as {
          markers?: typeof timelineMarkers;
          tracks?: {
            role?: string;
            label?: string;
            clips?: {
              clip: number;
              name?: string;
              start_tick?: number;
              end_tick?: number;
              label?: string;
            }[];
          }[];
        };
        if (Array.isArray(r.markers)) {
          setTimelineMarkers(r.markers);
        }
        const bgTrack = (r.tracks ?? []).find(
          (t) => t.role === "background" || t.label === "V1",
        );
        if (bgTrack && Array.isArray(bgTrack.clips)) {
          setSceneTrackClips(
            bgTrack.clips.map((c, idx) => ({
              clip: c.clip ?? idx,
              name: c.name || `Scene ${idx + 1}`,
              start_sec: (c.start_tick ?? 0) / 254016000000,
              end_sec: (c.end_tick ?? 0) / 254016000000,
              label: c.label,
            })),
          );
        }
      })
      .catch(() => {
        executeCommand("video.markers", { action: "list" })
          .then((res) => {
            const r = res as { markers?: typeof timelineMarkers };
            if (Array.isArray(r.markers)) setTimelineMarkers(r.markers);
          })
          .catch(() => {});
      });
  }, []);

  useEffect(() => {
    void loadWords();
    refreshSyncedTimeline();
    executeCommand("song.waveform", { bins: 180 })
      .then((res) => {
        const peaks = (res as { peaks?: number[] })?.peaks;
        if (Array.isArray(peaks) && peaks.length > 0) {
          setWaveformPeaks(peaks);
        }
      })
      .catch(() => {});
  }, [loadWords, refreshSyncedTimeline]);

  // Flatten all lyric lines across sections
  const allLineGroups = useMemo<LyricLineGroup[]>(() => {
    const out: LyricLineGroup[] = [];
    for (const anim of readData?.document?.animations ?? []) {
      const sectionName =
        anim.scope?.section ||
        anim.animation_id.replace(/-/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
      const byLine = new Map<string, WordToken[]>();
      for (const w of anim.words ?? []) {
        const lid = w.line_id || "line-1";
        const list = byLine.get(lid) ?? [];
        list.push(w);
        byLine.set(lid, list);
      }
      for (const [lineId, words] of byLine.entries()) {
        const start = words[0]?.start ?? anim.song_anchor ?? 0;
        const end = words[words.length - 1]?.end ?? start + 3.5;
        const text = words.map((w) => w.text).join(" ");
        out.push({
          lineId,
          animId: anim.animation_id,
          sectionName,
          words,
          start,
          end,
          text,
        });
      }
    }
    return out;
  }, [readData]);

  const filteredLines = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return allLineGroups;
    return allLineGroups.filter(
      (g) =>
        g.text.toLowerCase().includes(q) ||
        g.sectionName.toLowerCase().includes(q),
    );
  }, [allLineGroups, searchQuery]);

  const activeLine = useMemo(() => {
    if (!allLineGroups.length) return null;
    if (selectedLineKey) {
      const found = allLineGroups.find(
        (g) => `${g.animId}::${g.lineId}` === selectedLineKey,
      );
      if (found) return found;
    }
    return allLineGroups[0];
  }, [allLineGroups, selectedLineKey]);

  const activeAnim = useMemo<WordsAnimation | undefined>(() => {
    if (!activeLine) return undefined;
    return readData?.document?.animations?.find(
      (a) => a.animation_id === activeLine.animId,
    );
  }, [activeLine, readData]);

  const activeKey = activeLine
    ? `${activeLine.animId}::${activeLine.lineId}`
    : "";

  const activeDesign: LineDesign = useMemo(() => {
    if (!activeLine) return { choreography: "cascade", items: [], draws: [] };
    return (
      draftDesigns[activeKey] ?? buildDefaultLineDesign(activeLine.words, "cascade")
    );
  }, [activeKey, activeLine, draftDesigns]);

  const updateActiveDesign = useCallback(
    (updater: (draft: LineDesign) => void) => {
      if (!activeLine) return;
      const key = `${activeLine.animId}::${activeLine.lineId}`;
      setDraftDesigns((prev) => {
        const base = prev[key]
          ? structuredClone(prev[key])
          : buildDefaultLineDesign(activeLine.words, "cascade");
        updater(base);
        return { ...prev, [key]: base };
      });
      setEcFrameUrl(null);
    },
    [activeLine],
  );

  // Save current animation's line designs to words.json and sync lettering/lyrics.ecproj
  const saveAndSyncEcproj = useCallback(async () => {
    if (!activeAnim || !activeLine) return;
    const designPayload: Record<string, LineDesign> = {};
    for (const [k, d] of Object.entries(draftDesigns)) {
      const [aid, lid] = k.split("::");
      if (aid === activeAnim.animation_id && lid) {
        designPayload[lid] = d;
      }
    }
    await executeCommand("words.save", {
      animation_id: activeAnim.animation_id,
      background_asset: activeAnim.background_asset ?? null,
      design: designPayload,
    });
    const letterRes = (await executeCommand("words.letter", {
      render: false,
    })) as { saved_by?: string; schema?: number };
    const refreshed = (await executeCommand("words.read", {})) as WordsReadReply;
    setReadData(refreshed);
    onToast(
      `Saved design/words.json & lettering/lyrics.ecproj (schema ${letterRes.schema ?? 2})`,
    );
  }, [activeAnim, activeLine, draftDesigns, onToast]);

  const previewEffectCraftStage = useCallback(async () => {
    if (!activeAnim || !activeLine) return;
    setRenderingEcFrame(true);
    try {
      await saveAndSyncEcproj();
      const res = (await executeCommand("words.preview_frame", {
        kind: "stage",
        animation_id: activeAnim.animation_id,
        line_id: activeLine.lineId,
        seconds: playheadSec,
      })) as { path?: string };
      if (res?.path) {
        setEcFrameUrl(`${convertFileSrc(res.path)}?t=${Date.now()}`);
        onToast("Rendered live frame from lettering/lyrics.ecproj via EffectCraft");
      }
    } catch (err) {
      onToast(`EffectCraft frame preview error: ${String(err)}`);
    } finally {
      setRenderingEcFrame(false);
    }
  }, [activeAnim, activeLine, onToast, playheadSec, saveAndSyncEcproj]);

  const renderLyricsToStudio = useCallback(async () => {
    setPublishingStudio(true);
    try {
      await saveAndSyncEcproj();
      await executeCommand("words.letter", { render: true, studio: true });
      onToast("Rendered transparent lettering & published to Studio");
      onNavigateStudio();
    } catch (err) {
      onToast(`Render to Studio failed: ${String(err)}`);
    } finally {
      setPublishingStudio(false);
    }
  }, [onNavigateStudio, onToast, saveAndSyncEcproj]);

  // Backdrop image resolution
  const images: BackdropImage[] = readData?.images ?? [];
  const sceneClips: SceneClip[] = readData?.scene_clips ?? [];
  const activeBackdrop = useMemo(() => {
    const wanted = activeAnim?.background_asset;
    if (wanted) {
      const found = images.find((img) => img.id === wanted || img.asset_id === wanted);
      if (found) return found;
    }
    return images[0] ?? null;
  }, [activeAnim?.background_asset, images]);

  const selectBackdrop = async (imgId: string) => {
    if (!activeAnim) return;
    try {
      await executeCommand("words.save", {
        animation_id: activeAnim.animation_id,
        background_asset: imgId,
      });
      const refreshed = (await executeCommand("words.read", {})) as WordsReadReply;
      setReadData(refreshed);
      onToast(`Scene backdrop set to ${imgId}`);
    } catch (err) {
      onToast(String(err));
    }
  };

  // Recompute musical-note tether coordinates whenever selection or layout changes
  const updateTether = useCallback(() => {
    const ws = workspaceRef.current;
    const anchor = selectedLineKey ? anchorRefs.current.get(selectedLineKey) : null;
    const box = boxRefs.current.get(activeItemIndex) ?? boxRefs.current.get(0);
    if (!ws || !anchor || !box) {
      setTetherCoords(null);
      return;
    }
    const wsRect = ws.getBoundingClientRect();
    const aRect = anchor.getBoundingClientRect();
    const bRect = box.getBoundingClientRect();
    const x1 = aRect.left + aRect.width * 0.5 - wsRect.left;
    const y1 = aRect.top + aRect.height * 0.5 - wsRect.top;
    const x2 = bRect.left - wsRect.left;
    const y2 = bRect.top + bRect.height * 0.5 - wsRect.top;
    const dx = Math.max(80, Math.abs(x2 - x1) * 0.45);
    setTetherCoords({
      x1,
      y1,
      x2,
      y2,
      cx1: x1 + dx,
      cy1: y1 - 24,
      cx2: x2 - dx,
      cy2: y2 + 18,
    });
  }, [activeItemIndex, selectedLineKey]);

  useEffect(() => {
    const id = window.requestAnimationFrame(updateTether);
    window.addEventListener("resize", updateTether);
    return () => {
      window.cancelAnimationFrame(id);
      window.removeEventListener("resize", updateTether);
    };
  }, [updateTether, activeDesign, mode]);

  // Pointer drag state for moving, resizing, rotating, or setting motion trajectory on stage
  const startBoxPointerDrag = (
    e: React.PointerEvent,
    idx: number,
    action: "move" | "resize" | "rotate" | "motion-target",
  ) => {
    e.stopPropagation();
    setActiveItemIndex(idx);
    const stageEl = stageRef.current;
    const item = activeDesign.items[idx];
    if (!stageEl || !item) return;
    const rect = stageEl.getBoundingClientRect();
    const scaleX = STAGE_W / Math.max(1, rect.width);
    const scaleY = STAGE_H / Math.max(1, rect.height);
    const startClientX = e.clientX;
    const startClientY = e.clientY;
    const origX = item.x;
    const origY = item.y;
    const origW = item.w ?? 640;
    const origH = item.h ?? 150;
    const origRot = item.rotate ?? 0;
    const origDx = item.dx ?? 0;
    const origDy = item.dy ?? -32;

    const onMove = (mv: PointerEvent) => {
      const dxStage = (mv.clientX - startClientX) * scaleX;
      const dyStage = (mv.clientY - startClientY) * scaleY;
      updateActiveDesign((draft) => {
        const target = draft.items[idx];
        if (!target) return;
        if (action === "move") {
          target.x = Math.round(Math.max(20, Math.min(STAGE_W - 120, origX + dxStage)));
          target.y = Math.round(Math.max(20, Math.min(STAGE_H - 80, origY + dyStage)));
        } else if (action === "resize") {
          target.w = Math.round(Math.max(140, Math.min(STAGE_W - target.x, origW + dxStage)));
          target.h = Math.round(Math.max(60, Math.min(STAGE_H - target.y, origH + dyStage)));
        } else if (action === "rotate") {
          const nextRot = Math.round(origRot + dxStage * 0.45);
          target.rotate = Math.max(-360, Math.min(360, nextRot));
        } else if (action === "motion-target") {
          target.dx = Math.round(origDx + dxStage);
          target.dy = Math.round(origDy + dyStage);
        }
      });
    };

    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  // Freehand drawing on the 16:9 stage
  const [drawingPoints, setDrawingPoints] = useState<[number, number][] | null>(null);
  const onStageDrawDown = (e: React.PointerEvent<SVGSVGElement>) => {
    if (!drawMode) return;
    const svg = e.currentTarget;
    const rect = svg.getBoundingClientRect();
    const x = Math.round(((e.clientX - rect.left) / Math.max(1, rect.width)) * STAGE_W);
    const y = Math.round(((e.clientY - rect.top) / Math.max(1, rect.height)) * STAGE_H);
    setDrawingPoints([[x, y]]);
  };
  const onStageDrawMove = (e: React.PointerEvent<SVGSVGElement>) => {
    if (!drawMode || !drawingPoints) return;
    const svg = e.currentTarget;
    const rect = svg.getBoundingClientRect();
    const x = Math.round(((e.clientX - rect.left) / Math.max(1, rect.width)) * STAGE_W);
    const y = Math.round(((e.clientY - rect.top) / Math.max(1, rect.height)) * STAGE_H);
    setDrawingPoints((prev) => (prev ? [...prev, [x, y]] : [[x, y]]));
  };
  const onStageDrawUp = () => {
    if (!drawMode || !drawingPoints || drawingPoints.length < 2) {
      setDrawingPoints(null);
      return;
    }
    const d = drawingPoints
      .map((pt, i) => `${i === 0 ? "M" : "L"} ${pt[0]} ${pt[1]}`)
      .join(" ");
    updateActiveDesign((draft) => {
      draft.draws.push({
        name: "Freehand Brush",
        view_box: `0 0 ${STAGE_W} ${STAGE_H}`,
        x: 0,
        y: 0,
        w: STAGE_W,
        h: STAGE_H,
        paths: [
          {
            d,
            color: brushColor,
            width: brushWidth,
            from: 0,
            to: 0.9,
          },
        ],
      });
    });
    setDrawingPoints(null);
  };

  const totalSongDuration = useMemo(() => {
    const last = allLineGroups[allLineGroups.length - 1];
    return Math.max(30, (last?.end ?? 118) + 4);
  }, [allLineGroups]);

  const moves = readData?.tools?.moves?.length ? readData.tools.moves : DEFAULT_MOVES;
  const letterings = readData?.tools?.letterings?.length
    ? readData.tools.letterings
    : DEFAULT_LETTERINGS;
  const strokes: StrokePreset[] = readData?.tools?.strokes ?? [];
  const choreographies = readData?.tools?.choreographies?.length
    ? readData.tools.choreographies
    : DEFAULT_CHOREOGRAPHIES;
  const easings = readData?.tools?.easings?.length
    ? readData.tools.easings
    : [
        { id: "linear", name: "Linear", does: "Linear" },
        { id: "quad_in", name: "Ease in (quad)", does: "Ease in (quad)" },
        { id: "quad_out", name: "Ease out (quad)", does: "Ease out (quad)" },
        { id: "quad_in_out", name: "Ease in-out (quad)", does: "Ease in-out (quad)" },
        { id: "cubic_in", name: "Ease in (cubic)", does: "Ease in (cubic)" },
        { id: "cubic_out", name: "Ease out (cubic)", does: "Ease out (cubic)" },
        { id: "cubic_in_out", name: "Ease in-out (cubic)", does: "Ease in-out (cubic)" },
        { id: "smooth", name: "Smooth (bezier)", does: "Smooth (bezier)" },
      ];
  const textPresets: (string | TextPresetItem)[] = readData?.tools?.effectcraft
    ?.text_presets?.length
    ? readData.tools.effectcraft.text_presets
    : [
        "typewriter",
        "fade_up_characters",
        "bounce_in_words",
        "tracking_in",
        "scramble",
        "blur_in",
      ];

  const ecprojMeta = readData?.ecproj;

  return (
    <div
      ref={workspaceRef}
      id="lyrics-workspace-root"
      className="lyrics-native-workspace"
      data-authoring-mode={mode}
    >
      {/* Musical-Note Curved Tether Linking Selected Moment Card -> Stage Word Box */}
      {tetherCoords && (
        <svg
          id="words-tether-svg"
          className="lyrics-tether-svg"
          aria-hidden="true"
        >
          <defs>
            <path
              id="words-tether-curve-def"
              d={`M ${tetherCoords.x1} ${tetherCoords.y1} C ${tetherCoords.cx1} ${tetherCoords.cy1}, ${tetherCoords.cx2} ${tetherCoords.cy2}, ${tetherCoords.x2} ${tetherCoords.y2}`}
            />
          </defs>
          <use
            href="#words-tether-curve-def"
            className="lyrics-tether-glow"
          />
          <use
            href="#words-tether-curve-def"
            className="lyrics-tether-stroke"
          />
          <text className="lyrics-tether-notes" dy="-6">
            <textPath href="#words-tether-curve-def" startOffset="28%">
              ♪ &nbsp;&nbsp;&nbsp; ♫ &nbsp;&nbsp;&nbsp; ♬
            </textPath>
          </text>
          <circle
            cx={tetherCoords.x2}
            cy={tetherCoords.y2}
            r={6.5}
            className="lyrics-tether-target-dot"
          />
        </svg>
      )}

      <div className="lyrics-main-columns">
        {/* LEFT RAIL: Lyric Moments with 16:9 Paper/Scene Miniature Thumbnails */}
        <aside id="words-lyrics" className="lyrics-moments-rail">
          <div className="lyrics-moments-header">
            <div className="lyrics-moments-title-row">
              <strong>Lyric moments</strong>
              <span className="muted">Artist words · read-only</span>
            </div>
            <input
              id="words-lyrics-search"
              type="search"
              placeholder="Filter lyrics…"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>

          <div className="lyrics-moments-list" onScroll={updateTether}>
            {loading && !allLineGroups.length ? (
              <p className="muted" style={{ padding: "12px" }}>
                Loading lyric moments…
              </p>
            ) : null}
            {filteredLines.map((group, idx) => {
              const key = `${group.animId}::${group.lineId}`;
              const isSelected = activeKey === key;
              const lineDesign =
                draftDesigns[key] ?? buildDefaultLineDesign(group.words, "cascade");
              const prevSection = idx > 0 ? filteredLines[idx - 1].sectionName : null;
              const showSectionHead = group.sectionName !== prevSection;
              const leadItem = lineDesign.items[0];
              return (
                <React.Fragment key={key}>
                  {showSectionHead && (
                    <div className="lyrics-section-divider">
                      <span className="badge">{group.sectionName}</span>
                      <span className="muted">{formatTime(group.start)}</span>
                    </div>
                  )}
                  <article
                    id={`words-moment-${group.animId}-${group.lineId}`}
                    className={`lyrics-moment-card ${isSelected ? "active" : ""}`}
                    onClick={() => {
                      setSelectedLineKey(key);
                      setActiveItemIndex(0);
                      setPlayheadSec(group.start);
                    }}
                  >
                    <div className="lyrics-moment-top">
                      {activeBackdrop && (
                        <span className="lyrics-moment-scene-thumb">
                          <img
                            src={convertFileSrc(activeBackdrop.path)}
                            alt=""
                          />
                          <em>16:9</em>
                        </span>
                      )}
                      <div className="lyrics-moment-meta">
                        <div className="lyrics-moment-line-text">
                          <span className="time">[{formatTime(group.start)}]</span>{" "}
                          {group.text}
                        </div>
                        <div className="lyrics-moment-badges">
                          <span className="pill-amber">
                            ⚡ {leadItem?.move ?? "pop"}
                          </span>
                          {leadItem?.warp_path && leadItem.warp_path !== "none" && (
                            <span className="pill-teal">
                              ⤴ {leadItem.warp_path}
                            </span>
                          )}
                          {lineDesign.draws.length > 0 && (
                            <span className="pill-emerald">
                              ✎ {lineDesign.draws.length} stroke
                            </span>
                          )}
                        </div>
                      </div>
                      <button
                        ref={(el) => {
                          if (el) anchorRefs.current.set(key, el);
                          else anchorRefs.current.delete(key);
                        }}
                        type="button"
                        className="lyrics-moment-anchor-btn"
                        title="Tethered to 16:9 stage"
                      >
                        ⚓
                      </button>
                    </div>

                    {/* Miniature 16:9 Paper Preview */}
                    <svg
                      viewBox={`0 0 ${STAGE_W} ${STAGE_H}`}
                      className="lyrics-moment-mini-svg"
                    >
                      <rect
                        width={STAGE_W}
                        height={STAGE_H}
                        rx={32}
                        fill="#F5EFE4"
                      />
                      {lineDesign.items.map((it, i) => {
                        const txt = it.word_ids
                          .map((wid) => group.words.find((w) => w.word_id === wid)?.text)
                          .filter(Boolean)
                          .join(" ");
                        const font = fontStackForLettering(it.lettering);
                        const cx = it.x + (it.w ?? 600) * 0.5;
                        const cy = it.y + (it.h ?? 130) * 0.65;
                        const outline = effectiveOutlineColor(it);
                        return (
                          <g
                            key={i}
                            transform={`translate(${cx}, ${cy}) rotate(${it.rotate ?? 0}) skewX(${it.skew ?? 0}) scale(${(it.scale_x ?? 100) / 100}, ${(it.scale_y ?? 100) / 100})`}
                          >
                            <text
                              textAnchor="middle"
                              fill={it.color || "#173F49"}
                              stroke={outline ?? "none"}
                              strokeWidth={outline ? (it.stroke_width ?? 6) * 1.4 : 0}
                              style={{
                                fontFamily: font.fontFamily,
                                fontWeight: font.fontWeight,
                                fontStyle: font.fontStyle,
                                fontSize: `${Math.max(46, it.size ?? 84)}px`,
                                paintOrder: "stroke fill",
                              }}
                            >
                              {txt}
                            </text>
                          </g>
                        );
                      })}
                    </svg>

                    <div className="lyrics-moment-foot">
                      <span>
                        {formatTime(group.start)} – {formatTime(group.end)}
                      </span>
                      <span className="hero-tag">
                        ✎ &ldquo;
                        {(leadItem?.word_ids ?? [])
                          .map((wid) => group.words.find((w) => w.word_id === wid)?.text)
                          .filter(Boolean)
                          .join(" ") || group.words[0]?.text}
                        &rdquo; ({lineDesign.items.length})
                      </span>
                    </div>
                  </article>
                </React.Fragment>
              );
            })}
          </div>
        </aside>

        {/* CENTER STAGE & TIMELINE */}
        <main id="words-stage-panel" className="lyrics-center-column">
          {/* Header Bar */}
          <header className="lyrics-stage-topbar">
            <div className="lyrics-stage-title">
              <strong>
                {activeLine
                  ? `${activeLine.sectionName} · ${activeLine.lineId}`
                  : "Lyrics & Motion"}
              </strong>
              {ecprojMeta?.path && (
                <span className="lyrics-ecproj-pill" title={ecprojMeta.path}>
                  lettering/lyrics.ecproj · schema {ecprojMeta.schema ?? 2}
                  {ecprojMeta.layer_count
                    ? ` · ${ecprojMeta.layer_count} layers`
                    : ""}
                </span>
              )}
            </div>

            <div className="lyrics-mode-tabs">
              <button
                id="words-mode-design"
                type="button"
                className={`lyrics-mode-btn ${mode === "design" ? "active" : ""}`}
                onClick={() => onModeChange("design")}
              >
                T Lyrics
              </button>
              <button
                id="words-mode-motion"
                type="button"
                className={`lyrics-mode-btn ${mode === "motion" ? "active" : ""}`}
                onClick={() => onModeChange("motion")}
              >
                ↗ Motion
              </button>
              <button
                id="words-draw-toggle"
                type="button"
                className={`lyrics-mode-btn ${drawMode ? "active" : ""}`}
                title="Draw freehand vector overlay strokes on stage"
                onClick={() => setDrawMode((d) => !d)}
              >
                ✎ Draw
              </button>
              <button
                id="words-save"
                type="button"
                className="lyrics-mode-btn primary"
                title="Save words.json & rebuild lettering/lyrics.ecproj"
                onClick={() => void saveAndSyncEcproj()}
              >
                💾 Save
              </button>
            </div>
          </header>

          {/* Scene & Video Backdrop Strip */}
          <div id="words-scene-strip" className="lyrics-scene-strip">
            <span className="strip-label">Scene</span>
            {sceneClips.slice(0, 3).map((clip) => (
              <button
                key={clip.id}
                type="button"
                className="scene-video-pill"
                title={clip.label}
                onClick={() => setPlayheadSec(clip.start)}
              >
                ▶ {clip.id}
              </button>
            ))}
            <div className="scene-thumbs-scroll">
              {images.map((img) => {
                const selected = activeBackdrop?.id === img.id;
                return (
                  <button
                    key={img.id}
                    id={`words-bg-thumb-${img.id}`}
                    type="button"
                    className={`scene-thumb-btn ${selected ? "active" : ""}`}
                    title={img.name}
                    onClick={() => void selectBackdrop(img.id)}
                  >
                    <img src={convertFileSrc(img.path)} alt={img.name} />
                  </button>
                );
              })}
            </div>
            <button
              id="words-open-effectcraft"
              type="button"
              className="ghost"
              title="Open lettering/lyrics.ecproj in EffectCraft"
              onClick={() =>
                void saveAndSyncEcproj().then(() =>
                  executeCommand("workspace.open", {
                    app: "effectcraft",
                    path: "lettering/lyrics.ecproj",
                  }).catch((e) => onToast(String(e))),
                )
              }
            >
              EffectCraft ↗
            </button>
          </div>

          {/* 16:9 Interactive Stage */}
          <div className="lyrics-stage-viewport">
            <div
              ref={stageRef}
              id="words-stage"
              className={`lyrics-stage-canvas ${stageBgMode === "paper" ? "paper-mode" : "scene-mode"}`}
            >
              {/* Backdrop image when in Scene mode */}
              {stageBgMode === "scene" && activeBackdrop && (
                <img
                  className="lyrics-stage-backdrop-img"
                  src={convertFileSrc(activeBackdrop.path)}
                  alt=""
                  draggable={false}
                />
              )}

              {/* Optional live-rendered EffectCraft frame from lettering/lyrics.ecproj */}
              {ecFrameUrl && (
                <img
                  className="lyrics-stage-ecproj-overlay"
                  src={ecFrameUrl}
                  alt="EffectCraft live frame"
                  draggable={false}
                />
              )}

              {/* Floating Top Action Pills on Stage */}
              <div className="lyrics-stage-floating-pills">
                <button
                  id="words-pill-kinetic"
                  type="button"
                  className={`stage-pill ${mode === "motion" ? "active" : ""}`}
                  onClick={() =>
                    onModeChange(mode === "motion" ? "design" : "motion")
                  }
                >
                  + Kinetic Motion
                </button>
                <button
                  id="words-pill-draw"
                  type="button"
                  className={`stage-pill ${drawMode ? "active" : ""}`}
                  onClick={() => setDrawMode((v) => !v)}
                >
                  ✎ Draw Motion Overlay
                </button>
                <button
                  id="words-pill-warp"
                  type="button"
                  className={`stage-pill ${
                    (activeDesign.items[activeItemIndex]?.warp_path ?? "none") !==
                    "none"
                      ? "active-amber"
                      : ""
                  }`}
                  onClick={() => {
                    const order: WordItem["warp_path"][] = [
                      "none",
                      "arc_up",
                      "arc_down",
                      "wave",
                      "circle",
                    ];
                    const cur =
                      activeDesign.items[activeItemIndex]?.warp_path ?? "none";
                    const next = order[(order.indexOf(cur) + 1) % order.length];
                    updateActiveDesign((draft) => {
                      const target =
                        draft.items[activeItemIndex] ?? draft.items[0];
                      if (target) target.warp_path = next;
                    });
                  }}
                >
                  ⤴ Warp Path
                </button>
                <button
                  id="words-pill-ec-render"
                  type="button"
                  className={`stage-pill ${ecFrameUrl ? "active-amber" : ""}`}
                  disabled={renderingEcFrame}
                  onClick={() => void previewEffectCraftStage()}
                >
                  {renderingEcFrame ? "Rendering EC…" : "✦ EC Render"}
                </button>
              </div>

              {/* Motion Trajectory Dashed Vectors & Drawn Strokes SVG Layer */}
              <svg
                id="words-draw-svg"
                viewBox={`0 0 ${STAGE_W} ${STAGE_H}`}
                className={`lyrics-stage-svg-overlay ${drawMode ? "drawing-enabled" : ""}`}
                onPointerDown={onStageDrawDown}
                onPointerMove={onStageDrawMove}
                onPointerUp={onStageDrawUp}
              >
                {/* Motion Trajectory Arrows for every item on stage */}
                {activeDesign.items.map((it, idx) => {
                  const bw = it.w ?? 640;
                  const bh = it.h ?? 150;
                  const cx = it.x + bw * 0.5;
                  const cy = it.y + bh * 0.5;
                  const dx = it.dx ?? 0;
                  const dy = it.dy ?? -36;
                  if (Math.hypot(dx, dy) < 4) return null;
                  const tx = cx + dx;
                  const ty = cy + dy;
                  return (
                    <g key={`traj-${idx}`} className="lyrics-motion-trajectory">
                      <path
                        d={`M ${cx} ${cy} Q ${(cx + tx) * 0.5} ${Math.min(cy, ty) - 36} ${tx} ${ty}`}
                        fill="none"
                        stroke="#2AF5D6"
                        strokeWidth={3.5}
                        strokeDasharray="10 8"
                      />
                      <circle cx={tx} cy={ty} r={8} fill="#2AF5D6" />
                    </g>
                  );
                })}

                {/* Saved Vector / Brush Strokes */}
                {activeDesign.draws.map((dr, di) => {
                  const tx = dr.x ?? 0;
                  const ty = dr.y ?? 0;
                  return (
                    <g key={`draw-${di}`} transform={`translate(${tx}, ${ty})`}>
                      {dr.paths.map((p, pi) => (
                        <path
                          key={pi}
                          d={p.d}
                          fill="none"
                          stroke={p.color || "#FF9E3D"}
                          strokeWidth={p.width || 8}
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        />
                      ))}
                    </g>
                  );
                })}

                {/* Active Freehand Stroke In Progress */}
                {drawingPoints && drawingPoints.length > 1 && (
                  <path
                    d={drawingPoints
                      .map((pt, i) => `${i === 0 ? "M" : "L"} ${pt[0]} ${pt[1]}`)
                      .join(" ")}
                    fill="none"
                    stroke={brushColor}
                    strokeWidth={brushWidth}
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                )}
              </svg>

              {/* Interactive Draggable / Rotatable / Skewable / Warpable Word Boxes */}
              {activeDesign.items.map((item, idx) => {
                const isSelected = idx === activeItemIndex;
                const text = (item.word_ids ?? [])
                  .map(
                    (wid) =>
                      activeLine?.words.find((w) => w.word_id === wid)?.text,
                  )
                  .filter(Boolean)
                  .join(" ");
                const bw = item.w ?? 640;
                const bh = item.h ?? 150;
                const leftPct = (item.x / STAGE_W) * 100;
                const topPct = (item.y / STAGE_H) * 100;
                const widthPct = (bw / STAGE_W) * 100;
                const heightPct = (bh / STAGE_H) * 100;
                const font = fontStackForLettering(item.lettering);
                const outline = effectiveOutlineColor(item);
                const strokePx = outline ? item.stroke_width ?? 6 : 0;
                const warpD = warpSvgPathD(item.warp_path, bw, bh);
                const warpId = `stage-warp-path-${idx}`;

                return (
                  <div
                    key={`${item.word_ids.join("_")}_${idx}`}
                    ref={(el) => {
                      if (el) boxRefs.current.set(idx, el);
                      else boxRefs.current.delete(idx);
                    }}
                    id={`word-box-${idx}`}
                    className={`lyrics-stage-word-box ${isSelected ? "selected" : ""}`}
                    style={{
                      left: `${leftPct}%`,
                      top: `${topPct}%`,
                      width: `${widthPct}%`,
                      height: `${heightPct}%`,
                      transform: `rotate(${item.rotate ?? 0}deg) skewX(${item.skew ?? 0}deg) scale(${(item.scale_x ?? 100) / 100}, ${(item.scale_y ?? 100) / 100})`,
                    }}
                    onPointerDown={(e) => startBoxPointerDrag(e, idx, "move")}
                  >
                    <svg
                      viewBox={`0 0 ${bw} ${bh}`}
                      className="lyrics-word-box-svg"
                    >
                      {warpD && (
                        <defs>
                          <path id={warpId} d={warpD} />
                        </defs>
                      )}
                      {warpD ? (
                        <>
                          {isSelected && (
                            <path
                              d={warpD}
                              fill="none"
                              stroke="rgba(255, 190, 92, 0.55)"
                              strokeWidth={2}
                              strokeDasharray="6 6"
                            />
                          )}
                          <text
                            fill={item.color || "#D95B38"}
                            stroke={outline ?? "none"}
                            strokeWidth={strokePx * 2}
                            letterSpacing={item.tracking ?? 0}
                            style={{
                              fontFamily: font.fontFamily,
                              fontWeight: font.fontWeight,
                              fontStyle: font.fontStyle,
                              fontSize: `${item.size ?? 88}px`,
                              paintOrder: "stroke fill",
                            }}
                          >
                            <textPath
                              href={`#${warpId}`}
                              startOffset="50%"
                              textAnchor="middle"
                            >
                              {text}
                            </textPath>
                          </text>
                        </>
                      ) : (
                        <text
                          x={bw * 0.5}
                          y={bh * 0.68}
                          textAnchor="middle"
                          fill={item.color || "#D95B38"}
                          stroke={outline ?? "none"}
                          strokeWidth={strokePx * 2}
                          letterSpacing={item.tracking ?? 0}
                          style={{
                            fontFamily: font.fontFamily,
                            fontWeight: font.fontWeight,
                            fontStyle: font.fontStyle,
                            fontSize: `${item.size ?? 88}px`,
                            paintOrder: "stroke fill",
                          }}
                        >
                          {text}
                        </text>
                      )}
                    </svg>

                    {/* Rotation Stem & Resize Handles when Selected */}
                    {isSelected && (
                      <>
                        <button
                          id={`word-box-rotate-${idx}`}
                          type="button"
                          className="word-handle-rotate"
                          title="Drag horizontally to rotate (-360°..360°)"
                          onPointerDown={(e) =>
                            startBoxPointerDrag(e, idx, "rotate")
                          }
                        />
                        <button
                          id={`word-box-resize-${idx}`}
                          type="button"
                          className="word-handle-resize se"
                          title="Drag to resize word box"
                          onPointerDown={(e) =>
                            startBoxPointerDrag(e, idx, "resize")
                          }
                        />
                        {mode === "motion" && (
                          <button
                            id={`word-box-motion-target-${idx}`}
                            type="button"
                            className="word-handle-motion"
                            title="Drag to set motion trajectory offset"
                            onPointerDown={(e) =>
                              startBoxPointerDrag(e, idx, "motion-target")
                            }
                          />
                        )}
                      </>
                    )}
                  </div>
                );
              })}

              <div className="lyrics-stage-hint-pill">
                {mode === "motion"
                  ? "Drag any word to adjust spatial trajectory · Use dials on right to keyframe in lyrics.ecproj"
                  : "Drag to position · Top circle to rotate 360° · Corner to resize"}
              </div>
            </div>
          </div>

          {/* Transport & Export Bar */}
          <div className="lyrics-transport-bar">
            <input
              id="words-scrub"
              type="range"
              min={0}
              max={totalSongDuration}
              step={0.05}
              value={playheadSec}
              onChange={(e) => setPlayheadSec(Number(e.target.value))}
            />
            <span id="words-time" className="lyrics-time-readout">
              {formatTime(playheadSec)} / {formatTime(totalSongDuration)}
            </span>
            <button
              id="words-add-marker"
              type="button"
              className="ghost"
              title="Add synced Cue/Range Marker at playhead in FilmCraft (.fcproj) and EffectCraft (lyrics.ecproj)"
              onClick={() => {
                const cueName = activeLine
                  ? `${activeLine.sectionName} · ${activeLine.lineId}`
                  : `cue_${formatTime(playheadSec)}`;
                const cueComment = activeLine?.text || cueName;
                void executeCommand("video.markers", {
                  action: "add",
                  seconds: playheadSec,
                  name: cueName,
                  comment: cueComment,
                  color: "Iris",
                  kind: "flashCue",
                })
                  .then((res) => {
                    const r = res as { markers?: typeof timelineMarkers };
                    if (Array.isArray(r.markers)) setTimelineMarkers(r.markers);
                    onToast(
                      `Added marker "${cueName}" at ${formatTime(playheadSec)} in FilmCraft & EffectCraft`,
                    );
                  })
                  .catch((err) => onToast(String(err)));
              }}
            >
              ◆ + Marker (M)
            </button>
            <button
              id="words-seed-markers"
              type="button"
              className="ghost"
              title="Seed section Range Markers & line Flash Cues across FilmCraft (.fcproj) and EffectCraft (lyrics.ecproj)"
              onClick={() => {
                void executeCommand("video.markers", {
                  action: "seed",
                })
                  .then((res) => {
                    const r = res as {
                      seeded_sections?: number;
                      seeded_lines?: number;
                      markers?: typeof timelineMarkers;
                    };
                    if (Array.isArray(r.markers)) setTimelineMarkers(r.markers);
                    onToast(
                      `Synced ${r.seeded_sections ?? 0} section range markers & ${r.seeded_lines ?? 0} line cues (FilmCraft ↔ EffectCraft)`,
                    );
                  })
                  .catch((err) => onToast(String(err)));
              }}
            >
              ⇄ Sync Cues ({timelineMarkers.length})
            </button>
            <div className="lyrics-bg-mode-toggle">
              <button
                id="words-bg-scene"
                type="button"
                className={stageBgMode === "scene" ? "active" : ""}
                onClick={() => setStageBgMode("scene")}
              >
                Scene
              </button>
              <button
                id="words-bg-paper"
                type="button"
                className={stageBgMode === "paper" ? "active" : ""}
                onClick={() => setStageBgMode("paper")}
              >
                Paper
              </button>
            </div>
            <button
              id="words-render-studio"
              type="button"
              className="ghost"
              disabled={publishingStudio}
              onClick={() => void renderLyricsToStudio()}
            >
              {publishingStudio ? "Rendering…" : "Render Lyrics to Studio"}
            </button>
            <button
              id="words-export-film"
              type="button"
              className="primary-emerald"
              onClick={onNavigateStudio}
            >
              Export Film
            </button>
          </div>

          {/* Bottom Multi-Lane Timeline with Cues/Markers Ruler, Grouped Tracks & FilmCraft Audio Waveform */}
          <section id="authoring-timeline" className="lyrics-timeline-dock">
            <div className="lyrics-timeline-tracks">
              {/* Cues / Markers Ruler Lane (Synced FilmCraft + EffectCraft) */}
              <div className="lyrics-timeline-row markers-row">
                <span className="lane-label">Cues</span>
                <div
                  className="lane-track"
                  onClick={(e) => {
                    const r = e.currentTarget.getBoundingClientRect();
                    const pct = Math.max(
                      0,
                      Math.min(1, (e.clientX - r.left) / Math.max(1, r.width)),
                    );
                    setPlayheadSec(pct * totalSongDuration);
                  }}
                >
                  {timelineMarkers.map((mk, i) => {
                    const left = Math.max(
                      0,
                      Math.min(98.5, (mk.time_sec / totalSongDuration) * 100),
                    );
                    const width =
                      mk.duration_sec > 0
                        ? Math.max(
                            1.5,
                            Math.min(
                              100 - left,
                              (mk.duration_sec / totalSongDuration) * 100,
                            ),
                          )
                        : undefined;
                    const colorClass = (mk.color || "Iris").toLowerCase();
                    return (
                      <button
                        key={`mk-${mk.id ?? i}-${mk.time_sec}`}
                        type="button"
                        className={`timeline-marker-pin color-${colorClass} ${
                          width ? "is-range" : "is-cue"
                        }`}
                        style={{
                          left: `${left}%`,
                          width: width ? `${width}%` : undefined,
                        }}
                        title={`[${mk.source}] ${mk.name}: ${mk.comment} (${formatTime(mk.time_sec)})`}
                        onClick={(ev) => {
                          ev.stopPropagation();
                          setPlayheadSec(mk.time_sec);
                        }}
                      >
                        {width ? `◆ ${mk.name || mk.comment}` : "◆"}
                      </button>
                    );
                  })}
                  <div
                    className="timeline-playhead"
                    style={{
                      left: `${Math.min(100, (playheadSec / totalSongDuration) * 100)}%`,
                    }}
                  />
                </div>
              </div>

              {/* Scene / V1 Grouped Track Lane */}
              {sceneTrackClips.length > 0 && (
                <div className="lyrics-timeline-row">
                  <span className="lane-label">Scenes</span>
                  <div className="lane-track">
                    {sceneTrackClips.map((sc) => {
                      const left = (sc.start_sec / totalSongDuration) * 100;
                      const width = Math.max(
                        1.5,
                        ((sc.end_sec - sc.start_sec) / totalSongDuration) * 100,
                      );
                      return (
                        <button
                          key={`sc-${sc.clip}`}
                          type="button"
                          className="timeline-scene-pill"
                          style={{ left: `${left}%`, width: `${width}%` }}
                          title={`${sc.name} (${formatTime(sc.start_sec)} – ${formatTime(sc.end_sec)})`}
                          onClick={() => setPlayheadSec(sc.start_sec)}
                        >
                          {sc.name}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Waveform Lane */}
              <div className="lyrics-timeline-row waveform-row">
                <span className="lane-label">Audio</span>
                <div
                  className="lane-track"
                  onClick={(e) => {
                    const r = e.currentTarget.getBoundingClientRect();
                    const pct = Math.max(
                      0,
                      Math.min(1, (e.clientX - r.left) / Math.max(1, r.width)),
                    );
                    setPlayheadSec(pct * totalSongDuration);
                  }}
                >
                  <svg
                    viewBox="0 0 1000 36"
                    preserveAspectRatio="none"
                    className="lyrics-waveform-svg"
                  >
                    {(waveformPeaks.length > 0
                      ? waveformPeaks
                      : Array.from({ length: 120 }, (_, i) =>
                          0.2 + 0.6 * Math.abs(Math.sin(i * 0.23)),
                        )
                    ).map((pk, i, arr) => {
                      const x = (i / arr.length) * 1000;
                      const h = Math.max(3, Math.min(34, pk * 34));
                      return (
                        <rect
                          key={i}
                          x={x}
                          y={18 - h * 0.5}
                          width={Math.max(2, 800 / arr.length)}
                          height={h}
                          rx={1}
                          fill="rgba(72, 219, 198, 0.55)"
                        />
                      );
                    })}
                  </svg>
                  <div
                    className="timeline-playhead"
                    style={{
                      left: `${Math.min(100, (playheadSec / totalSongDuration) * 100)}%`,
                    }}
                  />
                </div>
              </div>

              {/* Lyric Lines Lane */}
              <div className="lyrics-timeline-row">
                <span className="lane-label">Lyrics</span>
                <div className="lane-track">
                  {allLineGroups.map((grp) => {
                    const key = `${grp.animId}::${grp.lineId}`;
                    const left = (grp.start / totalSongDuration) * 100;
                    const width = Math.max(
                      1.2,
                      ((grp.end - grp.start) / totalSongDuration) * 100,
                    );
                    return (
                      <button
                        key={key}
                        type="button"
                        className={`timeline-clip-pill ${activeKey === key ? "active" : ""}`}
                        style={{ left: `${left}%`, width: `${width}%` }}
                        title={`${grp.sectionName}: ${grp.text}`}
                        onClick={() => {
                          setSelectedLineKey(key);
                          setActiveItemIndex(0);
                          setPlayheadSec(grp.start);
                        }}
                      >
                        {grp.text}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Active Line Word Tokens Lane */}
              <div className="lyrics-timeline-row">
                <span className="lane-label">Words</span>
                <div className="lane-track">
                  {(activeLine?.words ?? []).map((w) => {
                    const lineStart = activeLine?.start ?? 0;
                    const lineDur = Math.max(
                      0.5,
                      (activeLine?.end ?? lineStart + 3) - lineStart,
                    );
                    const left = Math.max(
                      0,
                      Math.min(96, ((w.start - lineStart) / lineDur) * 100),
                    );
                    const width = Math.max(
                      4,
                      Math.min(100 - left, ((w.end - w.start) / lineDur) * 100),
                    );
                    return (
                      <span
                        key={w.word_id}
                        className="timeline-word-token"
                        style={{ left: `${left}%`, width: `${width}%` }}
                      >
                        {w.text}
                      </span>
                    );
                  })}
                </div>
              </div>
            </div>
          </section>
        </main>

        {/* RIGHT INSPECTOR RAIL: Unified .panel.motion-section + Dial/Slider/Toggle */}
        <LyricsInspector
          mode={mode}
          anim={activeAnim}
          lineId={activeLine?.lineId ?? null}
          lineWords={activeLine?.words ?? []}
          design={activeDesign}
          activeItemIndex={activeItemIndex}
          onSelectItemIndex={setActiveItemIndex}
          onUpdateDesign={updateActiveDesign}
          onSaveAndSync={saveAndSyncEcproj}
          moves={moves}
          letterings={letterings}
          strokes={strokes}
          choreographies={choreographies}
          easings={easings}
          textPresets={textPresets}
          drawMode={drawMode}
          onToggleDrawMode={() => setDrawMode((d) => !d)}
          brushColor={brushColor}
          onBrushColorChange={setBrushColor}
          brushWidth={brushWidth}
          onBrushWidthChange={setBrushWidth}
          onToast={onToast}
        />
      </div>
    </div>
  );
};
