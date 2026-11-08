//! Word style clipboard: copy visual styling between word items.
//!
//! Ported from apps/magic-cli/ui/native-word-controls.js (styleClipboardText,
//! parseStyleClipboard, applyStyleClipboard).
//!
//! A style clipboard copies visual styling only — it never carries `text`,
//! `word_ids` or `accent`, and applying it rebinds `accent` to the target's
//! word ids without touching `items[].text`.

import { useCallback } from "react";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

/**
 * Word style clipboard hooks using the Tauri clipboard-manager plugin.
 */
interface LegacyClipboard {
  readText?: () => Promise<string>;
  writeText?: (text: string) => Promise<void>;
}
const legacyClipboard = (): LegacyClipboard | undefined =>
  (window as unknown as { __TAURI__?: { clipboardManager?: LegacyClipboard } }).__TAURI__?.clipboardManager;
const clipboardRead = (): Promise<string> => legacyClipboard()?.readText?.() ?? readText();
const clipboardWrite = (text: string): Promise<void> => legacyClipboard()?.writeText?.(text) ?? writeText(text);

const STYLE_CLIPBOARD_MAX_BYTES = 128 * 1024;
const STYLE_FONTS = ["song", "serif", "sans"];
const STYLE_MOVES = ["quiet", "sleep", "settle", "fall", "pop", "rise", "east", "west", "loop"];
const STYLE_PROPERTIES = ["transform/rotation", "transform/opacity", "transform/scale", "transform/position"];

const STYLE_SIZE_DEFAULT = 58;
const STYLE_SIZE_MIN = 8;
const STYLE_SIZE_MAX = 600;
const STYLE_ROTATE_DEFAULT = 0;
const STYLE_ROTATE_MIN = -45;
const STYLE_ROTATE_MAX = 45;
const STYLE_WEIGHT_DEFAULT = 400;
const STYLE_WEIGHT_BOLD = 700;
const DEFAULT_COLOR = "#ffffff";

type NativeMotion = {
  property: string;
  from?: number;
  to?: number;
  speed?: number;
  duration?: number;
  delay?: number;
};

type WordStyle = {
  font: string;
  weight: number;
  italic: boolean;
  size: number;
  rotate: number;
  color: string;
  outline: string | null;
  motion: string;
  native_motion: NativeMotion | null;
  motion_keys: number[][] | null;
};

type StyleClipboardProps = {
  design: { items: unknown[]; draws?: unknown[] };
  itemIndex: number;
  onChange: (next: { items: unknown[]; draws?: unknown[] }) => void;
  onToast: (message: string) => void;
  /** Keep the legacy panel's id on the motion panel, not on this nested clipboard block. */
  id?: string;
  title?: string;
};

function validStyleColour(value: unknown): boolean {
  return typeof value === "string" && value.length <= 32 && !/[<>";]/.test(value);
}

function parseStyleClipboard(text: string): WordStyle {
  if (typeof text !== "string" || text.length > STYLE_CLIPBOARD_MAX_BYTES) {
    throw new Error("Clipboard style is too large");
  }
  let payload: { format?: unknown; version?: unknown; style?: unknown };
  try {
    payload = JSON.parse(text);
  } catch {
    throw new Error("Copy word styling first");
  }
  if (payload.format !== "magic.style" || payload.version !== 1) {
    throw new Error("Copy word styling first");
  }
  const source = payload.style as { [key: string]: unknown } | undefined;
  if (!source || typeof source !== "object") {
    throw new Error("Invalid style clipboard");
  }

  const font = STYLE_FONTS.includes(source.font as string) ? (source.font as string) : "song";
  const weight = source.weight === 700 ? STYLE_WEIGHT_BOLD : STYLE_WEIGHT_DEFAULT;
  const italic = Boolean(source.italic);
  const size = typeof source.size === "number" && source.size >= STYLE_SIZE_MIN && source.size <= STYLE_SIZE_MAX
    ? Math.round(source.size)
    : STYLE_SIZE_DEFAULT;
  const rotate = typeof source.rotate === "number" && source.rotate >= STYLE_ROTATE_MIN && source.rotate <= STYLE_ROTATE_MAX
    ? Math.round(source.rotate)
    : STYLE_ROTATE_DEFAULT;
  const color = validStyleColour(source.color) ? (source.color as string) : DEFAULT_COLOR;
  const outline = source.outline === null || source.outline === undefined ? null : validStyleColour(source.outline) ? (source.outline as string) : null;
  const motion = STYLE_MOVES.includes(source.motion as string) ? (source.motion as string) : "quiet";

  let native_motion: NativeMotion | null = null;
  const nm = source.native_motion;
  if (nm && typeof nm === "object" && STYLE_PROPERTIES.includes((nm as { property?: unknown }).property as string)) {
    native_motion = {
      property: (nm as { property?: string }).property || "",
      from: typeof (nm as { from?: unknown }).from === "number" ? (nm as { from?: number }).from : undefined,
      to: typeof (nm as { to?: unknown }).to === "number" ? (nm as { to?: number }).to : undefined,
      speed: typeof (nm as { speed?: unknown }).speed === "number" ? (nm as { speed?: number }).speed : undefined,
      duration: typeof (nm as { duration?: unknown }).duration === "number" ? (nm as { duration?: number }).duration : undefined,
      delay: typeof (nm as { delay?: unknown }).delay === "number" ? (nm as { delay?: number }).delay : undefined,
    };
  }

  let motion_keys: number[][] | null = null;
  const mk = source.motion_keys;
  if (Array.isArray(mk) && mk.length >= 2 && mk.length <= 24 && mk.every(row => Array.isArray(row) && row.length === 7 && row.every(v => typeof v === "number" && Number.isFinite(v)))) {
    motion_keys = mk.map(row => row as number[]);
  }

  return { font, weight, italic, size, rotate, color, outline, motion, native_motion, motion_keys };
}

function styleClipboardText(style: WordStyle): string {
  return JSON.stringify({ format: "magic.style", version: 1, style });
}

function applyStyleClipboard(item: { [key: string]: unknown }, style: WordStyle): { [key: string]: unknown } {
  const result = { ...item };
  result.font = style.font;
  result.weight = style.weight;
  result.italic = style.italic;
  result.size = style.size;
  result.rotate = style.rotate;
  result.color = style.color;
  result.outline = style.outline;
  result.motion = style.motion;

  const word_ids: string[] = Array.isArray(result.word_ids) ? result.word_ids.filter((id): id is string => typeof id === "string") : [];

  if (style.native_motion && style.motion_keys) {
    result.native_motion = { ...style.native_motion };
    result.motion_keys = style.motion_keys.map(row => [...row]);
    const accent = result.accent as string | undefined;
    const validAccent = typeof accent === "string" && word_ids.includes(accent);
    result.accent = validAccent ? accent : (word_ids[0] ?? null);
  } else {
    delete result.native_motion;
    delete result.motion_keys;
    const accent = result.accent as string | undefined;
    if (style.motion === "quiet") {
      result.accent = null;
    } else if (typeof accent === "string" && word_ids.includes(accent)) {
      result.accent = accent;
    } else {
      result.accent = word_ids.length > 0 ? word_ids[word_ids.length - 1] : null;
    }
  }
  return result;
}

export function StyleClipboard({ design, itemIndex, onChange, onToast, id = "native-word-controls", title = "Word style · clipboard" }: StyleClipboardProps) {
  const item = design.items?.[itemIndex] as { [key: string]: unknown } | undefined;

  const handleCopy = useCallback(async () => {
    try {
      if (!item) throw new Error("Select a word group first");
      await clipboardWrite(styleClipboardText(item as WordStyle));
      onToast("Word style copied");
    } catch (error) {
      onToast(`Clipboard: ${error}`);
    }
  }, [item, onToast]);

  const handlePaste = useCallback(async () => {
    try {
      const text = await clipboardRead();
      const style = parseStyleClipboard(text);
      const updated = { ...design };
      const items = [...(updated.items as unknown[])];
      if (itemIndex >= items.length) throw new Error("Select a word group to style");
      items[itemIndex] = applyStyleClipboard(items[itemIndex] as { [key: string]: unknown }, style);
      updated.items = items;
      onChange(updated);
      onToast("Word style pasted · Save to persist");
    } catch (error) {
      onToast(`Clipboard: ${error}`);
    }
  }, [design, itemIndex, onChange, onToast]);

  return (
    <section id={id} className="panel-body native-word-controls" aria-label={title}>
      <strong>{title}</strong>
      <div className="words-selection-actions">
        <button id="words-copy-style" className="ghost" onClick={handleCopy}>
          Copy style
        </button>
        <button id="words-paste-style" className="ghost" onClick={handlePaste}>
          Paste style
        </button>
      </div>
      <p id="native-word-motion-status" className="muted" role="status">
        Visual styling only — never touches artist words[].text or word_ids.
      </p>
    </section>
  );
}
