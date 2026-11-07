//! Drawing clipboard and patch controls for the Motion workspace.
//!
//! Ported from apps/magic-cli/ui/drawing-controls.js (patchDrawing,
//! drawingClipboardText, parseDrawingClipboard, existingDrawingControls).
//! All DOM ids are preserved so control-surface tests and --control automation
//! keep working. Clipboard validation is pure TypeScript; persistence goes
//! through the existing engine commands (words.read / words.save) rather than
//! inventing new Rust commands.

import { useCallback, useRef } from "react";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

// ---------- limits (mirror legacy JS guards) ----------

const DRAW_CLIPBOARD_MAX_BYTES = 512 * 1024;
const X_RANGE: [number, number] = [-2000, 6000];
const Y_RANGE: [number, number] = [-2000, 6000];
const W_RANGE: [number, number] = [1, 6000];
const H_RANGE: [number, number] = [1, 6000];
const STROKE_WIDTH_RANGE: [number, number] = [0.5, 60];
const DRAW_START_MIN = 0;
const DRAW_END_MAX = 600;

const SVG_PATH_WHITELIST = /^[MmLlHhVvCcSsQqTtAaZz0-9eE.,\s+\-]+$/;
const HEX_COLOR = /^#[0-9a-f]{6}$/i;

type DrawingPath = {
  d: string;
  stroke: string;
  width: number;
  from: number;
  to: number;
};

type Drawing = {
  name: string;
  view_box: string;
  x: number;
  y: number;
  w: number;
  h: number;
  word_ids: string[];
  paths: DrawingPath[];
};

type Design = {
  items: Array<{ word_ids?: string[]; [key: string]: unknown }>;
  draws: Drawing[];
};

type PatchChanges = {
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  strokeWidth?: number;
  name?: string;
  color?: string;
  word_ids?: string[];
  start?: number;
  end?: number;
};

// ---------- pure domain logic (ported from drawing-controls.js) ----------

function isFiniteIn(n: unknown, lo: number, hi: number): n is number {
  return typeof n === "number" && Number.isFinite(n) && n >= lo && n <= hi;
}

function parseDrawingClipboard(text: string): Drawing {
  if (typeof text !== "string" || text.length > DRAW_CLIPBOARD_MAX_BYTES) {
    throw new Error("Clipboard drawing is too large");
  }
  let payload: { format?: unknown; version?: unknown; drawing?: unknown };
  try {
    payload = JSON.parse(text);
  } catch {
    throw new Error("Copy a Motion drawing first");
  }
  if (payload.format !== "magic.drawing" || payload.version !== 1) {
    throw new Error("Copy a Motion drawing first");
  }
  const source = payload.drawing as { [key: string]: unknown } | undefined;
  if (!source || typeof source !== "object") {
    throw new Error("Invalid drawing clipboard");
  }

  const vb = String(source.view_box || "").trim().split(/\s+/).map(Number);
  if (vb.length !== 4 || !vb.every(Number.isFinite) || vb[2] <= 0 || vb[3] <= 0) {
    throw new Error("Invalid drawing bounds");
  }

  const rawPaths = Array.isArray(source.paths) ? source.paths : [];
  if (!rawPaths.length || rawPaths.length > 256) {
    throw new Error("Invalid drawing paths");
  }

  const paths: DrawingPath[] = [];
  for (const raw of rawPaths) {
    if (typeof raw !== "object" || raw === null) {
      throw new Error("Invalid drawing paths");
    }
    const p = raw as DrawingPath;
    if (typeof p.d !== "string" || !p.d.length || p.d.length > 100000 || !SVG_PATH_WHITELIST.test(p.d)) {
      throw new Error("Invalid drawing geometry");
    }
    if (!HEX_COLOR.test(p.stroke) || !isFiniteIn(p.width, STROKE_WIDTH_RANGE[0], STROKE_WIDTH_RANGE[1])) {
      throw new Error("Invalid drawing stroke");
    }
    if (typeof p.from !== "number" || typeof p.to !== "number" || !Number.isFinite(p.from) || !Number.isFinite(p.to) || p.from < 0 || p.to <= p.from || p.to > 600) {
      throw new Error("Invalid drawing timing");
    }
    paths.push({
      d: p.d,
      stroke: p.stroke,
      width: p.width,
      from: p.from,
      to: p.to,
    });
  }

  const name = String(source.name || "Drawing").slice(0, 200);
  const drawing: Drawing = {
    name,
    view_box: vb.join(" "),
    x: Number(source.x),
    y: Number(source.y),
    w: Number(source.w),
    h: Number(source.h),
    word_ids: [] as string[],
    paths,
  };
  return patchDrawing({ draws: [drawing] } as unknown as Design, 0, {
    x: source.x as number,
    y: source.y as number,
    w: source.w as number,
    h: source.h as number,
  }).draws[0];
}

function patchDrawing(design: Design, index: number, changes: PatchChanges): Design {
  const result = JSON.parse(JSON.stringify(design));
  const drawing = result.draws?.[index];
  if (!drawing) throw new Error("Select an existing drawing");

  const ranges: Record<string, [number, number]> = {
    x: X_RANGE, y: Y_RANGE, w: W_RANGE, h: H_RANGE, strokeWidth: STROKE_WIDTH_RANGE,
  };
  const changesRecord = changes as Record<string, unknown>;
  for (const [key, [min, max]] of Object.entries(ranges)) {
    if (!(key in changesRecord)) continue;
    const v = changesRecord[key];
    if (!isFiniteIn(v, min, max)) {
      throw new Error(`${key} must be between ${min} and ${max}`);
    }
    if (key === "strokeWidth") {
      for (const path of drawing.paths) path.width = v as number;
    } else {
      (drawing as Record<string, unknown>)[key] = v;
    }
  }
  if ("name" in changesRecord) drawing.name = String(changesRecord.name).trim() || "Drawing";
  if ("color" in changesRecord) {
    if (!HEX_COLOR.test(String(changesRecord.color))) throw new Error("Choose a colour");
    for (const path of drawing.paths) path.stroke = String(changesRecord.color);
  }
  if ("word_ids" in changesRecord) drawing.word_ids = [...(changesRecord.word_ids as string[])];
  if ("start" in changesRecord || "end" in changesRecord) {
    const from = Math.min(...drawing.paths.map((p: DrawingPath) => p.from));
    const to = Math.max(...drawing.paths.map((p: DrawingPath) => p.to));
    const start = ("start" in changesRecord ? changesRecord.start : from) as number;
    const end = ("end" in changesRecord ? changesRecord.end : to) as number;
    if (typeof start !== "number" || typeof end !== "number" || !Number.isFinite(start) || !Number.isFinite(end) || start < 0 || end <= start || end > 600) {
      throw new Error("Drawing end must follow its start (0–600 seconds)");
    }
    for (const path of drawing.paths) {
      path.from = start + (path.from - from) / Math.max(0.001, to - from) * (end - start);
      path.to = start + (path.to - from) / Math.max(0.001, to - from) * (end - start);
    }
  }
  return result;
}

function drawingClipboardText(drawing: Drawing): string {
  return JSON.stringify({ format: "magic.drawing", version: 1, drawing });
}

// ---------- React component ----------

interface DrawingClipboardProps {
  design: Design;
  drawingIndex: number;
  onChange: (next: Design) => void;
  onToast: (message: string) => void;
}

export function DrawingClipboard({ design, drawingIndex, onChange, onToast }: DrawingClipboardProps) {
  const drawing = design.draws?.[drawingIndex];
  const statusRef = useRef<HTMLParagraphElement>(null);

  if (!drawing) {
    return (
      <section id="existing-drawing-controls" className="panel-body existing-drawing-controls" aria-label="Existing drawing settings">
        <strong>Existing drawing</strong>
        <p className="muted">No drawing in this moment. Choose a shape or draw on the stage.</p>
      </section>
    );
  }

  const start = Math.min(...drawing.paths.map(p => p.from));
  const end = Math.max(...drawing.paths.map(p => p.to));

  const applyPatch = useCallback(
    (changes: PatchChanges) => {
      try {
        const next = patchDrawing(design, drawingIndex, changes);
        onChange(next);
        statusRef.current!.textContent = "";
      } catch (error) {
        statusRef.current!.textContent = String(error);
      }
    },
    [design, drawingIndex, onChange],
  );

  const handleCopy = async () => {
    try {
      await writeText(drawingClipboardText(drawing));
      onToast("Drawing copied");
    } catch (error) {
      onToast(`Clipboard: ${error}`);
    }
  };

  const handlePaste = async () => {
    try {
      const text = await readText();
      const parsed = parseDrawingClipboard(text);
      const next = patchDrawing({ draws: [parsed] } as unknown as Design, 0, {
        x: parsed.x, y: parsed.y, w: parsed.w, h: parsed.h,
      });
      const updated = JSON.parse(JSON.stringify(design));
      updated.draws ||= [];
      updated.draws.push(next.draws[0]);
      onChange(updated);
      onToast("Drawing pasted · Save to persist");
    } catch (error) {
      onToast(`Clipboard: ${error}`);
    }
  };

  const handleDelete = () => {
    const updated = JSON.parse(JSON.stringify(design));
    const index = Math.min(updated.draws.length - 1, drawingIndex);
    updated.draws.splice(index, 1);
    onChange(updated);
  };

  return (
    <section id="existing-drawing-controls" className="panel-body existing-drawing-controls" aria-label="Existing drawing settings">
      <strong>Existing drawing</strong>
      <div className="words-selection-actions">
        <button id="motion-copy-drawing" className="ghost" disabled={!design.draws?.length} onClick={handleCopy}>
          Copy drawing
        </button>
        <button id="motion-paste-drawing" className="ghost" onClick={handlePaste}>
          Paste drawing
        </button>
        <button id="motion-delete-drawing" className="ghost" disabled={!design.draws?.length} onClick={handleDelete}>
          Remove drawing
        </button>
      </div>
      {!design.draws?.length && (
        <p className="muted">No drawing in this moment. Choose a shape or draw on the stage.</p>
      )}
      {design.draws?.length && (
        <>
          <select id="motion-existing-drawing" aria-label="Select existing drawing" defaultValue={String(drawingIndex)}>
            {design.draws.map((item, i) => (
              <option key={i} value={String(i)}>{item.name || `Drawing ${i + 1}`}</option>
            ))}
          </select>
          <p id="motion-drawing-edit-status" className="muted" role="status" ref={statusRef} />
          <label>
            Name <input id="motion-drawing-name" className="field" aria-label="Drawing name" defaultValue={drawing.name || "Drawing"} onChange={e => applyPatch({ name: e.target.value })} />
          </label>
          <div className="drawing-coordinate-grid">
            <label>
              X <input id="motion-drawing-x" className="field" type="number" min={X_RANGE[0]} max={X_RANGE[1]} step={1} aria-label="X" defaultValue={String(drawing.x)} onChange={e => applyPatch({ x: Number(e.target.value) })} />
            </label>
            <label>
              Y <input id="motion-drawing-y" className="field" type="number" min={Y_RANGE[0]} max={Y_RANGE[1]} step={1} aria-label="Y" defaultValue={String(drawing.y)} onChange={e => applyPatch({ y: Number(e.target.value) })} />
            </label>
            <label>
              Width <input id="motion-drawing-w" className="field" type="number" min={W_RANGE[0]} max={W_RANGE[1]} step={1} aria-label="Width" defaultValue={String(drawing.w)} onChange={e => applyPatch({ w: Number(e.target.value) })} />
            </label>
            <label>
              Height <input id="motion-drawing-h" className="field" type="number" min={H_RANGE[0]} max={H_RANGE[1]} step={1} aria-label="Height" defaultValue={String(drawing.h)} onChange={e => applyPatch({ h: Number(e.target.value) })} />
            </label>
            <label>
              Draw start · seconds in line <input id="motion-drawing-start" className="field" type="number" min={DRAW_START_MIN} max={DRAW_END_MAX} step={0.01} aria-label="Draw start · seconds in line" defaultValue={String(start)} onChange={e => applyPatch({ start: Number(e.target.value) })} />
            </label>
            <label>
              Draw end · seconds in line <input id="motion-drawing-end" className="field" type="number" min={DRAW_START_MIN} max={DRAW_END_MAX} step={0.01} aria-label="Draw end · seconds in line" defaultValue={String(end)} onChange={e => applyPatch({ end: Number(e.target.value) })} />
            </label>
            <label>
              Stroke width <input id="motion-drawing-stroke-width" className="field" type="number" min={STROKE_WIDTH_RANGE[0]} max={STROKE_WIDTH_RANGE[1]} step={0.5} aria-label="Stroke width" defaultValue={String(drawing.paths[0]?.width || 4)} onChange={e => applyPatch({ strokeWidth: Number(e.target.value) })} />
            </label>
            <label>
              Stroke colour <input id="motion-drawing-color" type="color" aria-label="Existing drawing stroke colour" defaultValue={HEX_COLOR.test(drawing.paths[0]?.stroke || "") ? drawing.paths[0].stroke.toLowerCase() : "#173f49"} onChange={e => applyPatch({ color: e.target.value })} />
            </label>
          </div>
          <p className="muted">Changes apply to this drawing. Multi-stroke timing keeps its relative rhythm.</p>
        </>
      )}
    </section>
  );
}
