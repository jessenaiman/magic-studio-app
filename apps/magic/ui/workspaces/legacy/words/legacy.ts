/**
 * Legacy Lyrics globals the word-motion island shares while the page is still script-driven.
 *
 * The island is mounted by `nativeWordControls(anim)` from `lyrics-layout.js`, so it reads the
 * same `wordsPanel`, design drafts and engine bridge the legacy panel used. Keeping the bridge
 * thin is deliberate: the panel is converted to React here, the surrounding page is not.
 */
export interface MotionKeyRow extends Array<number> {}

export interface NativeMotion {
  property: string;
  from?: number | [number, number];
  to?: number | [number, number];
  speed?: number;
  duration?: number;
  delay?: number;
}

export interface WordItem {
  word_ids: string[];
  text?: string;
  accent?: string | null;
  native_motion?: NativeMotion | null;
  motion_keys?: MotionKeyRow[];
  native_motion_binding?: {
    comp?: { id?: number; name?: string };
    layer?: { id?: number; name?: string };
    word_ids?: string[];
    box?: boolean;
  } | null;
  [key: string]: unknown;
}

export interface WordDesign {
  items: WordItem[];
  draws?: unknown[];
}

export interface WordsWord {
  word_id: string;
  text: string;
  [key: string]: unknown;
}

export interface WordsAnimation {
  animation_id: string;
  source_revision?: number;
  design?: Record<string, WordDesign>;
  words?: WordsWord[];
  [key: string]: unknown;
}

export interface WordsDocument {
  animations?: WordsAnimation[];
  [key: string]: unknown;
}

export interface WordsPanel {
  line: string | null;
  picked: string[];
  item: number | null;
  selected: string | null;
  designs: Map<string, WordDesign>;
  read: { document?: WordsDocument } | null;
  [key: string]: unknown;
}

export interface MotionProperty {
  path: string;
  name: string;
  [key: string]: unknown;
}

export interface PropertiesReply {
  engine?: string;
  project?: string;
  comp?: { id?: number; name?: string; width?: number; height?: number; duration?: number };
  layer?: { id?: number; name?: string; in?: number; out?: number };
  binding?: {
    animation_id?: string;
    line_id?: string;
    comp?: { id?: number; name?: string };
    layer?: { id?: number; name?: string };
    word_ids?: string[];
    box?: boolean;
  } | null;
  properties?: MotionProperty[];
  [key: string]: unknown;
}

export interface MotionSetReply {
  ok?: boolean;
  conflict?: boolean;
  reason?: string;
  animation?: WordsAnimation;
  config?: NativeMotion;
  motion_keys?: MotionKeyRow[];
  binding?: WordItem["native_motion_binding"];
  [key: string]: unknown;
}

declare global {
  const wordsPanel: WordsPanel;
  const wordsDesign: (anim: WordsAnimation, lineId: string) => WordDesign;
  const wordsDesignEdit: (anim: WordsAnimation, lineId: string, change: (design: WordDesign) => void) => void;
  const wordsLineWords: (anim: WordsAnimation, lineId: string) => WordsWord[];
  const wordsStageController: {
    resolveActiveItemIndex: (design: WordDesign, picked: string[]) => number;
  };
  const wordsCommand: <T = unknown>(command: string, params?: Record<string, unknown>) => Promise<T>;
  const wordsRender: () => void;
  const toast: (message: string) => void;
}

export {};
