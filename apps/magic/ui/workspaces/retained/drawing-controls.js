// Editing a derived drawing never rewrites the source words or their order.
function patchDrawing(design, index, changes) {
  const result = structuredClone(design), drawing = result.draws?.[index];
  if (!drawing) throw new Error("Select an existing drawing");
  const ranges = { x: [-2000, 6000], y: [-2000, 6000], w: [1, 6000], h: [1, 6000], strokeWidth: [.5, 60] };
  for (const [key, [min, max]] of Object.entries(ranges)) {
    if (!(key in changes)) continue;
    if (!Number.isFinite(changes[key]) || changes[key] < min || changes[key] > max) throw new Error(`${key} must be between ${min} and ${max}`);
    if (key === "strokeWidth") for (const path of drawing.paths) path.width = changes[key];
    else drawing[key] = changes[key];
  }
  if ("name" in changes) drawing.name = String(changes.name).trim() || "Drawing";
  if ("color" in changes) {
    if (!/^#[0-9a-f]{6}$/i.test(changes.color)) throw new Error("Choose a colour");
    for (const path of drawing.paths) path.stroke = changes.color;
  }
  if ("word_ids" in changes) drawing.word_ids = [...changes.word_ids];
  if ("start" in changes || "end" in changes) {
    const from = Math.min(...drawing.paths.map(path => path.from)), to = Math.max(...drawing.paths.map(path => path.to));
    const start = changes.start ?? from, end = changes.end ?? to;
    if (![start, end].every(Number.isFinite) || start < 0 || end <= start || end > 600) throw new Error("Drawing end must follow its start (0–600 seconds)");
    for (const path of drawing.paths) {
      path.from = start + (path.from - from) / Math.max(.001, to - from) * (end - start);
      path.to = start + (path.to - from) / Math.max(.001, to - from) * (end - start);
    }
  }
  return result;
}

function drawingClipboardText(drawing) {
  return JSON.stringify({format:"magic.drawing",version:1,drawing});
}

function parseDrawingClipboard(text) {
  if (typeof text !== "string" || text.length > 512 * 1024) throw new Error("Clipboard drawing is too large");
  const payload = JSON.parse(text);
  if (payload.format !== "magic.drawing" || payload.version !== 1) throw new Error("Copy a Motion drawing first");
  const source = payload.drawing;
  const box = String(source?.view_box || "").trim().split(/\s+/).map(Number);
  if (box.length !== 4 || !box.every(Number.isFinite) || box[2] <= 0 || box[3] <= 0) throw new Error("Invalid drawing bounds");
  if (!Array.isArray(source.paths) || !source.paths.length || source.paths.length > 256) throw new Error("Invalid drawing paths");
  const paths = source.paths.map(path => {
    if (typeof path.d !== "string" || !path.d.length || path.d.length > 100000 || !/^[MmLlHhVvCcSsQqTtAaZz0-9eE.,\s+\-]+$/.test(path.d)) throw new Error("Invalid drawing geometry");
    if (!/^#[0-9a-f]{6}$/i.test(path.stroke) || !Number.isFinite(path.width) || path.width < .5 || path.width > 60) throw new Error("Invalid drawing stroke");
    if (![path.from,path.to].every(Number.isFinite) || path.from < 0 || path.to <= path.from || path.to > 600) throw new Error("Invalid drawing timing");
    return {d:path.d,stroke:path.stroke,width:path.width,from:path.from,to:path.to};
  });
  const drawing = {name:String(source.name || "Drawing").slice(0,200),view_box:box.join(" "),paths};
  return patchDrawing({draws:[drawing]},0,{x:source.x,y:source.y,w:source.w,h:source.h}).draws[0];
}

// Style clipboard copies visual styling only; it never carries or mutates artist words or word_ids.
const STYLE_FONTS = ["song", "serif", "sans"];
const STYLE_MOVES = ["quiet", "sleep", "settle", "fall", "pop", "rise", "east", "west", "loop"];
const STYLE_PROPERTIES = ["transform/rotation", "transform/opacity", "transform/scale", "transform/position"];

function styleClipboardText(item) {
  const style = parseStyleClipboard(JSON.stringify({format:"magic.style",version:1,style:item}));
  return JSON.stringify({format:"magic.style",version:1,style});
}

function parseStyleClipboard(text) {
  if (typeof text !== "string" || text.length > 128 * 1024) throw new Error("Clipboard style is too large");
  const payload = JSON.parse(text);
  if (payload.format !== "magic.style" || payload.version !== 1) throw new Error("Copy word styling first");
  const source = payload.style;
  if (!source || typeof source !== "object") throw new Error("Invalid style clipboard");
  const font = STYLE_FONTS.includes(source.font) ? source.font : "song";
  const weight = source.weight === 700 ? 700 : 400;
  const italic = Boolean(source.italic);
  const size = Number.isFinite(source.size) && source.size >= 8 && source.size <= 600 ? Math.round(source.size) : 58;
  const rotate = Number.isFinite(source.rotate) && source.rotate >= -45 && source.rotate <= 45 ? Math.round(source.rotate) : 0;
  const validColor = value => typeof value === "string" && value.length <= 32 && !/[<>";]/.test(value);
  if (source.color !== undefined && !validColor(source.color)) throw new Error("Invalid style colour");
  const color = validColor(source.color) ? source.color : "#ffffff";
  if (source.outline !== undefined && source.outline !== null && !validColor(source.outline)) throw new Error("Invalid style outline colour");
  const outline = source.outline === null || source.outline === undefined ? null : source.outline;
  const motion = STYLE_MOVES.includes(source.motion) ? source.motion : "quiet";
  const style = {font,weight,italic,size,rotate,color,outline,motion};
  if (source.native_motion && typeof source.native_motion === "object" && STYLE_PROPERTIES.includes(source.native_motion.property)) {
    style.native_motion = structuredClone(source.native_motion);
  }
  if (Array.isArray(source.motion_keys) && source.motion_keys.length >= 2 && source.motion_keys.length <= 24 && source.motion_keys.every(row => Array.isArray(row) && row.length === 7 && row.every(Number.isFinite))) {
    style.motion_keys = structuredClone(source.motion_keys);
  }
  return style;
}

function applyStyleClipboard(design, index, style) {
  const result = structuredClone(design), target = result.items?.[index];
  if (!target) throw new Error("Select a word group to style");
  const validated = parseStyleClipboard(JSON.stringify({format:"magic.style",version:1,style}));
  target.font = validated.font;
  target.weight = validated.weight;
  target.italic = validated.italic;
  target.size = validated.size;
  target.rotate = validated.rotate;
  target.color = validated.color;
  target.outline = validated.outline;
  target.motion = validated.motion;
  if (validated.native_motion && validated.motion_keys) {
    target.native_motion = structuredClone(validated.native_motion);
    target.motion_keys = structuredClone(validated.motion_keys);
    if (!target.accent || !target.word_ids?.includes(target.accent)) target.accent = target.word_ids?.[0] || null;
  } else {
    delete target.native_motion;
    delete target.motion_keys;
    target.accent = validated.motion === "quiet" ? null : (target.word_ids?.includes(target.accent) ? target.accent : target.word_ids?.at(-1) || null);
  }
  return result;
}

function existingDrawingControls(anim) {
  const lineId = wordsPanel.line, design = wordsDesign(anim, lineId), drawings = design.draws || [];
  const panel = el("section", { id: "existing-drawing-controls", class: "panel-body existing-drawing-controls", "aria-label": "Existing drawing settings" });
  panel.append(el("strong", {}, "Existing drawing"));
  const project = state.project;
  panel.append(el("div", {class:"words-selection-actions"},
    el("button", {id:"motion-copy-drawing",class:"ghost",disabled:!drawings.length,onclick:async () => {
      try {
        const selected = drawings[Math.min(drawings.length - 1, wordsPanel.selectedDrawingIndex || 0)];
        await window.__TAURI__.clipboardManager.writeText(drawingClipboardText(selected));
        toast("Drawing copied");
      } catch (error) { toast(`Clipboard: ${error}`); }
    }}, "Copy drawing"),
    el("button", {id:"motion-paste-drawing",class:"ghost",onclick:async () => {
      try {
        const drawing = parseDrawingClipboard(await window.__TAURI__.clipboardManager.readText());
        if (state.project !== project || wordsPanel.selected !== anim.animation_id || wordsPanel.line !== lineId) throw new Error("Selection changed; drawing was not pasted");
        wordsDesignEdit(anim,lineId,next => { next.draws ||= []; next.draws.push(drawing); wordsPanel.selectedDrawingIndex = next.draws.length - 1; });
        toast("Drawing pasted · Save to persist");
      } catch (error) { toast(`Clipboard: ${error}`); }
    }}, "Paste drawing"),
    el("button", {id:"motion-delete-drawing",class:"ghost",disabled:!drawings.length,onclick:() => {
      wordsDesignEdit(anim,lineId,next => {
        const index = Math.min(next.draws.length - 1, wordsPanel.selectedDrawingIndex || 0);
        next.draws.splice(index,1);
        wordsPanel.selectedDrawingIndex = Math.max(0,index - 1);
      });
    }}, "Remove drawing")));
  if (!drawings.length) { panel.append(el("p", { class: "muted" }, "No drawing in this moment. Choose a shape or draw on the stage.")); return panel; }
  const index = Math.max(0, Math.min(drawings.length - 1, wordsPanel.selectedDrawingIndex || 0));
  wordsPanel.selectedDrawingIndex = index;
  const drawing = drawings[index];
  const select = el("select", { id: "motion-existing-drawing", "aria-label": "Select existing drawing", onchange: event => {
    wordsPanel.selectedDrawingIndex = Number(event.target.value); wordsRender();
  } }, ...drawings.map((item, i) => el("option", { value: String(i) }, item.name || `Drawing ${i + 1}`)));
  select.value = String(index); panel.append(select);
  const status = el("p", { id: "motion-drawing-edit-status", class: "muted", role: "status" });
  const apply = changes => {
    try {
      const next = patchDrawing(wordsDesign(anim, lineId), index, changes);
      wordsDesignEdit(anim, lineId, target => { target.draws = next.draws; });
    } catch (error) { status.textContent = String(error); }
  };
  const field = (id, label, value, min, max, step, key = id) => {
    const input = el("input", { id: `motion-drawing-${id}`, class: "field", type: "number", min, max, step, "aria-label": label,
      onchange: event => apply({ [key]: Number(event.target.value) }) });
    input.value = input.defaultValue = String(value);
    return el("label", {}, label, input);
  };
  const name = el("input", { id: "motion-drawing-name", class: "field", "aria-label": "Drawing name", onchange: event => apply({ name: event.target.value }) });
  name.value = name.defaultValue = drawing.name || "Drawing";
  const start = Math.min(...drawing.paths.map(path => path.from)), end = Math.max(...drawing.paths.map(path => path.to));
  const color = el("input", { id: "motion-drawing-color", type: "color", "aria-label": "Existing drawing stroke colour", onchange: event => apply({ color: event.target.value }) });
  color.value = color.defaultValue = /^#[0-9a-f]{6}$/i.test(drawing.paths[0]?.stroke || "") ? drawing.paths[0].stroke.toLowerCase() : "#173f49";
  const words = wordsLineWords(anim, lineId);
  const linkedWord = el("select", { id: "motion-drawing-linked-word", "aria-label": "Drawing's source word", onchange: event => apply({ word_ids: event.target.value ? [event.target.value] : [] }) },
    el("option", { value: "" }, "No linked word"), ...words.map(word => el("option", { value: word.word_id }, `${word.text} · ${wordsClock(wordsAt(word))}`)));
  linkedWord.value = drawing.word_ids?.[0] || "";
  const align = el("button", { id: "motion-drawing-align-word", class: "ghost", disabled: !linkedWord.value, onclick: () => {
    const word = words.find(word => word.word_id === linkedWord.value);
    if (!word) return;
    const from = Math.max(0, wordsAt(word) - wordsLineSpan(anim, lineId).start);
    apply({ start: from, end: from + end - start });
  } }, "Align draw start to word");
  panel.append(el("label", {}, "Link to artist word", linkedWord), align, el("label", {}, "Name", name), el("div", { class: "drawing-coordinate-grid" },
    field("x", "X", drawing.x, -2000, 6000, 1), field("y", "Y", drawing.y, -2000, 6000, 1),
    field("w", "Width", drawing.w, 1, 6000, 1), field("h", "Height", drawing.h, 1, 6000, 1),
    field("start", "Draw start · seconds in line", start, 0, 600, .01), field("end", "Draw end · seconds in line", end, 0, 600, .01),
    field("stroke-width", "Stroke width", drawing.paths[0]?.width || 4, .5, 60, .5, "strokeWidth"), el("label", {}, "Stroke colour", color)),
    el("p", { class: "muted" }, "Changes apply to this drawing. Multi-stroke timing keeps its relative rhythm."), status);
  return panel;
}
