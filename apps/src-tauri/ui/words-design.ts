// Design > Lyrics: design each sung line of the song, one verse/chorus at a time.
// Words come from the artist's sheet and times from the video's lyric track (words.studio_track);
// neither is edited here. Each line gets a design (where its words sit, their lettering, how they
// move on a sung word, and strokes that draw on), saved with words.save. The lyrics (left) and a
// film roll of the designed lines (right) run down the page on the song clock; the stage, the
// line's lanes and the options for what is picked stay pinned in the centre. The Video view plays
// the finished HyperFrames video through its preview player.
// The Lyrics view's one state bag; the fields set here are its starting state and the rest are
// added as the view runs, hence the open index signature.
const wordsPanel: Record<string, any> = { mode:"lyrics", topShelf:"scene", shelf:"acts", links:{color:null,front:false,pulse:true,hidden:false}, linkDrag:null, cue: null, cueDrag: null, previewRange: null, autoPlay: false, loopLine: false, stageDirty: false, pendingSeek: null, read: null, align: null, studio: null, selected: null, line: null, item: null, picked: [], designs: new Map(), styleEdits: {}, stage: null, time: 0, playing: false, size: { width: 1920, height: 1080 }, lanes: null, view: "design", tab: "look", scene: true, sceneAt: null, followed: null, canvas: null, audio: null, drag: null, server: null, player: null, show: "still", offset: 0, timeLabel: null, effects: null };
try {const links=JSON.parse(localStorage.getItem("magicstudio.lyrics.links")||"{}");
  if(/^#[0-9a-f]{6}$/i.test(links.color))wordsPanel.links.color=links.color;
  for(const key of ["front","pulse","hidden"])if(typeof links[key]==="boolean")wordsPanel.links[key]=links[key];
}catch{}
// How a designed lyric item moves on its accent word (the engine's MOVES), and what each does.
const WORDS_MOVES = [["quiet", "Quiet", "Fades in and holds"], ["sleep", "Sleep", "Letters sink one by one"], ["settle", "Settle", "Drops and squashes into place"],
  ["fall", "Fall", "Drops down"], ["pop", "Pop", "A quick swell on the beat"], ["rise", "Rise", "Arcs up and away"], ["east", "East", "Slides right"],
  ["west", "West", "Slides left"], ["loop", "Loop", "Makes one loop"]];
// Lettering: [key, name, what it is for, font (the song's own, a serif or a sans), weight, italic].
const WORDS_LETTERING = [["song", "Song font", "The video's own lettering", "song", 400, false], ["song-bold", "Song bold", "The video's lettering, heavy", "song", 700, false],
  ["serif-italic", "Serif italic", "A hero word, expressive", "serif", 400, true], ["serif-bold", "Serif bold", "A hero word, strong", "serif", 700, false],
  ["sans", "Sans, quiet", "Context words, small and calm", "sans", 400, false]];
// Hand-drawn strokes that draw on in time with the line (shapes from the Pterodactyl Song's lyric scenes).
const WORDS_STROKES = [
  { name: "Arrow down", does: "A wandering arrow pointing down", view_box: "0 0 160 330", w: 130, h: 300, draw: 1.3, d: "M 73 13 C 27 67 104 129 66 196 Q 47 231 61 289 M 30 264 L 61 294 L 87 260" },
  { name: "Arrow right", does: "Travels right", view_box: "0 0 670 65", w: 520, h: 52, draw: 0.9, d: "M 15 33 Q 208 17 409 30 T 650 26 M 625 6 L 655 26 L 626 49" },
  { name: "Arrow left", does: "Travels left", view_box: "0 0 670 65", w: 520, h: 52, draw: 0.9, d: "M 655 33 Q 462 17 261 30 T 20 26 M 45 6 L 15 26 L 44 49" },
  { name: "Rising swoop", does: "A flight trail from low to high", view_box: "0 0 790 160", w: 600, h: 120, draw: 1.8, d: "M 10 145 Q 126 170 217 93 T 438 58 Q 578 29 756 7 M 730 5 L 761 5 L 746 29" },
  { name: "Loop", does: "One playful loop", view_box: "0 0 620 110", w: 500, h: 90, draw: 2.1, d: "M 15 20 C 110 95 425 95 455 40 C 485 -4 350 -3 352 37 C 355 95 515 85 602 50 M 579 33 L 608 49 L 586 73" },
  { name: "Underline", does: "Comes to rest under a word", view_box: "0 0 280 30", w: 280, h: 30, draw: 0.55, d: "M 5 14 Q 61 5 125 13 T 269 10" },
  { name: "Hill", does: "A rounded hill beneath the words", view_box: "0 0 410 95", w: 410, h: 95, draw: 1.7, d: "M 7 77 Q 73 72 118 32 Q 171 -8 228 31 Q 279 75 400 76" }];
const wordsCommand = (command, params = {}) => invoke("engine_command", { id: command, params });

const wordsSongPath = () => state.data?.status?.stages?.find((s) => s.id === "song")?.inputs?.[0]?.path;
const wordsCurrent = () => wordsPanel.read?.document?.animations?.find((a) => a.animation_id === wordsPanel.selected);
// The same printed section in the finished video: its span on the song clock and its lyric clips.
const wordsStudioSection = (anim) => wordsPanel.studio?.sections?.find((s) => s.section === anim?.scope?.section && s.occurrence === anim?.scope?.occurrence);
const wordsClip = (id, anim = wordsCurrent()) => wordsStudioSection(anim)?.clips.find((c) => c.word_id === id);
const wordsClock = (t) => `${Math.floor(t / 60)}:${(t % 60).toFixed(1).padStart(4, "0")}`;

function wordsLabel(anim: any) { return wordsDesignController.label(anim); }
const wordsEnsureKinetic = (a: any) => (a.kinetic ??= {
  layout: "karaoke", entrance: "pop", acts: {}, draws: [], overlay: "none", band: "top", seed: 1,
  style: { font: "Fraunces", hero_font: "Fraunces", weight: 600, hero_weight: 700, hero_italic: true, size: 58, hero_size: 92, color: "#FFF6E3", hero_color: "#FFD27D", outline: "#143042" }
});
async function wordsBootstrapStudio(studio: any) { return wordsDesignController.bootstrapStudio(studio); }
async function wordsOpen() { return wordsDesignController.open(); }
const wordsFontsLoaded = new Set<string>();
async function wordsLoadFonts(fonts: any[]) { return wordsDesignController.loadFonts(fonts); }
const wordsSongFont = (anim: any) => anim.style?.font || wordsPanel.read?.fonts?.[0]?.family || "sans-serif";
function wordsSelect(id: string) { return wordsDesignController.select(id); }
function wordsEffects(anim: any) { return wordsDesignController.effects(anim); }
async function wordsApplyRegistry(name: string | null) { return wordsDesignController.applyRegistry(name); }
const wordsAt = (w: any) => wordsClip(w.word_id, wordsPanel.read?.document?.animations?.find((a: any) => a.words.includes(w)) || wordsCurrent())?.start ?? +w.start;
function wordsSpan(anim: any) { return wordsDesignController.span(anim); }
function wordsLines(anim: any) { return wordsDesignController.lines(anim); }
function wordsPlace(node: any, at: number, len?: number) { return wordsDesignController.place(node, at, len); }
function wordsRows(animations: any[]) { return wordsDesignController.rows(animations); }
const wordsBand = (a: any, tag: string, extra: Record<string, any> = {}, nextStart = Infinity) => {
  const span = wordsSpan(a), end = Math.min(span.end, nextStart);
  return wordsPlace(el(tag, { class: `words-band${a.animation_id === wordsPanel.selected ? " active" : ""}`, "data-kind": a.scope?.section === "chorus" ? "chorus" : "verse", ...extra },
    el("strong", {}, wordsLabel(a)), el("span", { class: "meta" }, wordsClock(span.start))), span.start, Math.max(0.2, end - span.start));
};
const wordsLineWords = (anim: any, lineId: string) => anim.words.filter((w: any) => w.line_id === lineId);
const wordsWorld = () => [wordsPanel.size?.width || 1920, wordsPanel.size?.height || 1080];
function wordsLineSpan(anim: any, lineId: string) { return wordsDesignController.lineSpan(anim, lineId); }
function wordsLineWindow(anim: any, lineId = wordsPanel.line) { return wordsDesignController.lineWindow(anim, lineId); }
function wordsPreviewLine(anim = wordsCurrent(), lineId = wordsPanel.line, play = true) { return wordsDesignController.previewLine(anim, lineId, play); }
function wordsDesign(anim: any, lineId: string) { return wordsDesignController.design(anim, lineId); }
function wordsDesignEdit(anim: any, lineId: string, change: (d: any) => void) { return wordsDesignController.designEdit(anim, lineId, change); }
function wordsLayout(anim: any, lineId: string, kind: string) { return wordsDesignController.layout(anim, lineId, kind); }
const wordsMeasurer = document.createElement("canvas").getContext("2d")!;
function wordsMeasure(anim: any, text: string, size: number, look: string) { return wordsDesignController.measure(anim, text, size, look); }
const wordsEase = { out2: (x: number) => 1 - (1 - x) ** 2, out3: (x: number) => 1 - (1 - x) ** 3, sin: (x: number) => 0.5 - 0.5 * Math.cos(Math.PI * x) };
const wordsClamp = (x: number) => Math.max(0, Math.min(1, x));

class WordsDesignController {
  label(anim: any) {
    const s = anim.scope || {};
    const name = String(s.section || "lyrics").replace(/_(\d+)$/, " $1").replace(/_/g, " ");
    return `${name[0].toUpperCase()}${name.slice(1)}${s.section === "chorus" || s.occurrence > 1 ? ` ${s.occurrence}` : ""}`;
  }

  async bootstrapStudio(studio: any) {
    if (!serverUrl() && typeof designCommand === "function") await designCommand("video.server_start").catch(() => {});
    if (studio?.sections?.length) return studio;
    await wordsCommand("words.track").catch(() => {});
    return wordsCommand("words.studio_track").catch((error: any) => ({ error: String(error).replace(/^.*?: /, "") }));
  }

  async open() {
    const view = $("view-design");
    if (!view) return;
    if (!wordsPanel.read) view.replaceChildren(el("p", { class: "panel-empty inline" }, "Loading the words…"));
    try {
      const [read, align, rawStudio, backgrounds] = await Promise.all([
        wordsCommand("words.read"),
        wordsCommand("words.align"),
        wordsCommand("words.studio_track").catch((error: any) => ({ error: String(error).replace(/^.*?: /, "") })),
        wordsCommand("storyboard.backgrounds").catch((error: any) => ({ images: [], error: String(error).replace(/^.*?: /, "") })),
      ]);
      const studio = await this.bootstrapStudio(rawStudio);
      (read.document?.animations || []).forEach(wordsEnsureKinetic);
      wordsPanel.read = read;
      wordsPanel.backgrounds = backgrounds;
      wordsPanel.align = align;
      await this.loadFonts(read.fonts || []);
      wordsPanel.studio = studio;
      if (!wordsCurrent()) wordsPanel.selected = read.document.animations[0]?.animation_id || null;
      wordsRender();
      wordsVideoRange();
    } catch (error) {
      view.replaceChildren(el("div", { class: "panel-empty boxed" },
        el("p", {}, "Words starts from the artist's lyric sheet."),
        el("p", { class: "muted" }, String(error).replace(/^.*?: /, "")),
        el("p", { class: "muted" }, "Attach the original in Project › Song › Lyrics & notes.")));
    }
  }

  async loadFonts(fonts: any[]) {
    await Promise.all(fonts.filter((f) => !wordsFontsLoaded.has(f.path)).map(async (f) => {
      wordsFontsLoaded.add(f.path);
      try { document.fonts.add(await new FontFace(f.family, `url(${JSON.stringify(convertFileSrc(f.path))})`, { weight: f.weight || "400" }).load()); }
      catch (error) { toast(`The video's font ${f.family} did not load: ${error}`); }
    }));
  }

  select(id: string) {
    if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) {
      if (!confirm("Discard unsaved lyric changes?")) return;
    }
    wordsPanel.designs.clear(); wordsPanel.styleEdits = {}; wordsPanel.line = null; wordsPanel.item = null; wordsPanel.selected = id;
    wordsRender();
    wordsVideoRange();
  }

  effects(anim: any) {
    const current = anim.effects?.text?.name;
    const registry = wordsPanel.effects?.kinds?.text?.effects || [];
    if (!wordsPanel.effects) {
      wordsPanel.effects = { loading: true };
      wordsCommand("words.effects", { kind: "text" }).catch((error: any) => ({ error: String(error).replace(/^.*?: /, "") }))
        .then((found: any) => { wordsPanel.effects = found; if (state.view === "design") wordsRender(); });
    }
    return el("div", { class: "words-effects" },
      current ? el("button", { class: "effect", "data-effect": "", onclick: () => this.applyRegistry(null) },
        el("strong", {}, `Using ${current}`), el("span", { class: "muted" }, "Click to remove this effect.")) : null,
      wordsPanel.effects?.error ? el("p", { class: "muted" }, wordsPanel.effects.error)
        : wordsPanel.effects?.loading ? el("p", { class: "muted" }, "Finding effects…") : null,
      ...registry.filter((e: any) => e.name !== current).map((e: any) => el("button", { class: "effect", "data-effect": e.name, title: (e.tags || []).join(" · "), onclick: () => this.applyRegistry(e.name) },
        el("strong", {}, e.title || e.name), el("span", { class: "muted" }, e.description || ""))));
  }

  async applyRegistry(name: string | null) {
    const anim = wordsCurrent();
    if (!anim) return;
    if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) return toast("Save or discard the lyric changes first");
    try {
      if (name) { toast(`Installing ${name}…`); await wordsCommand("words.effect_install", { name }); }
      const result = await wordsCommand("words.save", {
        animation_id: anim.animation_id, expected_source_revision: anim.source_revision,
        origin: "magicstudio-design", effects: { text: name ? { name } : null }
      });
      if (result.conflict) { toast("Someone changed this section first; reloaded the latest"); return this.open(); }
      await wordsCommand("words.compose", { animation_id: anim.animation_id });
      wordsPanel.show = name ? "edited" : "full";
      wordsPanel.stage = null;
      toast(name ? `${name} on ${this.label(anim)}` : `Effect removed from ${this.label(anim)}`);
      await this.open();
    } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
  }

  span(anim: any) {
    const video = wordsStudioSection(anim);
    return video ? { start: video.start, end: video.end } : { start: Math.min(...anim.words.map(wordsAt)), end: Math.max(...anim.words.map((w: any) => +w.end)) };
  }

  lines(anim: any) {
    const lines: Array<{ id: string; words: any[] }> = [];
    for (const w of anim.words) { if (lines.at(-1)?.id !== w.line_id) lines.push({ id: w.line_id, words: [] }); lines.at(-1)!.words.push(w); }
    return lines;
  }

  place(node: any, at: number, len?: number) {
    node.style.setProperty("--at-s", at.toFixed(3));
    if (len != null) node.style.setProperty("--len-s", Math.max(0, len).toFixed(3));
    return node;
  }

  rows(animations: any[]) {
    return animations.flatMap((a, idx) => {
      const span = this.span(a), lines = this.lines(a), nextStart = animations[idx + 1] ? this.span(animations[idx + 1]).start : Infinity;
      const secEnd = Math.min(span.end, nextStart);
      return lines.map((line, j) => { const at = wordsAt(line.words[0]); return [a, j, line, at, Math.max(0.2, (lines[j + 1] ? wordsAt(lines[j + 1].words[0]) : secEnd) - at)]; });
    });
  }

  lineSpan(anim: any, lineId: string) {
    const lines = this.lines(anim), k = Math.max(0, lines.findIndex((l) => l.id === lineId));
    const start = wordsAt(lines[k].words[0]);
    return { start, end: lines[k + 1] ? wordsAt(lines[k + 1].words[0]) : this.span(anim).end };
  }

  lineWindow(anim: any, lineId = wordsPanel.line) {
    const sec = this.span(anim), span = this.lineSpan(anim, lineId || this.lines(anim)[0]?.id);
    return { start: Math.max(sec.start, span.start - 0.15), end: Math.min(sec.end, Math.max(span.start + 0.6, span.end)) };
  }

  previewLine(anim = wordsCurrent(), lineId = wordsPanel.line, play = true) {
    if (!anim) return;
    const targetLine = lineId || this.lines(anim)[0]?.id;
    if (!targetLine) return;
    const span = this.lineSpan(anim, targetLine);
    wordsPanel.previewRange = this.lineWindow(anim, targetLine);
    wordsPanel.autoPlay = play;
    wordsPanel.time = play ? wordsPanel.previewRange.start : Math.max(span.start, span.end - 0.18);
    wordsVideoRange();
  }

  design(anim: any, lineId: string) {
    const pending = wordsPanel.designs.get(lineId);
    return pending !== undefined ? pending ?? this.layout(anim, lineId, "plain") : anim.design?.[lineId] ?? this.layout(anim, lineId, "plain");
  }

  designEdit(anim: any, lineId: string, change: (d: any) => void) {
    const design = structuredClone(this.design(anim, lineId));
    change(design);
    wordsPanel.designs.set(lineId, design);
    wordsRender();
  }

  layout(anim: any, lineId: string, kind: string) {
    const words = wordsLineWords(anim, lineId), [W, H] = wordsWorld();
    const item = (ws: any[], x: number, y: number, size: number, look = "song", motion = "quiet") => {
      const [, , , font, weight, italic] = WORDS_LETTERING.find((l) => l[0] === look)!;
      return {
        word_ids: ws.map((w) => w.word_id), x: Math.round(x), y: Math.round(y), size: Math.round(size), font, weight, italic, color: anim.style?.color || "#1b2a33",
        outline: null, rotate: 0, motion, accent: motion === "quiet" ? null : ws.at(-1).word_id
      };
    };
    const width = (ws: any[], size: number, look = "song") => this.measure(anim, ws.map((w) => w.text).join(" "), size, look);
    if (kind === "poster") {
      const hero = words.reduce((a: any, b: any) => (b.text.length > a.text.length ? b : a)), k = words.indexOf(hero), small = H * 0.04;
      return {
        items: [
          ...(k ? [item(words.slice(0, k), W * 0.45, H * 0.1, small, "sans")] : []),
          item([hero], W * 0.44, H * 0.15, H * 0.15, "serif-italic", "pop"),
          ...(k < words.length - 1 ? [item(words.slice(k + 1), W * 0.45 + width([hero], H * 0.15, "serif-italic") * 0.6, H * 0.33, small, "sans")] : [])], draws: []
      };
    }
    if (kind === "stack") {
      const size = H * 0.085;
      return { items: words.map((w: any, i: number) => item([w], W * 0.08, H * 0.12 + i * size * 1.1, size, i === words.length - 1 ? "serif-bold" : "song", i === words.length - 1 ? "settle" : "quiet")), draws: [] };
    }
    const size = H * 0.06, gap = size * 0.35, total = words.reduce((sum: number, w: any) => sum + width([w], size) + gap, -gap);
    let x = (W - total) / 2;
    return { items: words.map((w: any) => { const it = item([w], x, H * 0.82, size); x += width([w], size) + gap; return it; }), draws: [] };
  }

  measure(anim: any, text: string, size: number, look: string) {
    const [, , , font, weight, italic] = WORDS_LETTERING.find((l) => l[0] === look)!;
    wordsMeasurer.font = `${italic ? "italic " : ""}${weight} ${size}px ${font === "song" ? JSON.stringify(wordsSongFont(anim)) : font === "serif" ? "serif" : "sans-serif"}`;
    return wordsMeasurer.measureText(text).width;
  }
}

const wordsDesignController = new WordsDesignController();
(window as any).WordsDesignController = WordsDesignController;
(window as any).wordsDesignController = wordsDesignController;

function wordsItemMotionOffset(motion: string, settled: number, arc: number) { return wordsTimelineController.itemMotionOffset(motion, settled, arc); }
function wordsItemPulse(local: number, a: number) { return wordsTimelineController.itemPulse(local, a); }
function wordsBuildSleepSpans(text: string, local: number, a: number) { return wordsTimelineController.buildSleepSpans(text, local, a); }
function wordsDrawLineItem(world: any, words: any[], span: any, local: number, interactive: boolean, W: number, it: any, i: number) {
  return wordsTimelineController.drawLineItem(world, words, span, local, interactive, W, it, i);
}
function wordsDrawLine(world: any, anim: any, lineId: string, t: number, interactive: boolean) {
  return wordsTimelineController.drawLine(world, anim, lineId, t, interactive);
}
function wordsCanvas(anim: any) { return wordsTimelineController.canvas(anim); }
function wordsSceneSeek(t: number) { return wordsTimelineController.sceneSeek(t); }
function wordsCanvasDraw(t = wordsPanel.time) { return wordsTimelineController.canvasDraw(t); }
const wordsFitter = new ResizeObserver((entries) => {
  const [W, H] = wordsWorld();
  for (const { target } of entries) (target as HTMLElement).style.setProperty("--scale", String(Math.min((target as HTMLElement).clientWidth / W, (target as HTMLElement).clientHeight / H)));
});
const wordsPose = (anim: any, lineId: string) => wordsLineSpan(anim, lineId).end - 0.05;
function wordsPlay() { return wordsTimelineController.play(); }
function wordsSetChipVars(chip: any, font: string, weight: any, italic: any, color: string, scale: any) {
  return wordsTimelineController.setChipVars(chip, font, weight, italic, color, scale);
}
function wordsApplyItemChipStyle(chip: any, anim: any, it: any) { return wordsTimelineController.applyItemChipStyle(chip, anim, it); }
function wordsApplyKineticChipStyle(chip: any, kin: any, wordId: string) { return wordsTimelineController.applyKineticChipStyle(chip, kin, wordId); }
function wordsApplyChipStyle(chip: any, anim: any, w: any, it: any) { return wordsTimelineController.applyChipStyle(chip, anim, w, it); }
function wordsTogglePickedWord(w: any, sameLine: boolean) { return wordsTimelineController.togglePickedWord(w, sameLine); }
function wordsSwitchSection(id: string) { return wordsTimelineController.switchSection(id); }
function wordsChipClick(anim: any, w: any, index: number, event: any) { return wordsTimelineController.chipClick(anim, w, index, event); }
function wordsApplyPreviewWindow(range: any, pausedTime: number, ensureAt: number) { return wordsTimelineController.applyPreviewWindow(range, pausedTime, ensureAt); }
function wordsSelectLine(anim: any, line: any) { return wordsTimelineController.selectLine(anim, line); }
function wordsChip(anim: any, w: any) { return wordsTimelineController.chip(anim, w); }
function wordsPickedItems(design: any) { return wordsTimelineController.pickedItems(design); }
function wordsGaps(animations: any[]) { return wordsTimelineController.gaps(animations); }
const wordsTicks = (end: number) => Array.from({ length: Math.ceil(end / 10) }, (_, i) => wordsPlace(el("span", { class: "words-tick" }, wordsClock(i * 10)), i * 10));
function wordsTimeline(animations: any[], end: number) { return wordsTimelineController.timeline(animations, end); }
function wordsRoll(animations: any[], end: number) { return wordsTimelineController.roll(animations, end); }
function wordsStroke(stroke: any) { return wordsTimelineController.stroke(stroke); }
function wordsCueIntersects(cue: any, animationId: string, start: number, end: number, wordIds: string[]) {
  return wordsTimelineController.cueIntersects(cue, animationId, start, end, wordIds);
}
function wordsCueWindow(anim: any, cue: any, line: any) { return wordsTimelineController.cueWindow(anim, cue, line); }
function wordsPreviewCue(cue: any) { return wordsTimelineController.previewCue(cue); }
async function wordsRetimeCue(cue: any, start: number, end: number) { return wordsTimelineController.retimeCue(cue, start, end); }
function wordsCueDrag(event: any, cue: any, edge: string) { return wordsTimelineController.cueDrag(event, cue, edge); }

addEventListener("pointermove", (event) => {
  const d = wordsPanel.cueDrag;
  if (!d) return;
  const delta = (event.clientY - d.y) / d.scale;
  if (d.edge === "start") {
    d.cue.start = Math.max(0, Math.min(d.before.anchor + 600, Math.max(d.before.anchor - 600, d.before.start + delta)));
    d.cue.end = d.cue.start + d.before.end - d.before.start;
    if (d.before.draw_end != null) d.cue.draw_end = d.cue.start + d.before.draw_end - d.before.start;
  } else d.cue.end = Math.max(d.cue.start + 0.05, Math.min(d.cue.start + (d.cue.kind === "overlay" ? 600 : 30), d.before.end + delta));
  const frame = [...$("view-design").querySelectorAll(".words-graphic")].find((f: any) => f.dataset.cueId === d.cue.id);
  if (frame) wordsPlace(frame, d.cue.start, d.cue.end - d.cue.start);
  wordsArrows($("view-design"));
});
addEventListener("pointerup", () => {
  const d = wordsPanel.cueDrag;
  if (!d) return;
  wordsPanel.cueDrag = null;
  const { start, end } = d.cue; Object.assign(d.cue, d.before);
  wordsRetimeCue(d.before, start, end);
});
addEventListener("pointercancel", () => {
  const d = wordsPanel.cueDrag;
  if (!d) return;
  Object.assign(d.cue, d.before); wordsPanel.cueDrag = null; wordsRender();
});

class WordsTimelineController {
  itemMotionOffset(motion: string, settled: number, arc: number) {
    const map: Record<string, number[]> = {
      settle: [0, 6 * settled],
      fall: [0, 20 * settled],
      rise: [24 * settled, -22 * Math.sin(arc * Math.PI) - 18 * settled],
      east: [18 * settled, 0],
      west: [-18 * settled, 0],
      loop: [12 * Math.sin(arc * Math.PI * 2), 7 * (Math.cos(arc * Math.PI * 2) - 1)]
    };
    return map[motion] || [0, 0];
  }

  itemPulse(local: number, a: number) {
    if (local < a || local >= a + 0.43) return 1;
    return local < a + 0.13 ? 1 + 0.12 * (local - a) / 0.13 : 1.12 - 0.12 * (local - a - 0.13) / 0.3;
  }

  buildSleepSpans(text: string, local: number, a: number) {
    return [...text].map((ch, k) => {
      const u = wordsEase.sin(wordsClamp((local - a - k * 0.035) / 0.8)), letter = el("span", {}, ch);
      letter.style.setProperty("--ty", `${7 * u + 3 * Math.sin(k * 0.6) * Math.sin(u * Math.PI)}px`);
      letter.style.setProperty("--lr", `${-0.6 * u}deg`);
      return letter;
    });
  }

  drawLineItem(world: any, words: any[], span: any, local: number, interactive: boolean, W: number, it: any, i: number) {
    const text = it.word_ids.map((id: string) => words.find((w: any) => w.word_id === id)?.text).join(" ");
    const accentWord = words.find((w: any) => w.word_id === it.accent);
    const a = (accentWord ? wordsAt(accentWord) : span.start) - span.start;
    const settled = wordsEase.out3(wordsClamp((local - a) / 0.65)), arc = wordsClamp((local - a) / 0.75);
    const pulse = this.itemPulse(local, a);
    const [tx, ty] = this.itemMotionOffset(it.motion, settled, arc);
    const node = el("div", { class: `lyric-item${interactive && wordsPanel.item === i ? " active" : ""}`, "data-weight": String(it.weight), "data-italic": String(it.italic), "data-font": it.font || "song" });
    if (it.outline) { node.dataset.outline = ""; node.style.setProperty("--outline", it.outline); }
    const sy = it.motion === "pop" ? pulse : (it.motion === "settle" ? 1 - 0.035 * settled : 1);
    for (const [key, value] of [["--x", `${it.x}px`], ["--y", `${it.y}px`], ["--size", `${it.size}px`], ["--ink", it.color], ["--rot", `${it.rotate}deg`],
      ["--o", String(wordsEase.out2(wordsClamp((local + 0.4) / 0.4)))], ["--tx", `${tx}px`], ["--ty", `${ty}px`],
      ["--sx", String(it.motion === "pop" ? pulse : 1)], ["--sy", String(sy)]]) node.style.setProperty(key, value);
    node.append(...(it.motion === "sleep" ? this.buildSleepSpans(text, local, a) : [el("span", {}, text)]));
    if (interactive) node.onpointerdown = (event: any) => {
      event.preventDefault();
      wordsPanel.item = i; wordsPanel.drag = { i, x: event.clientX, y: event.clientY, from: [it.x, it.y], scale: world.getBoundingClientRect().width / W, moved: false };
      wordsRender();
    };
    return node;
  }

  drawLine(world: any, anim: any, lineId: string, t: number, interactive: boolean) {
    const [W, H] = wordsWorld(), design = wordsDesign(anim, lineId), span = wordsLineSpan(anim, lineId), words = wordsLineWords(anim, lineId);
    world.style.setProperty("--w", `${W}px`); world.style.setProperty("--h", `${H}px`);
    world.style.setProperty("--family", JSON.stringify(wordsSongFont(anim)));
    const local = t - span.start;
    const strokes = design.draws.map((d: any) => {
      const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("class", "lyric-draw");
      svg.setAttribute("viewBox", d.view_box);
      for (const [key, value] of [["--x", d.x], ["--y", d.y], ["--dw", d.w], ["--dh", d.h]]) svg.style.setProperty(key, `${value}px`);
      svg.innerHTML = d.paths.map((p: any) => `<path d="${p.d}" pathLength="1" fill="none" stroke="${p.stroke}" stroke-width="${p.width}" stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="1" stroke-dashoffset="${1 - wordsClamp((local - p.from) / Math.max(0.01, p.to - p.from))}"/>`).join("");
      return svg;
    });
    const items = design.items.map((it: any, i: number) => this.drawLineItem(world, words, span, local, interactive, W, it, i));
    world.replaceChildren(...strokes, ...items);
  }

  canvas(anim: any) {
    const world = el("div", { class: "lyric-world" });
    const scene = wordsPanel.scene && serverUrl() ? el("iframe", { class: "lyric-scene", title: "The scene behind the lyrics", tabindex: "-1" }) : null;
    const box = el("div", { class: "lyric-canvas", "aria-label": "Line designer" }, scene, world);
    wordsPanel.canvas = { box, world, scene };
    wordsFitter.observe(box);
    if (scene) wordsCommand("words.lyrics_view", { show: "scene" }).then((view: any) => {
      const server = new URL(serverUrl()!), project = server.hash.split("/")[1] || "hyperframes";
      scene.addEventListener("load", () => this.sceneSeek(wordsPanel.time));
      scene.setAttribute("src", `${server.origin}/api/projects/${project}/preview/comp/${view.src}`);
    }).catch((error: any) => toast(String(error).replace(/^.*?: /, "")));
    return box;
  }

  sceneSeek(t: number) {
    wordsPanel.canvas?.scene?.contentWindow?.postMessage({ source: "hf-parent", type: "control", action: "seek", timeSeconds: Math.max(0, t), seekMode: "commit" }, "*");
  }

  canvasDraw(t = wordsPanel.time) {
    const anim = wordsCurrent(), canvas = wordsPanel.canvas;
    if (!anim || !canvas || !wordsPanel.line) return;
    this.drawLine(canvas.world, anim, wordsPanel.line, t, true);
    if (!wordsPanel.playing || Math.abs(t - (wordsPanel.sceneAt ?? -9)) > 0.25) { this.sceneSeek(t); wordsPanel.sceneAt = t; }
  }

  play() {
    const anim = wordsCurrent();
    if (!anim || !wordsPanel.line) return;
    const path = wordsSongPath();
    if (!path) return toast("Add the song in Project › Song to hear it here");
    wordsPanel.audio ??= new Audio();
    const audio = wordsPanel.audio, span = wordsLineSpan(anim, wordsPanel.line);
    if (!audio.paused) { audio.pause(); return; }
    if (audio.getAttribute("src") !== convertFileSrc(path)) audio.src = convertFileSrc(path);
    audio.currentTime = Math.max(0, span.start - 0.4);
    audio.play();
    const tick = () => {
      if (audio.paused || audio.ended || state.view !== "design") { audio.pause(); wordsPanel.playing = false; return; }
      const t = audio.currentTime, animations = wordsPanel.read.document.animations;
      const here = animations.find((a: any) => { const s = wordsSpan(a); return t >= s.start - 0.4 && t < s.end; });
      const unsaved = wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length;
      if (here && here.animation_id !== wordsPanel.selected && !unsaved) { wordsPanel.selected = here.animation_id; wordsPanel.line = null; wordsPanel.item = null; wordsPanel.followed = `${here.animation_id}/playing`; wordsRender(); }
      const current = wordsCurrent(), line = wordsLines(current).findLast((l) => wordsAt(l.words[0]) <= t + 0.4);
      if (line && line.id !== wordsPanel.line) { wordsPanel.line = line.id; wordsPanel.item = null; wordsPanel.followed = `${current.animation_id}/${line.id}`; wordsRender(); }
      wordsPanel.time = t; wordsPanel.playing = true;
      this.canvasDraw(t); wordsNow(t);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

  setChipVars(chip: any, font: string, weight: any, italic: any, color: string, scale: any) {
    chip.dataset.weight = String(weight);
    if (italic) chip.dataset.italic = "true";
    chip.style.setProperty("--word-font", font);
    chip.style.setProperty("--word-color", color);
    chip.style.setProperty("--word-scale", String(scale));
  }

  applyItemChipStyle(chip: any, anim: any, it: any) {
    const font = it.font === "song" || !it.font ? wordsSongFont(anim) : it.font;
    const color = /^#[0-9a-f]{6}$/i.test(it.color) ? it.color : "currentColor";
    const scale = Math.min(1.7, Math.max(0.8, (it.size || 110) / 110)).toFixed(2);
    this.setChipVars(chip, font, it.weight || 400, it.italic, color, scale);
  }

  applyKineticChipStyle(chip: any, kin: any, wordId: string) {
    const st = kin.style || {}, hero = !!kin.acts?.[wordId];
    const font = (hero ? st.hero_font : null) || st.font || "sans-serif";
    const weight = (hero ? st.hero_weight : null) || st.weight || 700;
    const color = (hero ? st.hero_color : null) || st.color || "currentColor";
    const scale = hero ? Math.min(1.7, (st.hero_size || 110) / (st.size || 64)) : 1;
    if (hero) chip.dataset.hero = "true";
    this.setChipVars(chip, font, weight, hero && st.hero_italic, color, scale);
  }

  applyChipStyle(chip: any, anim: any, w: any, it: any) {
    if (it) this.applyItemChipStyle(chip, anim, it);
    if (anim.kinetic) this.applyKineticChipStyle(chip, anim.kinetic, w.word_id);
    if (wordsPanel.designs.has(w.line_id)) chip.dataset.changed = "";
  }

  togglePickedWord(w: any, sameLine: boolean) {
    if (!sameLine) return [w.word_id];
    return wordsPanel.picked.includes(w.word_id)
      ? wordsPanel.picked.filter((id: string) => id !== w.word_id)
      : [...wordsPanel.picked, w.word_id];
  }

  switchSection(id: string) {
    if (id === wordsPanel.selected) return true;
    if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) {
      if (!confirm("Discard unsaved lyric changes?")) return false;
    }
    wordsPanel.designs.clear(); wordsPanel.styleEdits = {};
    wordsPanel.selected = id;
    return true;
  }

  chipClick(anim: any, w: any, index: number, event: any) {
    event.stopPropagation();
    const sameLine = anim.animation_id === wordsPanel.selected && wordsPanel.line === w.line_id;
    if (!this.switchSection(anim.animation_id)) return;
    wordsVideo("pause"); wordsPanel.playing = false; wordsPanel.autoPlay = false;
    const nextPicked = this.togglePickedWord(w, sameLine && event.shiftKey);
    wordsPanel.picked = nextPicked.length ? nextPicked : [w.word_id];
    if (wordsPanel.mode === "timing") wordsPanel.mode = "lyrics";
    wordsPanel.cue = null; wordsPanel.shelf = "acts";
    wordsPanel.line = w.line_id; wordsPanel.item = index;
    wordsPanel.followed = `${anim.animation_id}/${w.line_id}`;
    wordsPanel.previewRange = wordsLineWindow(anim, w.line_id);
    wordsPanel.time = wordsAt(w) + 0.12;
    wordsRender(); wordsSeekSong(wordsPanel.time);
    requestAnimationFrame(() => wordsEnsureBelowVideo(wordsLineSpan(anim, w.line_id).start));
  }

  applyPreviewWindow(range: any, pausedTime: number, ensureAt: number) {
    wordsPanel.previewRange = range;
    wordsPanel.autoPlay = wordsPanel.loopLine;
    wordsPanel.time = wordsPanel.loopLine ? range.start : pausedTime;
    wordsRender();
    if (wordsPanel.loopLine) wordsVideoRange(); else wordsSeekSong(wordsPanel.time);
    requestAnimationFrame(() => wordsEnsureBelowVideo(ensureAt));
  }

  selectLine(anim: any, line: any) {
    if (!this.switchSection(anim.animation_id)) return;
    wordsVideo("pause"); wordsPanel.playing = false;
    const hero = line.words.find((w: any) => anim.kinetic?.acts?.[w.word_id]) || line.words[0];
    if (wordsPanel.mode === "timing") wordsPanel.mode = "lyrics";
    wordsPanel.line = line.id; wordsPanel.item = null; wordsPanel.cue = null;
    wordsPanel.picked = hero ? [hero.word_id] : [];
    if (!wordsPanel.shelf || wordsPanel.shelf === "cues") wordsPanel.shelf = "arrive";
    wordsPanel.followed = `${anim.animation_id}/${line.id}`;
    const span = wordsLineSpan(anim, line.id), lastWord = line.words.at(-1);
    const paused = Math.min(wordsSpan(anim).end - 0.35, span.end - 0.12, (lastWord ? wordsAt(lastWord) : span.start) + 0.18);
    this.applyPreviewWindow(wordsLineWindow(anim, line.id), paused, span.start);
  }

  chip(anim: any, w: any) {
    const design = wordsDesign(anim, w.line_id), index = design.items.findIndex((it: any) => it.word_ids.includes(w.word_id)), it = design.items[index];
    const isPicked = wordsPanel.picked.includes(w.word_id) || (!wordsPanel.picked.length && wordsPanel.item === index);
    const on = anim.animation_id === wordsPanel.selected && wordsPanel.line === w.line_id && isPicked;
    const move = it && it.motion !== "quiet" ? WORDS_MOVES.find((m) => m[0] === it.motion)?.[1] : "";
    const at = wordsAt(w), clip = wordsClip(w.word_id, anim), dur = clip?.duration ?? Math.max(0.2, (+w.end || at + 0.35) - at);
    const chip = el("button", {
      class: `word lettered${on ? " active" : ""}`, "data-word-id": w.word_id,
      "data-start": at.toFixed(3), "data-end": (at + dur).toFixed(3),
      title: `${w.text} · ${wordsClock(at)}${move ? ` · ${move}` : ""} · Click to edit word act (Shift-click adds words)`,
      onclick: (event: any) => this.chipClick(anim, w, index, event)
    }, w.text);
    this.applyChipStyle(chip, anim, w, it);
    return chip;
  }

  pickedItems(design: any) {
    const picked = design.items.flatMap((it: any, i: number) => it.word_ids.some((id: string) => wordsPanel.picked.includes(id)) ? [i] : []);
    return picked.length ? picked : wordsPanel.item == null ? [] : [wordsPanel.item];
  }

  gaps(animations: any[]) {
    const spans = animations.map(wordsSpan).sort((a, b) => a.start - b.start), gaps: number[][] = [];
    let at = 0;
    for (const s of spans) { if (s.start - at > 1.5) gaps.push([at, s.start]); at = Math.max(at, s.end); }
    return gaps.map(([a, b]) => wordsPlace(el("div", { class: "words-gap", title: `No singing from ${wordsClock(a)} to ${wordsClock(b)}` },
      el("span", {}, "Instrumental"), el("span", { class: "meta" }, `${wordsClock(a)} – ${wordsClock(b)}`)), a, b - a));
  }

  timeline(animations: any[], end: number) {
    return wordsPlace(el("div", { class: "words-tl words-roll", "aria-label": "Lyrics on the song timeline" },
      ...wordsTicks(end),
      ...this.gaps(animations),
      ...animations.map((a, idx) => wordsBand(a, "button", { "data-section": a.animation_id, onclick: () => { const first = wordsLines(a)[0]; if (first) this.selectLine(a, first); } }, animations[idx + 1] ? wordsSpan(animations[idx + 1]).start : Infinity)),
      ...wordsRows(animations).map(([a, j, line, at, len]) => wordsPlace(el("div", {
        class: `words-line${j ? "" : " first"}${a.animation_id === wordsPanel.selected && wordsPanel.line === line.id ? " active" : ""}`,
        "data-kind": a.scope?.section === "chorus" ? "chorus" : "verse", "data-animation-id": a.animation_id, "data-line-id": line.id, tabindex: "0",
        title: "Click line to focus and edit its entrance or words below the preview",
        onkeydown: (event: any) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); this.selectLine(a, line); } },
        onclick: () => this.selectLine(a, line)
      },
        ...line.words.map((w: any) => this.chip(a, w))), at, len)),
      el("span", { class: "words-tl-head" })), 0, end);
  }

  private buildRollFrame(cue: any, lane: number) {
    const thumb = (progress: number) => {
      const image = cue.kind === "drawing" ? this.stroke(cue.stroke) : wordsOptionPreview("overlays", cue.overlay);
      image.setAttribute("class", "words-cue-preview");
      if (cue.kind === "drawing") {
        const path = image.querySelector("path")!; path.setAttribute("pathLength", "1");
        path.setAttribute("stroke-dasharray", "1"); path.setAttribute("stroke-dashoffset", String(1 - progress));
      }
      return image;
    };
    const handle = (edge: string, label: string) => cue.index == null && cue.kind !== "overlay" ? null : el("button", {
      class: `words-cue-handle ${edge}`, "aria-label": `${label} ${cue.name}`, title: `${label} · drag or use ↑ ↓ (Shift for fine timing)`,
      onpointerdown: (event: any) => this.cueDrag(event, cue, edge), onclick: (event: any) => event.stopPropagation(),
      onkeydown: (event: any) => {
        if (!["ArrowUp", "ArrowDown"].includes(event.key)) return;
        event.preventDefault(); event.stopPropagation();
        if (cue.animation_id !== wordsPanel.selected) wordsSelect(cue.animation_id);
        const delta = (event.key === "ArrowUp" ? -1 : 1) * (event.shiftKey ? 0.01 : 0.1);
        const start = edge === "start" ? Math.max(0, cue.start + delta) : cue.start;
        this.retimeCue(cue, start, edge === "start" ? start + cue.end - cue.start : Math.max(start + 0.05, cue.end + delta));
      }
    }, edge === "start" ? "●" : "━");
    const frame = wordsPlace(el("div", {
      class: `words-graphic${wordsPanel.cue === cue.id ? " active" : ""}`, "data-cue-id": cue.id, role: "button", tabindex: "0",
      "aria-label": `${cue.name} at ${wordsClock(cue.start)}`, title: `${cue.name}${cue.word ? ` · “${cue.word}”` : ""} · ${wordsClock(cue.start)} – ${wordsClock(cue.end)} · Click to replay this connection`,
      onclick: () => this.previewCue(cue), onkeydown: (event: any) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); this.previewCue(cue); } }
    },
      el("div", { class: "words-cue-film" }, thumb(0.35), thumb(0.7), thumb(1)),
      handle("start", "Move"), handle("end", "Trim end of"),
      el("span", { class: "words-cue-caption" }, cue.name)), cue.start, cue.end - cue.start);
    frame.style.setProperty("--cue-lane", String(lane));
    frame.style.setProperty("--cue-ink", cue.color || "var(--accent)");
    return frame;
  }

  roll(_animations: any[], end: number) {
    const cues = wordsPanel.read.graphics || [], laneEnds: number[] = [];
    const placed = cues.map((cue: any) => {
      let lane = laneEnds.findIndex((t) => t <= cue.start);
      if (lane < 0) lane = laneEnds.length;
      laneEnds[lane] = cue.end; return { cue, lane };
    });
    const frames = placed.map(({ cue, lane }: any) => this.buildRollFrame(cue, lane));
    const roll = wordsPlace(el("div", { class: "words-tl words-roll", "aria-label": "Motion graphics on the song timeline" },
      ...wordsTicks(end),
      ...frames, !cues.length ? el("p", { class: "words-graphics-empty" }, "Pick a word, then add a drawing to show its meaning here.") : null,
      el("span", { class: "words-tl-head" })), 0, end);
    roll.style.setProperty("--cue-lanes", String(Math.max(1, laneEnds.length)));
    return roll;
  }

  stroke(stroke: any) {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg"), path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    svg.setAttribute("viewBox", stroke.view_box); svg.setAttribute("aria-hidden", "true");
    path.setAttribute("d", stroke.d); path.setAttribute("fill", "none"); path.setAttribute("stroke", "currentColor");
    path.setAttribute("stroke-width", "2"); path.setAttribute("vector-effect", "non-scaling-stroke"); path.setAttribute("stroke-linecap", "round");
    svg.appendChild(path); return svg;
  }

  cueIntersects(cue: any, animationId: string, start: number, end: number, wordIds: string[]) {
    return cue.animation_id === animationId && (cue.word_ids?.length
      ? cue.word_ids.some((id: string) => wordIds.includes(id)) : cue.start < end && cue.end > start);
  }

  cueWindow(anim: any, cue: any, line: any) {
    const sec = wordsSpan(anim);
    return { start: Math.max(sec.start, Math.min(cue.start, line.start) - 0.15), end: Math.min(sec.end, Math.max(cue.end, line.end)) };
  }

  previewCue(cue: any) {
    const anim = wordsPanel.read.document.animations.find((a: any) => a.animation_id === cue.animation_id);
    if (!anim || !this.switchSection(anim.animation_id)) return;
    wordsVideo("pause"); wordsPanel.playing = false;
    wordsPanel.line = cue.line_id || wordsLines(anim)[0]?.id;
    const fallbackWord = wordsLines(anim).find((l) => l.id === wordsPanel.line)?.words[0]?.word_id;
    wordsPanel.picked = cue.word_ids?.length ? cue.word_ids : (fallbackWord ? [fallbackWord] : []);
    if (wordsPanel.mode === "timing") wordsPanel.mode = "lyrics";
    wordsPanel.cue = cue.id;
    wordsPanel.shelf = "cues";
    if (cue.kind === "overlay") wordsPanel.topShelf = "overlay";
    wordsPanel.followed = `${anim.animation_id}/${wordsPanel.line}`;
    const range = this.cueWindow(anim, cue, wordsLineSpan(anim, wordsPanel.line));
    this.applyPreviewWindow(range, Math.min(cue.end - 0.2, (cue.draw_end ?? cue.end) - 0.05), cue.start);
  }

  async retimeCue(cue: any, start: number, end: number) {
    const anim = wordsCurrent();
    if (!anim?.kinetic || (cue.index == null && cue.kind !== "overlay")) return;
    start = Math.max(0, start);
    end = Math.max(start + 0.05, end);
    wordsPanel.previewRange = this.cueWindow(anim, { ...cue, start, end }, wordsLineSpan(anim, cue.line_id || wordsLines(anim)[0]?.id));
    wordsPanel.autoPlay = false;
    if (cue.kind === "overlay") return wordsKinetic({ overlay_offset: +(start - cue.anchor).toFixed(2), overlay_duration: +(end - start).toFixed(2) });
    const duration = Math.min((cue.draw_end ?? end) - cue.start, end - start);
    const draws = anim.kinetic.draws.map((d: any, i: number) => i === cue.index ? { ...d, offset: +(start - cue.anchor).toFixed(2), duration: +duration.toFixed(2), hold: +(end - start - duration).toFixed(2) } : d);
    await wordsKinetic({ draws });
  }

  cueDrag(event: any, cue: any, edge: string) {
    event.preventDefault(); event.stopPropagation(); wordsVideo("pause");
    if (cue.animation_id !== wordsPanel.selected) wordsSelect(cue.animation_id);
    if (cue.animation_id !== wordsPanel.selected) return;
    const view = $("view-design");
    view.setPointerCapture(event.pointerId);
    wordsPanel.cueDrag = { cue, before: { ...cue }, edge, y: event.clientY, scale: parseFloat(getComputedStyle(view).getPropertyValue("--song-s")) || 18 };
  }
}

const wordsTimelineController = new WordsTimelineController();
(window as any).WordsTimelineController = WordsTimelineController;
(window as any).wordsTimelineController = wordsTimelineController;


