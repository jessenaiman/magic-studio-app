export interface WordToken {
  word_id: string;
  line_id: string;
  sung_id?: string;
  text: string;
  source_text?: string;
  display_text?: string;
  start: number;
  end: number;
  start_tick?: number;
  end_tick?: number;
  emphasis?: boolean;
  action?: string;
  entrance?: string;
  exit?: string;
}

export interface MotionProperty {
  path: string;
  name?: string;
  keys?: unknown[];
}

export interface NativeMotion {
  property?: string;
  delay?: number;
  duration?: number;
  from?: unknown;
  to?: unknown;
  speed?: number;
}

export interface WordItem {
  word_ids: string[];
  x: number;
  y: number;
  w?: number;
  h?: number;
  lettering?: string;
  size?: number;
  color?: string;
  outline?: string | null;
  stroke_width?: number;
  tracking?: number;
  rotate?: number;
  skew?: number;
  skew_axis?: number;
  scale_x?: number;
  scale_y?: number;
  per_char_3d?: boolean;
  warp_path?: "none" | "arc_up" | "arc_down" | "wave" | "circle";
  move?: string;
  dx?: number;
  dy?: number;
  text_preset?: string;
  accent?: string | null;
  motion_keys?: [number, unknown][];
  native_motion?: NativeMotion | null;
  native_motion_binding?: {
    comp?: string;
    layer?: { id?: number; index?: number; name?: string };
    word_index?: number;
  } | null;
}

export interface DrawnPath {
  d: string;
  color?: string;
  width?: number;
  from?: number;
  to?: number;
}

export interface LineDraw {
  name?: string;
  view_box?: string;
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  paths: DrawnPath[];
}

export interface LineDesign {
  choreography?: string;
  items: WordItem[];
  draws: LineDraw[];
}

export interface WordsAnimation {
  animation_id: string;
  in_video?: boolean;
  source_revision?: number;
  song_anchor?: number;
  authored_duration?: number;
  background_asset?: string | null;
  scope?: {
    section?: string;
    index?: number;
  };
  style?: {
    font?: string;
    size?: number;
    color?: string;
    emphasis_color?: string;
    outline_color?: string;
    background?: string;
  };
  words: WordToken[];
  design?: Record<string, LineDesign>;
  effects?: Record<string, unknown>;
  kinetic?: {
    preset?: string;
    style?: Record<string, unknown>;
    draws?: Record<string, unknown>[];
    overlay?: string;
  } | null;
}

export interface BackdropImage {
  id: string;
  asset_id?: string;
  name: string;
  path: string;
}

export interface SceneClip {
  id: string;
  label: string;
  sectionIndex: number;
  start: number;
  path: string;
}

export interface StrokePreset {
  id: string;
  name: string;
  does?: string;
  view_box: string;
  w: number;
  h: number;
  draw: number;
  d: string;
}

export interface MovePreset {
  id: string;
  name: string;
  does: string;
}

export interface LetteringPreset {
  id: string;
  name: string;
  does: string;
  font: string;
  weight: number;
  italic: boolean;
}

export interface TextPresetItem {
  id?: string;
  match_name?: string;
  name?: string;
  category?: string;
}

export interface EcprojOutlineInfo {
  path?: string;
  saved_by?: string | null;
  schema?: number;
  comp_count?: number;
  layer_count?: number;
}

export interface EasingPreset {
  id: string;
  name: string;
  does: string;
  bezier?: [number, number, number, number];
  samples?: number[];
}

export interface WordsReadReply {
  ok: boolean;
  saved: boolean;
  path: string;
  ecproj?: EcprojOutlineInfo | null;
  images: BackdropImage[];
  scene_clips: SceneClip[];
  graphics: Record<string, unknown>[];
  document: {
    animations: WordsAnimation[];
  };
  tools?: {
    moves?: MovePreset[];
    letterings?: LetteringPreset[];
    strokes?: StrokePreset[];
    choreographies?: { id: string; name: string; does: string }[];
    easings?: EasingPreset[];
    symbol_loop_modes?: string[];
    effectcraft?: {
      text_presets?: (string | TextPresetItem)[];
      animator_properties?: string[];
      effects?: TextPresetItem[];
    };
  };
}

export interface LyricLineGroup {
  lineId: string;
  animId: string;
  sectionName: string;
  words: WordToken[];
  start: number;
  end: number;
  text: string;
}

export const STAGE_W = 1920;
export const STAGE_H = 1080;

export function isDarkHex(hex?: string | null): boolean {
  if (!hex) return false;
  const clean = hex.trim().replace(/^#/, "");
  if (clean.length !== 6) return false;
  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  if (Number.isNaN(r) || Number.isNaN(g) || Number.isNaN(b)) return false;
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0 < 0.34;
}

export function effectiveOutlineColor(item: WordItem): string | null {
  if (item.outline && item.outline !== "none") return item.outline;
  if (isDarkHex(item.color)) return "#FFF8E9";
  return null;
}

export function fontStackForLettering(lettering?: string): {
  fontFamily: string;
  fontWeight: number;
  fontStyle: "normal" | "italic";
} {
  switch (lettering) {
    case "serif-italic":
      return {
        fontFamily: "'Fraunces', 'Playfair Display', Georgia, serif",
        fontWeight: 600,
        fontStyle: "italic",
      };
    case "serif-bold":
      return {
        fontFamily: "'Fraunces', 'Playfair Display', Georgia, serif",
        fontWeight: 800,
        fontStyle: "normal",
      };
    case "song-bold":
      return {
        fontFamily: "'Plus Jakarta Sans', 'Inter', system-ui, sans-serif",
        fontWeight: 800,
        fontStyle: "normal",
      };
    case "sans":
      return {
        fontFamily: "'Plus Jakarta Sans', 'Inter', system-ui, sans-serif",
        fontWeight: 500,
        fontStyle: "normal",
      };
    default:
      return {
        fontFamily: "'Plus Jakarta Sans', 'Inter', system-ui, sans-serif",
        fontWeight: 700,
        fontStyle: "normal",
      };
  }
}

export function buildDefaultLineDesign(
  words: WordToken[],
  choreography = "cascade",
): LineDesign {
  if (!words.length) return { choreography, items: [], draws: [] };
  const items: WordItem[] = [];
  if (words.length <= 3) {
    items.push({
      word_ids: words.map((w) => w.word_id),
      x: 420,
      y: 360,
      w: 1080,
      h: 180,
      lettering: "serif-italic",
      size: 108,
      color: "#E05A36",
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
      dy: -36,
    });
    return { choreography, items, draws: [] };
  }

  // Group into 2 or 3 expressive phrase blocks
  const mid = Math.max(1, Math.floor(words.length / 2));
  const groups = [words.slice(0, mid), words.slice(mid)];
  groups.forEach((grp, idx) => {
    const isHero = idx === 0;
    const x =
      choreography === "stack"
        ? 480
        : choreography === "plain"
          ? 260 + idx * 720
          : 280 + idx * 540;
    const y =
      choreography === "plain"
        ? 440
        : choreography === "poster"
          ? 240 + idx * 310
          : 260 + idx * 260;
    const rot =
      choreography === "scatter"
        ? idx % 2 === 0
          ? -8
          : 7
        : choreography === "poster" && isHero
          ? -4
          : 0;
    items.push({
      word_ids: grp.map((w) => w.word_id),
      x,
      y,
      w: isHero ? 820 : 760,
      h: isHero ? 170 : 140,
      lettering: isHero ? "serif-italic" : "song-bold",
      size: isHero ? 104 : 76,
      color: isHero ? "#D95B38" : "#173F49",
      outline: "#FFF8E9",
      stroke_width: isHero ? 6 : 4,
      tracking: 0,
      rotate: rot,
      skew: 0,
      skew_axis: 0,
      scale_x: 100,
      scale_y: 100,
      warp_path: "none",
      move: isHero ? "pop" : "rise",
      dx: 0,
      dy: isHero ? -32 : 36,
    });
  });
  return { choreography, items, draws: [] };
}

export function warpSvgPathD(
  warp: WordItem["warp_path"],
  w: number,
  h: number,
): string | null {
  const cy = h * 0.64;
  switch (warp) {
    case "arc_up":
      return `M 16 ${cy + 28} Q ${w * 0.5} ${cy - 44} ${w - 16} ${cy + 28}`;
    case "arc_down":
      return `M 16 ${cy - 24} Q ${w * 0.5} ${cy + 44} ${w - 16} ${cy - 24}`;
    case "wave":
      return `M 12 ${cy} C ${w * 0.28} ${cy - 46}, ${w * 0.72} ${cy + 46}, ${w - 12} ${cy}`;
    case "circle": {
      const rx = Math.max(48, w * 0.42);
      const ry = Math.max(28, h * 0.36);
      const cx = w * 0.5;
      return `M ${cx - rx} ${cy} A ${rx} ${ry} 0 1 1 ${cx + rx} ${cy}`;
    }
    default:
      return null;
  }
}
