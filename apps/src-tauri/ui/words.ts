const wordsCard = (on: boolean, title: string, does: string, onclick: () => void) => el("button", { class: "effect", role: "radio", "aria-checked": String(!!on), title: `${title} · ${does}`, "aria-label": title, onclick },
  el("strong", {}, title), el("span", { class: "muted" }, does));

function wordsOptions(anim: any) { return wordsViewController.options(anim); }
const WORDS_VIEW_ICONS: Record<string, string> = {
  timing: '<svg viewBox="0 0 24 24"><path d="M4 5v14M20 5v14M4 9h16M4 15h16"/><circle cx="9" cy="9" r="2"/><circle cx="16" cy="15" r="2"/></svg>',
  lyrics: '<svg viewBox="0 0 24 24"><path d="M4 5h16M12 5v14M8 19h8"/></svg>',
  motion: '<svg viewBox="0 0 24 24"><path d="M3 17c4-13 8 6 13-8l4-4M14 5h6v6"/></svg>'
};
function wordsModeTabs() { return wordsViewController.modeTabs(); }
function wordsSetMode(mode: string) { return wordsViewController.setMode(mode); }
function wordsPersistLinks() { return wordsViewController.persistLinks(); }
function wordsLinksToggleButton() { return wordsViewController.linksToggleButton(); }
function wordsLinkControls() { return wordsViewController.linkControls(); }
function wordsMotionActions(anim: any) { return wordsViewController.motionActions(anim); }
function wordsDrawingEdit(anim: any, cue: any) { return wordsViewController.drawingEdit(anim, cue); }
function wordsDrawCue(anim: any, replaceIndex: number | null = null) { return wordsViewController.drawCue(anim, replaceIndex); }
function wordsCueEditor(anim: any) { return wordsViewController.cueEditor(anim); }
function wordsTimingPanel(anim: any) { return wordsViewController.timingPanel(anim); }
function wordsInnerControls(anim: any) { return wordsViewController.innerControls(anim); }
function wordsDesignerToolbar(anim: any) { return wordsViewController.designerToolbar(anim); }
function wordsHeaderMeta(anim: any) { return wordsViewController.headerMeta(anim); }
function wordsMountMain(view: any, anim: any, animations: any[], end: number, parts: any[]) { return wordsViewController.mountMain(view, anim, animations, end, parts); }
function wordsRender() { return wordsViewController.render(); }
function wordsFitClock(view: any) { return wordsViewController.fitClock(view); }
function wordsSizeInput(anim: any) { return wordsViewController.sizeInput(anim); }

addEventListener("pointermove", (event) => {
  const drag = wordsPanel.drag, anim = wordsCurrent();
  if (!drag || !anim) return;
  const design = wordsPanel.designs.get(wordsPanel.line) ?? structuredClone(wordsDesign(anim, wordsPanel.line));
  design.items[drag.i].x = Math.round(drag.from[0] + (event.clientX - drag.x) / drag.scale);
  design.items[drag.i].y = Math.round(drag.from[1] + (event.clientY - drag.y) / drag.scale);
  wordsPanel.designs.set(wordsPanel.line, design); drag.moved = true;
  wordsCanvasDraw();
});
addEventListener("pointerup", () => { const drag = wordsPanel.drag; wordsPanel.drag = null; if (drag?.moved) wordsRender(); });
addEventListener("keydown", (event: any) => {
  if (state.view !== "design" || event.key !== "Escape" || wordsPanel.item == null || /INPUT|SELECT|TEXTAREA/.test(event.target.tagName)) return;
  wordsPanel.item = null; wordsRender();
});

function wordsStyleFields(anim: any) { return wordsViewController.styleFields(anim); }
function wordsUseButton(anim: any) { return wordsViewController.useButton(anim); }
function wordsAvailableBackdrops() { return wordsViewController.availableBackdrops(); }
async function wordsPickBackdrop(anim: any, assetId: string | null) { return wordsViewController.pickBackdrop(anim, assetId); }
function wordsBackdropPicker(anim: any) { return wordsViewController.backdropPicker(anim); }
function wordsSongSpan() { return wordsViewController.songSpan(); }
function wordsToggleSongPlay() { return wordsViewController.toggleSongPlay(); }
function wordsStageEmptyMessage(url: string | null) { return wordsViewController.stageEmptyMessage(url); }
function wordsStage(anim: any) { return wordsViewController.stage(anim); }
async function wordsResolveShowUrl(show: string, anim: any) { return wordsViewController.resolveShowUrl(show, anim); }
async function wordsShow(show: string, frame?: any) { return wordsViewController.show(show, frame); }
function wordsSeekSong(t: number) { return wordsViewController.seekSong(t); }
function wordsStatusRefresh() { return wordsViewController.statusRefresh(); }
function wordsVideo(action: string, params: Record<string, any> = {}) { return wordsViewController.video(action, params); }
function wordsVideoRange() { return wordsViewController.videoRange(); }
function wordsFit(size = wordsPanel.size) { return wordsViewController.fit(size); }
function wordsFollowLivePlayback(song: number) { return wordsViewController.followLivePlayback(song); }
function wordsHandlePreviewBoundary(song: number, isPlaying: boolean) { return wordsViewController.handlePreviewBoundary(song, isPlaying); }
function wordsSyncPendingSeek(song: number, isPlaying: boolean) { return wordsViewController.syncPendingSeek(song, isPlaying); }
function wordsHandleStateMessage(data: any) { return wordsViewController.handleStateMessage(data); }

addEventListener("message", (event) => {
  const frame = $("words-video-frame");
  if (!frame || event.source !== frame.contentWindow || event.data?.source !== "hf-preview") return;
  const data = event.data;
  if (data.type === "stage-size") { wordsFit({ width: data.width, height: data.height }); wordsVideoRange(); }
  if (data.type === "state") wordsHandleStateMessage(data);
});

function wordsWidth(view = $("view-design")) { return wordsViewController.width(view); }
let wordsObservedWidth: string | number = -1;
const wordsViewFitter = new ResizeObserver(() => {
  const view = $("view-design"); if (!view) return;
  const width = `${view.clientWidth}:${innerHeight}:${view.getBoundingClientRect().top}`;
  if (width === wordsObservedWidth) return; wordsObservedWidth = width;
  requestAnimationFrame(() => { const view = $("view-design"); if (view?.classList.contains("active")) { wordsWidth(view); wordsFit(); wordsFitClock(view); wordsArrows(view); } });
});
addEventListener("resize", () => { wordsWidth(); wordsFit(); const view = $("view-design"); if (view) wordsArrows(view); });
new MutationObserver(() => {
  if ($("view-design")?.classList.contains("active")) return;
  if (wordsPanel.playing) wordsVideo("pause");
}).observe(document.getElementById("view-design") || document.body, { attributes: true, attributeFilter: ["class"] });

async function wordsKinetic(change: any) { return wordsViewController.kinetic(change); }
async function wordsShuffle(kinetic = true) { return wordsViewController.shuffle(kinetic); }

class WordsViewController {
  private buildLookOptions(lineId: string, design: any, it: any, picks: number[], itemText: (x: any) => string, edit: (change: (d: any) => void) => void, each: (d: any, change: (x: any) => void) => void) {
    const look = WORDS_LETTERING.find((l) => l[3] === (it.font || "song") && l[4] === it.weight && l[5] === it.italic)?.[0];
    const range = (label: string, key: string, min: number, max: number, unit = "") => {
      const input = el("input", {
        type: "range", min: String(min), max: String(max), step: "1", "aria-label": label,
        oninput: (e: any) => {
          const held = wordsPanel.designs.get(lineId) ?? structuredClone(design);
          each(held, (x) => { x[key] = Number(e.target.value); }); wordsPanel.designs.set(lineId, held); wordsCanvasDraw();
        }, onchange: () => wordsRender()
      });
      input.value = input.defaultValue = String(it[key]);
      return el("label", { class: "field-row" }, `${label} ${unit}`, input);
    };
    const colour = el("input", { type: "color", "aria-label": "Colour", onchange: (e: any) => edit((d) => each(d, (x) => { x.color = e.target.value; })) });
    colour.value = colour.defaultValue = /^#[0-9a-f]{6}$/i.test(it.color) ? it.color : "#000000";
    const outline = el("input", { type: "color", "aria-label": "Outline colour", onchange: (e: any) => edit((d) => each(d, (x) => { x.outline = e.target.value; })) });
    outline.value = outline.defaultValue = /^#[0-9a-f]{6}$/i.test(it.outline || "") ? it.outline : "#173b43";
    const outlined = el("input", { type: "checkbox", "aria-label": "Outline on", onchange: (e: any) => edit((d) => each(d, (x) => { x.outline = e.target.checked ? outline.value : null; })) });
    outlined.checked = outlined.defaultChecked = !!it.outline;
    return [el("div", { class: "group-label" }, `Lettering for “${picks.map((i) => itemText(design.items[i])).join(" ")}”`), el("div", { class: "words-effects" },
      ...WORDS_LETTERING.map(([key, name, does, font, weight, italic]) => wordsCard(look === key, String(name), String(does), () => edit((d) => each(d, (x) => Object.assign(x, { font, weight, italic })))))),
    el("div", { class: "words-head" }, el("label", { class: "field-row" }, "Colour", colour), el("label", { class: "field-row" }, outlined, "Outline", outline),
      range("Size", "size", 8, 600, "px"), range("Tilt", "rotate", -45, 45, "°"))];
  }

  options(anim: any) {
    const lineId = wordsPanel.line, design = wordsDesign(anim, lineId), it = design.items[wordsPanel.item], words = wordsLineWords(anim, lineId);
    const itemText = (x: any) => x.word_ids.map((id: string) => words.find((w: any) => w.word_id === id)?.text).join(" ");
    const edit = (change: (d: any) => void) => wordsDesignEdit(anim, lineId, change);
    const tab = wordsPanel.tab;
    if (tab === "look" && !it) return [el("div", { class: "group-label" }, "Line designs"), el("div", { class: "words-effects" },
      wordsCard(false, "Plain line", "Every word along the bottom, quiet", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "plain")))),
      wordsCard(false, "Poster", "The longest word big and italic, the rest small around it", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "poster")))),
      wordsCard(false, "Stack", "One word per row, the last one bold and settling", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "stack")))),
      anim.design?.[lineId] || wordsPanel.designs.has(lineId) ? wordsCard(false, "Clear the design", "Back to a plain line when saved", () => { wordsPanel.designs.set(lineId, null); wordsRender(); }) : null)];
    const picks = wordsPickedItems(design);
    const each = (d: any, change: (x: any) => void) => picks.forEach((i: number) => change(d.items[i]));
    if (tab === "look") return this.buildLookOptions(lineId, design, it, picks, itemText, edit, each);
    if (tab === "move" && !it) return [el("div", { class: "group-label" }, "Moves in this line"), el("div", { class: "words-effects" },
      ...design.items.map((x: any, i: number) => wordsCard(false, `“${itemText(x)}”`, x.motion === "quiet" ? "Quiet" : `${WORDS_MOVES.find((m) => m[0] === x.motion)?.[1]} on “${words.find((w: any) => w.word_id === x.accent)?.text ?? itemText(x)}”`,
        () => { wordsPanel.item = i; wordsRender(); })))];
    if (tab === "move") {
      const accent = el("select", { "aria-label": "Accent word", onchange: (e: any) => edit((d) => { d.items[wordsPanel.item].accent = e.target.value || null; }) },
        el("option", { value: "" }, "the line's start"), ...it.word_ids.map((id: string) => el("option", { value: id }, `“${words.find((w: any) => w.word_id === id)?.text}”`)));
      accent.value = it.accent || "";
      return [el("div", { class: "group-label" }, `How “${itemText(it)}” moves`), el("div", { class: "words-effects" },
        ...WORDS_MOVES.map(([key, name, does]) => wordsCard(it.motion === key, String(name), String(does), () => edit((d) => {
          const x = d.items[wordsPanel.item]; x.motion = key; if (key !== "quiet" && !x.accent) x.accent = x.word_ids.at(-1);
        })))),
      el("div", { class: "words-head" }, el("label", { class: "field-row" }, "Lands on", accent))];
    }
    const near = it || design.items[0];
    return [el("div", { class: "group-label" }, "Drawings in this line · click one to remove it"), el("div", { class: "words-effects" },
      ...design.draws.map((d: any, i: number) => wordsCard(true, d.name, `draws ${wordsClock(wordsLineSpan(anim, lineId).start + d.paths[0].from)}`, () => edit((x) => { x.draws.splice(i, 1); }))),
      design.draws.length ? null : el("p", { class: "muted" }, "No drawings yet.")),
    el("div", { class: "group-label" }, `Add a drawing ${it ? `by “${itemText(it)}”` : "to this line"}`), el("div", { class: "words-effects" },
      ...WORDS_STROKES.map((s) => wordsCard(false, s.name, s.does, () => edit((d) => {
        const accentWord = words.find((w: any) => w.word_id === near.accent), start = (accentWord ? wordsAt(accentWord) : wordsLineSpan(anim, lineId).start) - wordsLineSpan(anim, lineId).start;
        d.draws.push({
          name: s.name, view_box: s.view_box, x: Math.round(near.x), y: Math.round(near.y + near.size * 1.15), w: s.w, h: s.h,
          paths: [{ d: s.d, stroke: near.color, width: 6, from: Math.max(0, +start.toFixed(2)), to: +(Math.max(0, start) + s.draw).toFixed(2) }]
        });
      }))))];
  }

  modeTabs() {
    return el("div", { id: "words-view-modes", class: "segmented", role: "tablist", "aria-label": "Lyrics view" },
      ...Object.entries(WORDS_VIEW_ICONS).map(([mode, html]) => el("button", {
        role: "tab", "data-words-mode": mode, "aria-selected": String(wordsPanel.mode === mode),
        "aria-label": `${mode[0].toUpperCase() + mode.slice(1)} view`, title: `${mode[0].toUpperCase() + mode.slice(1)} controls`, onclick: () => this.setMode(mode)
      }, el("span", { class: "words-view-icon", html }), mode[0].toUpperCase() + mode.slice(1))));
  }

  setMode(mode: string) {
    if (!Object.hasOwn(WORDS_VIEW_ICONS, mode)) return;
    if (mode !== "motion") { document.querySelector(".words-sketch")?.remove(); document.querySelector(".words-sketch-cancel")?.remove(); }
    wordsPanel.mode = mode;
    if (mode === "motion") { wordsPanel.topShelf = "overlay"; wordsPanel.shelf = "cues"; }
    else if (mode === "lyrics" && !wordsPanel.topShelf) { wordsPanel.topShelf = "scene"; wordsPanel.shelf = "acts"; }
    this.render();
  }

  persistLinks() {
    try { localStorage.setItem("magicstudio.lyrics.links", JSON.stringify(wordsPanel.links)); } catch {}
    wordsArrows($("view-design"));
  }

  linksToggleButton() {
    const shown = !wordsPanel.links.hidden;
    const label = shown ? "Hide lyric-to-cue links" : "Show lyric-to-cue links";
    return el("button", {
      class: "icon words-links-toggle",
      "aria-label": label,
      title: label,
      "aria-pressed": String(shown),
      html: '<svg viewBox="0 0 24 24"><path d="M4 17c4-10 8 4 16-10"/><circle cx="4" cy="17" r="2"/><circle cx="20" cy="7" r="2"/></svg>',
      onclick: (e: any) => {
        wordsPanel.links.hidden = !wordsPanel.links.hidden;
        e.currentTarget.setAttribute("aria-pressed", String(!wordsPanel.links.hidden));
        this.persistLinks();
      }
    });
  }

  linkControls() {
    const links = wordsPanel.links;
    const colour = el("input", { type: "color", "aria-label": "Connection colour", title: "Connection colour", oninput: (e: any) => { links.color = e.target.value; e.target.defaultValue = e.target.value; this.persistLinks(); } });
    colour.value = colour.defaultValue = links.color || "#70c4ff";
    const toggle = (key: string, label: string, icon: string) => el("button", { class: "icon", "aria-label": label, title: label, "aria-pressed": String(links[key]), html: icon, onclick: (e: any) => { links[key] = !links[key]; e.currentTarget.setAttribute("aria-pressed", String(links[key])); this.persistLinks(); } });
    return el("div", { id: "words-link-controls", class: "transport" }, el("span", { class: "group-label" }, "Connections"), colour,
      this.linksToggleButton(),
      toggle("front", "Bring connections in front", '<svg viewBox="0 0 24 24"><rect x="4" y="8" width="12" height="12"/><path d="M8 4h12v12M11 13l8-8M13 5h6v6"/></svg>'),
      toggle("pulse", "Slow connection colour blend", '<svg viewBox="0 0 24 24"><path d="M3 12c3-8 6 8 9 0s6 8 9 0"/></svg>'));
  }

  motionActions(anim: any) {
    return el("div", { class: "transport", "aria-label": "Add visual cue" },
      el("button", { class: "start", "aria-label": "Draw cue on scene", onclick: () => this.drawCue(anim) }, "Draw cue"),
      el("button", { class: "ghost", "aria-label": "Add scene effect", onclick: () => { wordsPanel.motionAdd = "effect"; this.render(); } }, "Add effect"));
  }

  drawingEdit(anim: any, cue: any) {
    if (cue.index == null) return null;
    const draw = anim.kinetic.draws[cue.index];
    const change = (patch: any) => this.kinetic({ draws: anim.kinetic.draws.map((d: any, i: number) => i === cue.index ? { ...d, ...patch } : d) });
    const field = (key: string, label: string) => {
      const input = el("input", {
        type: "number", placeholder: "Auto", "aria-label": `${label} of drawing`, onchange: (e: any) => {
          if (e.target.value !== "" && Number.isFinite(Number(e.target.value))) { e.target.defaultValue = e.target.value; change({ [key]: Number(e.target.value) }); }
        }
      });
      input.value = input.defaultValue = draw[key] ?? ""; return el("label", { class: "field-row" }, label, input);
    };
    const color = el("input", { type: "color", "aria-label": "Drawing colour", onchange: (e: any) => { e.target.defaultValue = e.target.value; change({ color: e.target.value }); } });
    color.value = color.defaultValue = draw.color || "#e4572e";
    return el("div", { class: "words-drawing-edit" }, field("x", "X"), field("y", "Y"), field("width", "Size"), color,
      el("button", { class: "ghost", "aria-label": "Redraw selected cue", onclick: () => this.drawCue(anim, cue.index) }, "Redraw"));
  }

  drawCue(anim: any, replaceIndex: number | null = null) {
    const stage = $("words-preview"); if (!stage || stage.querySelector(".words-sketch")) return;
    const previous = replaceIndex == null ? null : anim.kinetic?.draws?.[replaceIndex];
    const word = anim.words.find((w: any) => w.word_id === (previous?.word_id || wordsPanel.picked[0])) || wordsLineWords(anim, wordsPanel.line)[0];
    if (!word) return toast("Select a lyric line to attach the drawing");
    const project = state.project, size = wordsPanel.size || { width: 1920, height: 1080 }; this.video("pause");
    const overlay = document.createElementNS("http://www.w3.org/2000/svg", "svg"), path = document.createElementNS(overlay.namespaceURI, "path");
    overlay.setAttribute("viewBox", `0 0 ${size.width} ${size.height}`); overlay.setAttribute("class", "words-sketch");
    overlay.setAttribute("tabindex", "0"); overlay.setAttribute("aria-label", "Draw a cue on the scene; Escape cancels");
    path.setAttribute("fill", "none"); path.setAttribute("stroke", previous?.color || "#e4572e"); path.setAttribute("stroke-width", "7"); path.setAttribute("stroke-linecap", "round"); overlay.appendChild(path);
    const cancel = el("button", { class: "words-sketch-cancel", "aria-label": "Cancel drawing", onclick: () => { overlay.remove(); cancel.remove(); } }, "Cancel drawing");
    stage.appendChild(overlay); stage.appendChild(cancel); overlay.focus();
    let points: number[][] = [], active = false, pending = false, stored = false;
    const savedIndex = replaceIndex ?? anim.kinetic?.draws?.length ?? 0;
    const point = (e: PointerEvent) => { const r = overlay.getBoundingClientRect(); return [Math.round(Math.max(0, Math.min(size.width, (e.clientX - r.left) * size.width / r.width))), Math.round(Math.max(0, Math.min(size.height, (e.clientY - r.top) * size.height / r.height)))]; };
    const paint = () => path.setAttribute("d", points.map((p, i) => `${i ? "L" : "M"}${p[0]} ${p[1]}`).join(" "));
    overlay.onpointerdown = (e) => { if (pending) return; e.preventDefault(); overlay.setPointerCapture(e.pointerId); points = [point(e)]; active = true; paint(); };
    overlay.onpointermove = (e) => { if (!active || points.length >= 500) return; const p = point(e), last = points.at(-1)!; if (Math.hypot(p[0] - last[0], p[1] - last[1]) > 2) { points.push(p); paint(); } };
    const save = async () => {
      if (points.length < 2) return;
      if (state.project !== project || wordsPanel.selected !== anim.animation_id) return toast("Selection changed; drawing was not saved");
      pending = true;
      const drawing = {
        ...previous, word_id: word.word_id, track: "graphics", drawing: { name: "Drawing", view_box: `0 0 ${size.width} ${size.height}`, d: path.getAttribute("d") },
        x: 0, y: 0, width: size.width, color: previous?.color || "#e4572e", duration: previous?.duration || 1.2, hold: previous?.hold ?? 1
      };
      const draws = [...(anim.kinetic?.draws || [])]; if (replaceIndex == null) draws.push(drawing); else draws[replaceIndex] = drawing;
      try {
        const kinetic = { layout: "karaoke", entrance: "fade-up", overlay: "none", ...anim.kinetic, draws };
        if (!stored) {
          const result = await wordsCommand("words.save", { animation_id: anim.animation_id, expected_source_revision: anim.source_revision, kinetic, _expected_project: project });
          if (result.conflict) throw new Error("This section changed; cancel and reload before drawing again");
          anim.source_revision = result.animation.source_revision; anim.kinetic = kinetic; stored = true;
        }
        if (wordsPanel.read.in_video?.includes(anim.animation_id)) await wordsCommand("words.apply", { animation_id: anim.animation_id, _expected_project: project });
        wordsPanel.cue = `${anim.animation_id}-draw-${savedIndex}`; wordsPanel.stage = null; wordsPanel.motionAdd = null;
        overlay.remove(); cancel.remove(); await wordsOpen();
      } catch (error) { pending = false; cancel.textContent = "Retry save"; cancel.setAttribute("aria-label", "Retry drawing save"); cancel.onclick = save; toast(String(error)); }
    };
    overlay.onpointerup = () => { if (!active) return; active = false; save(); };
    overlay.onpointercancel = () => { active = false; points = []; paint(); };
    overlay.onkeydown = (e) => { if (e.key === "Escape") { overlay.remove(); cancel.remove(); } };
  }

  cueEditor(anim: any) {
    const cue = wordsPanel.read.graphics?.find((c: any) => c.id === wordsPanel.cue && c.animation_id === anim.animation_id);
    if (!cue) return el("p", { class: "muted" }, "Select a drawing or effect clip on the right. Drag its top to move it, its lower edge to trim it, or type exact seconds here.");
    const field = (label: string, value: number, apply: (v: number) => void) => {
      const input = el("input", {
        type: "number", step: "0.01", min: "0", "aria-label": `${label} of ${cue.name} (seconds)`,
        onchange: (e: any) => { const v = Number(e.target.value); if (e.target.value !== "" && Number.isFinite(v) && v >= 0) { e.target.defaultValue = e.target.value; apply(v); } }
      });
      input.value = input.defaultValue = value.toFixed(2); return el("label", { class: "field-row" }, label, input);
    };
    return el("div", { class: "words-timing-summary", "data-cue-kind": cue.kind }, el("strong", {}, cue.name),
      field("Start", cue.start, (v) => wordsRetimeCue(cue, v, v + cue.end - cue.start)), field("End", cue.end, (v) => wordsRetimeCue(cue, cue.start, v)),
      el("button", { class: "icon", "aria-label": "Replay linked learning moment", title: "Replay the lyric and this cue together", html: ICONS.play, onclick: () => wordsPreviewCue(cue) }),
      this.drawingEdit(anim, cue),
      el("button", { class: "icon", "aria-label": "Delete selected visual cue", title: "Delete this cue", onclick: () => { wordsPanel.cue = null; this.kinetic(cue.kind === "overlay" ? { overlay: "none" } : { draws: anim.kinetic.draws.filter((_: any, i: number) => i !== cue.index) }); }, html: '<svg viewBox="0 0 24 24"><path d="M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7"/></svg>' }),
      el("p", { class: "muted" }, cue.word ? `Linked to “${cue.word}” · drag the left point to choose another word.` : "This atmosphere effect spans the section; adjust Start/End or pick another Atmosphere above."));
  }

  timingPanel(anim: any) {
    return el("section", { class: "words-timing", "aria-label": "Timing controls" }, this.linkControls(), this.cueEditor(anim));
  }

  innerControls(anim: any) {
    if (wordsPanel.mode === "timing") return [this.timingPanel(anim)];
    if (anim.kinetic) return [wordsLayers(anim)];
    wordsPanel.tab = wordsPanel.mode === "motion" ? "draw" : "look";
    return [wordsLanes(anim), ...this.options(anim)];
  }

  designerToolbar(anim: any) {
    const segmented = (label: string, current: string, options: string[][], pick: (key: string) => void) => el("div", { class: "segmented", role: "tablist", "aria-label": label },
      ...options.map(([key, name]) => el("button", { role: "tab", "aria-selected": String(current === key), onclick: () => pick(key) }, name)));
    const inVideo = wordsPanel.read.in_video?.includes(anim.animation_id);
    return el("div", { class: "words-toolbar" },
      el("button", { class: "icon", "aria-label": "Play this line", title: "Play from this line", onclick: wordsPlay, html: ICONS.play }),
      el("span", { class: "meta" }, wordsClock(wordsLineSpan(anim, wordsPanel.line).start)),
      segmented("Behind the lyrics", wordsPanel.scene ? "scene" : "paper", [["scene", "Scene"], ["paper", "Paper"]], (key) => { wordsPanel.scene = key === "scene"; this.render(); }),
      el("span", { class: "words-spacer" }),
      el("button", { class: "ghost", title: inVideo ? "Put the video's original lyrics back for this section" : "Replace this section's lyrics in the video with the designed lines", onclick: () => wordsApply(!inVideo) },
        inVideo ? "In the video · Restore original" : "Use in the video"));
  }

  headerMeta(anim: any) {
    const lines = wordsLines(anim);
    if (!lines.some((l) => l.id === wordsPanel.line)) {
      wordsPanel.line = lines[0]?.id || null;
      wordsPanel.item = null;
      if (!wordsPanel.playing) wordsPanel.time = wordsPanel.line ? wordsLineSpan(anim, wordsPanel.line).start + 0.35 : 0;
    }
    const activeLine = lines.find((l) => l.id === wordsPanel.line) || lines[0];
    if (activeLine && !wordsPanel.picked.some((id: string) => anim.words.some((w: any) => w.word_id === id))) {
      const hero = activeLine.words.find((w: any) => anim.kinetic?.acts?.[w.word_id]) || activeLine.words[0];
      if (hero) wordsPanel.picked = [hero.word_id];
    }
    const design = wordsPanel.line ? wordsDesign(anim, wordsPanel.line) : null, it = design?.items[wordsPanel.item];
    const lineNo = lines.findIndex((l) => l.id === wordsPanel.line) + 1;
    const itemText = it ? it.word_ids.map((id: string) => anim.words.find((w: any) => w.word_id === id)?.text).join(" ") : "";
    return { lineNo, description: `${wordsLabel(anim)} · line ${lineNo}${it ? ` · ${itemText}` : ""}` };
  }

  mountMain(view: any, anim: any, animations: any[], end: number, parts: any[]) {
    const main = wordsPanel.mainEl ||= el("section", { id: "words-main" });
    main.setAttribute("aria-label", `${wordsLabel(anim)} lyrics`);
    const [head, notice, upper, stageNode, toolbar, controls] = parts;
    const before = [head, ...(notice ? [notice] : []), upper];
    const after = [...(toolbar ? [toolbar] : []), controls];
    if (stageNode.parentElement === main) {
      for (const child of [...main.children]) { if (child !== stageNode) child.remove(); }
      stageNode.before(...before);
      stageNode.after(...after);
    } else {
      main.replaceChildren(...before, stageNode, ...after);
    }
    const left = wordsPanel.leftEl ||= el("aside", { id: "words-lyrics" });
    const right = wordsPanel.rightEl ||= el("aside", { id: "words-film" });
    left.replaceChildren(new PanelHeader({ id: "words-lyrics-head", title: "Lyrics" }).element, wordsTimeline(animations, end));
    right.replaceChildren(new PanelHeader({ id: "words-film-head", title: "Visual cues" }).element, wordsRoll(animations, end));
    for (const [node, id] of [[main, "words-main"], [left, "words-lyrics"], [right, "words-film"]] as const) registerElement(node, id);
    if (main.parentElement !== view || left.parentElement !== view || right.parentElement !== view) {
      view.replaceChildren(left, main, right);
    }
  }

  render() {
    const view = $("view-design"), anim = wordsCurrent(), animations = wordsPanel.read?.document?.animations || [];
    if (!anim) { view.replaceChildren(el("p", { class: "panel-empty inline" }, "No lyric sections yet.")); return; }
    const { lineNo, description } = this.headerMeta(anim);
    const end = Math.max(...animations.map((a: any) => wordsSpan(a).end)) + 4;
    const changes = wordsPanel.designs.size + Object.keys(wordsPanel.styleEdits).length;
    const saveLabel = changes ? `Save ${changes} change${changes > 1 ? "s" : ""} & sync Studio` : "Save & view in HyperFrames Studio";
    const save = el("button", {
      id: "words-save", class: "icon", title: saveLabel, "aria-label": saveLabel, onclick: () => wordsSave(!changes),
      html: '<svg viewBox="0 0 24 24"><path d="M5 3h12l4 4v14H3V3h2zM7 3v6h9V3M7 21v-8h10v8"/></svg>'
    });
    const designer = wordsPanel.view === "design" && wordsPanel.line && !anim.kinetic;
    this.mountMain(view, anim, animations, end, [
      new PanelHeader({
        id: "words-main-head", title: `${wordsLabel(anim)} · ${lineNo}`, description,
        actions: [this.modeTabs(), this.linksToggleButton(), save]
      }).element,
      wordsPanel.read.reference_changed ? el("p", { class: "not-linked" }, "The timing reference changed since this was saved; check the words before saving.") : null,
      wordsUpperTracks(anim),
      designer ? wordsCanvas(anim) : this.stage(anim),
      designer ? this.designerToolbar(anim) : null,
      el("div", { id: "words-workspace-controls" }, ...this.innerControls(anim))]);
    view.dataset.wordsMode = wordsPanel.mode;
    view.onwheel = (event: any) => {
      if (event.target.closest?.("#words-workspace-controls, .words-backgrounds")) return;
      wordsPanel.userWheelUntil = performance.now() + 260;
      wordsPanel.programmaticScrollUntil = 0;
      if (wordsPanel.playing) { this.video("pause"); wordsPanel.playing = false; }
    };
    view.onscroll = () => wordsSyncViewScroll(view);
    wordsViewFitter.observe(view); if ($("magic-workstation")) wordsViewFitter.observe($("magic-workstation")); this.width(view);
    requestAnimationFrame(() => {
      wordsCanvasDraw(); this.fitClock(view);
      const pick = `${wordsPanel.selected}/${wordsPanel.line}`;
      if (pick !== wordsPanel.followed && wordsPanel.line && !wordsPanel.playing) {
        wordsPanel.followed = pick;
        wordsFollow(wordsPanel.time || wordsLineSpan(anim, wordsPanel.line).start, false);
      }
      wordsArrows(view);
    });
    wordsNow(wordsPanel.time);
    this.fit();
  }

  fitClock(view: any) {
    view.style.removeProperty("--song-s");
    const base = parseFloat(getComputedStyle(view).getPropertyValue("--song-s-px")) || 14;
    let scale = base;
    for (const row of view.querySelectorAll(".words-tl > .words-line")) {
      const seconds = Number(row.style.getPropertyValue("--len-s"));
      if (seconds > 0) scale = Math.max(scale, (row.scrollHeight + parseFloat(getComputedStyle(row).marginTop || "0") + 4) / seconds);
    }
    if (scale > base) view.style.setProperty("--song-s", `${Math.ceil(scale)}px`);
  }

  sizeInput(anim: any) {
    const input = el("input", {
      type: "range", min: "60", max: "220", step: "2", "aria-label": "Word size",
      oninput: (e: any) => { wordsPanel.styleEdits.size = Number(e.target.value); if (wordsPanel.styleEdits.size === (anim.style?.size ?? 120)) delete wordsPanel.styleEdits.size; this.render(); }
    });
    input.value = String(wordsPanel.styleEdits.size ?? anim.style?.size ?? 120);
    input.defaultValue = String(anim.style?.size ?? 120);
    return input;
  }

  styleFields(anim: any) {
    const field = (key: string, label: string, type: string, fallback: string) => {
      const saved = anim.style?.[key] ?? fallback;
      const input = el("input", {
        type, "aria-label": label, oninput: (e: any) => {
          if (e.target.value === saved) delete wordsPanel.styleEdits[key]; else wordsPanel.styleEdits[key] = e.target.value;
        },
        onchange: () => this.render()
      });
      input.value = wordsPanel.styleEdits[key] ?? saved;
      input.defaultValue = saved;
      return el("label", { class: "field-row" }, label, input);
    };
    return [field("font", "Font", "text", ""), field("color", "Colour", "color", "#ffffff"), field("emphasis_color", "Emphasis", "color", "#ffd078"),
      field("outline_color", "Outline", "color", "#173b43"), field("background", "Background", "text", "")];
  }

  useButton(anim: any) {
    const used = wordsPanel.read.in_video?.includes(anim.animation_id);
    const label = used ? "Restore original video lyrics" : "Use this lyric design in the video";
    return el("button", {
      class: "icon", title: label, "aria-label": label, "aria-pressed": String(!!used), onclick: () => wordsApply(!used),
      html: '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 12 3 3 7-7"/></svg>'
    });
  }

  availableBackdrops() {
    const seen = new Set<string>(), list: Array<{ id: string; name: string; path: string }> = [];
    for (const img of [...(wordsPanel.backgrounds?.images || []), ...(wordsPanel.read?.images || [])]) {
      const id = img.asset_id || img.id;
      if (!id || !img.path || seen.has(id)) continue;
      seen.add(id);
      list.push({ id, name: img.name || id, path: img.path });
    }
    return list;
  }

  async pickBackdrop(anim: any, assetId: string | null) {
    const heldTime = wordsPanel.time, wasPlaying = wordsPanel.playing;
    try {
      const result = await wordsCommand("words.save", {
        animation_id: anim.animation_id,
        expected_source_revision: anim.source_revision,
        background_asset: assetId,
        _expected_project: state.project
      });
      if (result.conflict) { toast("This verse changed elsewhere; reloaded"); return wordsOpen(); }
      anim.source_revision = result.animation.source_revision;
      anim.background_asset = result.animation.background_asset;
      anim.background_custom = result.animation.background_custom;
      if (wordsPanel.read?.in_video?.includes(anim.animation_id)) {
        await wordsCommand("words.apply", { animation_id: anim.animation_id, use: true });
      }
      wordsPanel.show = assetId ? "still" : "stage";
      wordsPanel.stageDirty = true;
      document.querySelector("#words-main .words-backdrop")?.replaceWith(this.backdropPicker(anim));
      await this.show(wordsPanel.show);
      wordsPanel.time = heldTime;
      this.seekSong(heldTime);
      if (wasPlaying) this.video("play");
    } catch (error) { toast(String(error)); }
  }

  backdropPicker(anim: any) {
    const images = this.availableBackdrops();
    const isFullVideo = wordsPanel.show === "stage" || (!anim.background_custom && !anim.background_asset);
    const videoBtn = el("button", {
      class: "words-background words-background-mode",
      title: "Play full animated video scene",
      "aria-label": "Use full video scene",
      "aria-pressed": String(isFullVideo),
      onclick: () => this.pickBackdrop(anim, null)
    }, el("span", { class: "words-background-label" }, "Video"));
    return el("div", { class: "words-backdrop", role: "group", "aria-label": "Scene backgrounds" },
      el("span", { class: "meta" }, "Scene"),
      el("div", { class: "words-backgrounds" },
        videoBtn,
        ...images.map((image) => el("button", {
          class: "words-background",
          title: image.name,
          "aria-label": `Use background ${image.name}`,
          "aria-pressed": String(!isFullVideo && anim.background_asset === image.id),
          onclick: () => this.pickBackdrop(anim, image.id)
        }, el("img", { src: convertFileSrc(image.path), alt: image.name, loading: "lazy" })))));
  }

  songSpan() {
    const animations = wordsPanel.read?.document?.animations || [];
    const studioEnds = (wordsPanel.studio?.sections || []).map((s: any) => Number(s.end) || 0);
    const animEnds = animations.map((a: any) => wordsSpan(a).end || 0);
    return { start: 0, end: Math.max(10, ...studioEnds, ...animEnds) };
  }

  toggleSongPlay() {
    if (wordsPanel.playing) { this.video("pause"); return; }
    wordsPanel.previewRange = null;
    const full = this.songSpan(), o = wordsPanel.offset;
    this.video("set-play-range", { startSeconds: 0, endSeconds: Math.max(1, full.end - o) });
    if (wordsPanel.time >= full.end - 0.5) this.seekSong(0);
    this.video("play");
  }

  stageEmptyMessage(url: string | null) {
    if (!url) return "Start the Studio server to play the video here.";
    return wordsPanel.studio?.error
      ? `The video's lyric clips could not be read: ${wordsPanel.studio.error}`
      : "The video has no lyric clips for this section.";
  }

  stage(anim: any) {
    const section = wordsStudioSection(anim) || wordsSpan(anim), songSpan = this.songSpan();
    const url = serverUrl();
    wordsPanel.server = url || null;
    if (!section || !url) {
      wordsPanel.stage = null;
      return el("p", { class: "panel-empty inline" }, this.stageEmptyMessage(url));
    }
    if (wordsPanel.stage && wordsPanel.show !== "edited") {
      wordsPanel.stage.dataset.animation = anim.animation_id;
      if (wordsPanel.stageDirty) this.show(wordsPanel.show);
      return wordsPanel.stage;
    }
    const scrub = el("input", {
      type: "range", id: "words-scrub", min: "0", max: String(songSpan.end), step: "0.01", "aria-label": "Video time",
      oninput: (e: any) => {
        e.target.defaultValue = e.target.value;
        const t = Number(e.target.value);
        if (wordsPanel.playing) { this.video("pause"); wordsPanel.playing = false; }
        wordsPanel.previewRange = null; wordsPanel.autoPlay = false;
        wordsPanel.userScrollingUntil = performance.now() + 220;
        this.seekSong(t);
        this.followLivePlayback(t);
        wordsNow(t);
        wordsFollow(t, false);
      }
    });
    scrub.value = scrub.defaultValue = String(wordsPanel.time || section.start);
    const server = new URL(url), project = server.hash.split("/")[1] || "hyperframes";
    wordsPanel.player = `${server.origin}/api/projects/${project}/preview`;
    const frame = el("iframe", { id: "words-video-frame", title: "Lyric and drawing preview", allow: "autoplay" });
    frame.addEventListener("load", () => this.videoRange());
    const view = (show: string, label: string) => el("button", { role: "tab", "data-show": show, "aria-selected": String(wordsPanel.show === show), onclick: () => this.show(show) }, label);
    wordsPanel.stage = el("div", { class: "words-preview", "data-animation": anim.animation_id },
      anim.effects?.text ? el("div", { class: "segmented", role: "tablist", "aria-label": "What the stage shows" },
        view("still", "Scene image"), view("edited", `Edited · ${anim.effects.text.name}`)) : null,
      el("div", { class: "stage", id: "words-preview" }, frame),
      el("div", { class: "transport" },
        wordsPanel.playButton = el("button", { class: "icon", "aria-label": "Play song", title: "Play or pause the entire song", onclick: () => this.toggleSongPlay(), html: ICONS.play }),
        scrub,
        wordsPanel.timeLabel = el("span", { class: "meta" }, `${wordsClock(wordsPanel.time || 0)} / ${wordsClock(songSpan.end)}`), this.useButton(anim)));
    if (wordsPanel.show === "edited" && !anim.effects?.text) wordsPanel.show = "still";
    this.show(wordsPanel.show, frame);
    return wordsPanel.stage;
  }

  async resolveShowUrl(show: string, anim: any) {
    if (show === "edited") {
      return `${wordsPanel.player}/comp/${(await wordsCommand("words.compose", { animation_id: anim.animation_id })).src}`;
    }
    if (!["lyrics", "stage", "still"].includes(show)) return wordsPanel.player;
    const effectiveShow = show === "still" && !anim.background_asset && !this.availableBackdrops().length ? "stage" : show;
    return `${wordsPanel.player}/comp/${(await wordsCommand("words.lyrics_view", { show: effectiveShow, animation_id: anim.animation_id })).src}`;
  }

  async show(show: string, frame = wordsPanel.stage?.querySelector("#words-video-frame") || $("words-video-frame")) {
    const anim = wordsCurrent();
    wordsPanel.show = show;
    wordsPanel.stage?.querySelectorAll('[role="tab"]').forEach((tab: any) => tab.setAttribute("aria-selected", String(tab.dataset.show === show)));
    if (!frame || !wordsPanel.player || !anim) return;
    wordsPanel.offset = show === "edited" ? anim.song_anchor || 0 : 0;
    try {
      const src = await this.resolveShowUrl(show, anim);
      const cleanCurrent = (frame.getAttribute("src") || "").split("?")[0];
      if (cleanCurrent !== src || wordsPanel.stageDirty) {
        wordsPanel.stageDirty = false;
        frame.setAttribute("src", `${src}?v=${Date.now()}`);
      } else this.videoRange();
    } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
  }

  seekSong(t: number) {
    wordsPanel.time = t;
    wordsPanel.pendingSeek = t;
    const scrub = $("words-scrub");
    if (scrub) { scrub.value = String(t); scrub.defaultValue = scrub.value; }
    if (wordsPanel.timeLabel) wordsPanel.timeLabel.textContent = `${wordsClock(t)} / ${wordsClock(this.songSpan().end)}`;
    this.video("seek", { timeSeconds: Math.max(0, t - wordsPanel.offset), seekMode: "commit" });
  }

  statusRefresh() {
    const url = serverUrl() || null;
    if (url === wordsPanel.server || !wordsCurrent()) return;
    wordsPanel.stage = null;
    this.render();
    this.videoRange();
  }

  video(action: string, params: Record<string, any> = {}) {
    $("words-video-frame")?.contentWindow?.postMessage({ source: "hf-parent", type: "control", action, ...params }, "*");
  }

  videoRange() {
    const anim = wordsCurrent();
    if (!anim) return;
    const full = this.songSpan(), range = wordsPanel.previewRange || full;
    const o = wordsPanel.offset;
    this.video("set-play-range", { startSeconds: Math.max(0, range.start - o), endSeconds: Math.max(1, range.end - o) });
    if (!wordsPanel.playing) this.seekSong(wordsPanel.autoPlay && wordsPanel.previewRange ? range.start : (wordsPanel.time || wordsSpan(anim).start || 0));
    if (wordsPanel.autoPlay) this.video("play");
  }

  fit(size = wordsPanel.size) {
    const frame = $("words-video-frame"), box = $("words-preview");
    if (!frame || !box || !size) return;
    wordsPanel.size = size;
    frame.style.setProperty("--w", `${size.width}px`); frame.style.setProperty("--h", `${size.height}px`);
    const scale = Math.min(box.clientWidth / size.width, box.clientHeight / size.height);
    frame.style.setProperty("--scale", String(scale));
    frame.style.setProperty("--frame-x", `${(box.clientWidth - size.width * scale) / 2}px`);
    frame.style.setProperty("--frame-y", `${(box.clientHeight - size.height * scale) / 2}px`);
  }

  followLivePlayback(song: number) {
    const animations = wordsPanel.read?.document?.animations || [];
    const unsaved = wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length;
    if (unsaved || wordsPanel.previewRange) return;
    const here = animations.find((a: any) => { const s = wordsSpan(a); return song >= s.start - 0.3 && song < s.end + 0.1; });
    const view = $("view-design");
    if (here && here.animation_id !== wordsPanel.selected) {
      wordsPanel.selected = here.animation_id;
      view?.querySelectorAll(".words-band[data-section]").forEach((b: any) => b.classList.toggle("active", b.dataset.section === here.animation_id));
      document.querySelector("#words-main .words-upper-tracks")?.replaceWith(wordsUpperTracks(here));
      $("words-workspace-controls")?.replaceChildren(...this.innerControls(here));
    }
    const current = wordsCurrent();
    if (!current) return;
    const line = wordsLines(current).findLast((l) => wordsAt(l.words[0]) <= song + 0.25) || wordsLines(current)[0];
    if (line && line.id !== wordsPanel.line) {
      wordsPanel.line = line.id;
      view?.querySelectorAll(".words-tl > .words-line").forEach((r: any) => r.classList.toggle("active", r.dataset.animationId === current.animation_id && r.dataset.lineId === line.id));
      const lineNo = wordsLines(current).findIndex((l) => l.id === line.id) + 1;
      const headTitle = $("words-main-head")?.querySelector(".title");
      if (headTitle) headTitle.textContent = `${wordsLabel(current)} · ${lineNo}`;
    }
  }

  handlePreviewBoundary(song: number, isPlaying: boolean) {
    const range = wordsPanel.previewRange;
    if (!range || song < range.end - 0.04) return false;
    if (wordsPanel.loopLine) {
      this.seekSong(range.start);
      if (!isPlaying) this.video("play");
      return true;
    }
    if (isPlaying) {
      this.video("pause");
      this.seekSong(Math.max(range.start, range.end - 0.18));
      return true;
    }
    return false;
  }

  syncPendingSeek(song: number, isPlaying: boolean) {
    if (wordsPanel.pendingSeek == null) return false;
    if (Math.abs(song - wordsPanel.pendingSeek) <= 0.35 || isPlaying) {
      wordsPanel.pendingSeek = null;
      if (wordsPanel.autoPlay && !isPlaying) this.video("play");
      return false;
    }
    this.seekSong(wordsPanel.pendingSeek);
    if (wordsPanel.autoPlay) this.video("play");
    return true;
  }

  handleStateMessage(data: any) {
    const song = data.currentTime + wordsPanel.offset;
    if (!data.isPlaying && performance.now() < (wordsPanel.userScrollingUntil || 0)) return;
    if (this.syncPendingSeek(song, data.isPlaying)) return;
    wordsPanel.time = song; wordsPanel.playing = !!data.isPlaying;
    if (this.handlePreviewBoundary(song, data.isPlaying)) return;
    if (data.isPlaying) wordsPanel.autoPlay = false;
    if (data.isPlaying || !wordsPanel.previewRange) this.followLivePlayback(song);
    const scrub = $("words-scrub");
    if (scrub) { scrub.value = String(song); scrub.defaultValue = scrub.value; }
    if (wordsPanel.timeLabel) wordsPanel.timeLabel.textContent = `${wordsClock(song)} / ${wordsClock(this.songSpan().end)}`;
    wordsNow(song);
    wordsPanel.playButton?.setAttribute("aria-label", data.isPlaying ? "Pause song" : "Play song");
  }

  width(view = $("view-design")) {
    if (!view) return;
    view.style.setProperty("--words-available", `${Math.max(220, innerHeight - Math.max(0, view.getBoundingClientRect().top) - 48)}px`);
    view.classList.toggle("narrow", view.clientWidth < 1100);
    view.classList.toggle("tight", view.clientWidth < 600);
  }

  async kinetic(change: any) {
    const anim = wordsCurrent();
    if (!anim?.kinetic) return;
    const heldLine = wordsPanel.line || wordsLines(anim)[0]?.id;
    const kinetic = { ...anim.kinetic, ...change };
    try {
      const result = await wordsCommand("words.save", { animation_id: anim.animation_id, expected_source_revision: anim.source_revision, origin: "magicstudio-design", kinetic });
      if (result.conflict) { toast("Someone changed these lyrics first; reloaded the latest"); return wordsOpen(); }
      await wordsCommand("words.apply", { animation_id: anim.animation_id, use: true });
      wordsPanel.line = heldLine;
      wordsPanel.followed = `${anim.animation_id}/${heldLine}`;
      wordsPanel.previewRange = wordsLineWindow(anim, heldLine);
      wordsPanel.autoPlay = true;
      wordsPanel.time = wordsPanel.previewRange.start;
      wordsPanel.stageDirty = true;
      await wordsOpen();
    } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
  }

  async shuffle(kinetic = true) {
    const anim = wordsCurrent();
    if (!anim) return;
    try {
      if (kinetic && anim.kinetic) {
        const lib = wordsPanel.read.kinetic_library || {};
        const pick = (items: any, current: string) => { const keys = Object.keys(items || {}); return keys[Math.floor(Math.random() * keys.length)] || current; };
        return this.kinetic({ layout: pick(lib.layouts, anim.kinetic.layout), entrance: pick(lib.entrances, anim.kinetic.entrance), seed: Math.floor(Math.random() * 1000) });
      }
      await wordsCommand("words.design", { section: anim.animation_id, seed: Math.floor(Math.random() * 1000), kinetic });
      wordsPanel.stage = null; await wordsOpen();
    } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
  }
}

const wordsViewController = new WordsViewController();
(window as any).WordsViewController = WordsViewController;
(window as any).wordsViewController = wordsViewController;
