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

function existingDrawingControls(anim) {
  const lineId = wordsPanel.line, design = wordsDesign(anim, lineId), drawings = design.draws || [];
  const panel = el("section", { id: "existing-drawing-controls", class: "panel-body existing-drawing-controls", "aria-label": "Existing drawing settings" });
  panel.append(el("strong", {}, "Existing drawing"));
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
  color.value = color.defaultValue = /^#[0-9a-f]{6}$/i.test(drawing.paths[0]?.stroke || "") ? drawing.paths[0].stroke : "#173f49";
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
    field("start", "Draw start · seconds in line", start, 0, 600, .05), field("end", "Draw end · seconds in line", end, 0, 600, .05),
    field("stroke-width", "Stroke width", drawing.paths[0]?.width || 4, .5, 60, .5, "strokeWidth"), el("label", {}, "Stroke colour", color)),
    el("p", { class: "muted" }, "Changes apply to this drawing. Multi-stroke timing keeps its relative rhythm."), status);
  return panel;
}
