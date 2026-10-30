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


function wordsNearestAnchor(time: number) { return wordsFlowController.nearestAnchor(time); }
async function wordsApplyChangedSections(sections: string[], project: string) { return wordsFlowController.applyChangedSections(sections, project); }
async function wordsRelinkCue(cue: any, target: any) { return wordsFlowController.relinkCue(cue, target); }
function wordsAnchorDrag(event: any, cue: any) { return wordsFlowController.anchorDrag(event, cue); }
addEventListener("pointermove", (event) => {
  const drag = wordsPanel.linkDrag; if (!drag) return;
  const view = $("view-design"), tl = view.querySelector("#words-lyrics .words-tl"), r = tl.getBoundingClientRect();
  const scale = parseFloat(getComputedStyle(view).getPropertyValue("--song-s")) || 18;
  drag.target = event.clientX >= r.left - 24 && event.clientX <= r.right + 36 ? wordsNearestAnchor((event.clientY - r.top) / scale) : null;
  wordsArrows(view);
});
addEventListener("pointerup", () => {
  const drag = wordsPanel.linkDrag; if (!drag) return; wordsPanel.linkDrag = null;
  if (drag.target) wordsRelinkCue(drag.cue, drag.target); else wordsArrows($("view-design"));
});
addEventListener("pointercancel", () => { if (wordsPanel.linkDrag) { wordsPanel.linkDrag = null; wordsArrows($("view-design")); } });

function wordsLinkAnchorHandle(cue: any, x1: number, y1: number) { return wordsFlowController.linkAnchorHandle(cue, x1, y1); }
function wordsCurveControlY(view: any, box: DOMRect, y1: number, y2: number, active: boolean) { return wordsFlowController.curveControlY(view, box, y1, y2, active); }
function wordsArrowNode(view: any, box: DOMRect, scale: number, cue: any, frame: any, line: any) { return wordsFlowController.arrowNode(view, box, scale, cue, frame, line); }
function wordsArrows(view: any) { return wordsFlowController.arrows(view); }
function wordsLanes(anim: any) { return wordsFlowController.lanes(anim); }
function wordsPositionArrowNotes(group: any, t: number, start: number, end: number) { return wordsFlowController.positionArrowNotes(group, t, start, end); }
function wordsPickPrimaryArrow(groups: any[], t: number) { return wordsFlowController.pickPrimaryArrow(groups, t); }
function wordsUpdateArrowGroup(group: any, primary: any, t: number) { return wordsFlowController.updateArrowGroup(group, primary, t); }
function wordsEnsureNoteLoop() { return wordsFlowController.ensureNoteLoop(); }
function wordsUpdateGraphicFrame(frame: any, t: number) { return wordsFlowController.updateGraphicFrame(frame, t); }
function wordsAnimateFlow(view: any, t: number) { return wordsFlowController.animateFlow(view, t); }
function wordsNow(t: number) { return wordsFlowController.now(t); }
function wordsRollGeometry(view: any) { return wordsFlowController.rollGeometry(view); }
function wordsFollow(t: number, smooth = false) { return wordsFlowController.follow(t, smooth); }
function wordsEnsureBelowVideo(t: number) { return wordsFlowController.ensureBelowVideo(t); }
function wordsSyncViewScroll(view: any) { return wordsFlowController.syncViewScroll(view); }

class WordsFlowController {
  private noteLoopId = 0;

  nearestAnchor(time: number) {
    let nearest: any = null;
    for (const anim of wordsPanel.read?.document?.animations || []) for (const word of anim.words) {
      const at = wordsAt(word);
      if (!Number.isFinite(at)) continue;
      if (!nearest || Math.abs(at - time) < nearest.distance) nearest = { anim, word, at, distance: Math.abs(at - time) };
    }
    return nearest;
  }

  async applyChangedSections(sections: string[], project: string) {
    for (const id of sections) {
      if (wordsPanel.read.in_video?.includes(id)) await wordsCommand("words.apply", { animation_id: id, _expected_project: project });
    }
  }

  async relinkCue(cue: any, target: any) {
    if (!target || cue.index == null) return;
    const source = wordsPanel.read.document.animations.find((a: any) => a.animation_id === cue.animation_id), project = state.project;
    if (!source) return;
    wordsVideo("pause");
    try {
      const result = await wordsCommand("words.link", {
        animation_id: source.animation_id, index: cue.index, word_id: target.word.word_id, target_animation_id: target.anim.animation_id,
        expected_source_revision: source.source_revision, expected_target_revision: target.anim.source_revision, _expected_project: project
      });
      if (result.conflict) { toast("These lyrics changed elsewhere; reloaded before moving the link"); return wordsOpen(); }
      await this.applyChangedSections(result.changed_sections, project);
      if (state.project !== project) return;
      wordsPanel.selected = result.animation_id; wordsPanel.line = target.word.line_id; wordsPanel.picked = [target.word.word_id];
      wordsPanel.cue = `${result.animation_id}-draw-${result.index}`; wordsPanel.stage = null; wordsPanel.previewRange = null; await wordsOpen();
    } catch (error) { toast(String(error).replace(/^.*?: /, "")); if (state.project === project) await wordsOpen(); }
  }

  anchorDrag(event: any, cue: any) {
    event.preventDefault(); event.stopPropagation(); wordsVideo("pause");
    const view = $("view-design"); view.setPointerCapture(event.pointerId); wordsPanel.linkDrag = { cue, target: null };
  }

  linkAnchorHandle(cue: any, x1: number, y1: number) {
    const handle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
    for (const [key, value] of Object.entries({ cx: x1, cy: y1, r: 7, class: "words-link-anchor", role: "button", tabindex: 0, "aria-label": `Relink ${cue.name} from ${cue.word || "this word"}` })) {
      handle.setAttribute(key, String(value));
    }
    handle.onpointerdown = (event) => this.anchorDrag(event, cue);
    handle.onclick = (event) => { event.stopPropagation(); wordsPreviewCue(cue); };
    handle.onkeydown = (event) => {
      if (event.key === "Enter") { event.preventDefault(); wordsPreviewCue(cue); return; }
      if (!["ArrowUp", "ArrowDown"].includes(event.key)) return;
      event.preventDefault();
      const all = wordsPanel.read.document.animations.flatMap((anim: any) => anim.words.map((word: any) => ({ anim, word, at: wordsAt(word) }))).filter((x: any) => Number.isFinite(x.at)).sort((a: any, b: any) => a.at - b.at);
      const index = all.findIndex((x: any) => x.anim.animation_id === cue.animation_id && x.word.word_id === cue.word_ids?.[0]);
      this.relinkCue(cue, all[index + (event.key === "ArrowUp" ? -1 : 1)]);
    };
    return handle;
  }

  curveControlY(view: any, box: DOMRect, y1: number, y2: number, active: boolean) {
    const preview = view.querySelector(".words-preview, .lyric-canvas");
    if (!preview || !active) return [y1, y2];
    const floor = preview.getBoundingClientRect().bottom - box.top + view.scrollTop + 18;
    return [Math.max(y1, floor), Math.max(y2, floor)];
  }

  private buildArrowNotesNode() {
    const notes = document.createElementNS("http://www.w3.org/2000/svg", "g");
    notes.setAttribute("class", "words-arrow-notes");
    notes.setAttribute("aria-hidden", "true");
    for (const glyph of ["♪", "♫", "♬"]) {
      const note = document.createElementNS("http://www.w3.org/2000/svg", "text");
      note.setAttribute("class", "words-music-note");
      note.setAttribute("text-anchor", "middle");
      note.setAttribute("dominant-baseline", "central");
      note.textContent = glyph;
      notes.appendChild(note);
    }
    return notes;
  }

  arrowNode(view: any, box: DOMRect, scale: number, cue: any, frame: any, line: any) {
    const start = Number(line.style.getPropertyValue("--at-s")), end = start + Number(line.style.getPropertyValue("--len-s"));
    const chips = [...line.querySelectorAll("[data-word-id]")];
    if (!wordsCueIntersects(cue, line.dataset.animationId, start, end, chips.map((c: any) => c.dataset.wordId))) return null;
    const sourceChip = chips.find((c: any) => cue.word_ids?.includes(c.dataset.wordId));
    const lineRect = line.getBoundingClientRect(), frameRect = frame.getBoundingClientRect();
    const x1 = lineRect.right - box.left + view.scrollLeft + 5, x2 = frameRect.left - box.left + view.scrollLeft;
    const timeline = line.parentElement.getBoundingClientRect();
    const target = wordsPanel.linkDrag?.cue.id === cue.id ? wordsPanel.linkDrag.target : null;
    const anchorAt = target?.at ?? cue.anchor ?? (sourceChip ? Number(sourceChip.dataset.start) : start);
    const y1 = timeline.top - box.top + view.scrollTop + anchorAt * scale;
    const fraction = (Math.max(start, cue.start) - cue.start) / Math.max(0.05, cue.end - cue.start);
    const y2 = frameRect.top - box.top + view.scrollTop + Math.min(frameRect.height - 4, Math.max(4, frameRect.height * fraction));
    const flowStart = Math.min(anchorAt, cue.start), flowEnd = Math.max(flowStart + 0.65, cue.draw_end ?? cue.end);
    const active = wordsPanel.cue ? wordsPanel.cue === cue.id : line.classList.contains("active");
    const [cy1, cy2] = this.curveControlY(view, box, y1, y2, active);
    const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    group.setAttribute("class", `words-arrow-group${active ? " active" : ""}`);
    group.setAttribute("data-cue-id", cue.id);
    group.setAttribute("data-line-id", line.dataset.lineId || "");
    group.setAttribute("data-start", flowStart.toFixed(3));
    group.setAttribute("data-end", flowEnd.toFixed(3));
    group.style.setProperty("--link-a", wordsPanel.links.color || (line.dataset.kind === "chorus" ? "var(--tone-lyrics)" : "var(--tone-transcript)"));
    group.style.setProperty("--link-b", `color-mix(in srgb, ${cue.color || "var(--accent)"} 55%, var(--text))`);
    const curve = document.createElementNS("http://www.w3.org/2000/svg", "path");
    curve.setAttribute("d", `M ${x1} ${y1} C ${(x1 + x2) / 2} ${cy1}, ${(x1 + x2) / 2} ${cy2}, ${x2 - 6} ${y2}`);
    curve.setAttribute("class", `words-arrow-curve${active ? " active" : ""}`);
    curve.setAttribute("data-kind", line.dataset.kind);
    curve.setAttribute("aria-hidden", "true");
    curve.onclick = () => wordsPreviewCue(cue);
    const head = document.createElementNS("http://www.w3.org/2000/svg", "path");
    head.setAttribute("d", `M ${x2 - 6} ${y2} l -12 -6 m 12 6 l -12 6`);
    head.setAttribute("class", `words-arrow-head${active ? " active" : ""}`);
    head.setAttribute("aria-hidden", "true");
    group.append(curve, head, this.buildArrowNotesNode());
    if (cue.index != null && wordsPanel.mode === "timing") group.appendChild(this.linkAnchorHandle(cue, x1, y1));
    return group;
  }

  arrows(view: any) {
    view.querySelector(":scope > svg.words-arrows")?.remove();
    if (wordsPanel.links.hidden) return;
    const lines = [...view.querySelectorAll(".words-tl > .words-line")], frames = [...view.querySelectorAll(".words-roll > .words-graphic")];
    if (!lines.length || !frames.length || !frames[0].offsetParent) return;
    const box = view.getBoundingClientRect(), svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("class", `words-arrows${wordsPanel.links.pulse ? " blending" : ""}${wordsPanel.links.front ? " front" : ""}`);
    svg.setAttribute("role", "group"); svg.setAttribute("aria-label", "Lyric and drawing connections");
    svg.style.setProperty("--arrows-width", `${view.scrollWidth}px`); svg.style.setProperty("--arrows-height", `${view.scrollHeight}px`);
    const scale = parseFloat(getComputedStyle(view).getPropertyValue("--song-s")) || 18;
    for (const frame of frames) {
      const cue = wordsPanel.read.graphics.find((c: any) => c.id === frame.dataset.cueId);
      if (!cue) continue;
      for (const line of lines) {
        const node = this.arrowNode(view, box, scale, cue, frame, line);
        if (node) svg.appendChild(node);
      }
    }
    view.appendChild(svg);
    this.animateFlow(view, wordsPanel.time);
  }

  lanes(anim: any) {
    if (!wordsPanel.line) return null;
    const span = wordsLineSpan(anim, wordsPanel.line), from = span.start - 0.4, length = span.end - from;
    const words = wordsLineWords(anim, wordsPanel.line), design = wordsDesign(anim, wordsPanel.line);
    const at = (t: number) => `${(100 * (t - from) / length).toFixed(3)}%`;
    const block = (text: string, start: number, duration: number, onclick: () => void) => {
      const node = el("button", { class: "lane-block", title: `${text} · ${wordsClock(start)}`, onclick }, text);
      node.style.setProperty("--start", at(start)); node.style.setProperty("--len", `${(100 * Math.max(0.15, duration) / length).toFixed(3)}%`);
      return node;
    };
    const lane = (key: string, name: string, blocks: any[]) => el("div", { class: "lane", "data-track": key }, el("span", { class: "lane-name" }, name),
      el("div", { class: "lane-track" }, ...blocks, el("span", { class: "lane-head" })));
    const accent = (it: any) => words.find((w: any) => w.word_id === it.accent);
    wordsPanel.lanes = el("div", { class: "words-lanes", "aria-label": "Line timing" },
      lane("text", "Sung words", words.map((w: any) => { const clip = wordsClip(w.word_id); return block(w.text, wordsAt(w), clip?.duration ?? (+w.end - +w.start), () => { wordsPanel.time = wordsAt(w); wordsCanvasDraw(); this.now(wordsPanel.time); }); })),
      lane("motion", "Accents", design.items.flatMap((it: any, i: number) => it.motion === "quiet" || !accent(it) ? [] : [block(String(WORDS_MOVES.find((m) => m[0] === it.motion)?.[1] || it.motion), wordsAt(accent(it)), 0.65, () => { wordsPanel.item = i; wordsPanel.tab = "move"; wordsRender(); })])),
      lane("draw", "Drawings", design.draws.flatMap((d: any) => d.paths.map((p: any) => block(d.name, span.start + p.from, p.to - p.from, () => { wordsPanel.tab = "draw"; wordsRender(); })))));
    return wordsPanel.lanes;
  }

  positionArrowNotes(group: any, t: number, start: number, end: number) {
    const curve = group.querySelector(".words-arrow-curve");
    if (!curve || typeof curve.getTotalLength !== "function") return;
    const len = curve.getTotalLength();
    if (!len) return;
    const span = Math.max(0.4, end - start);
    const progress = wordsPanel.playing ? wordsClamp((t - start) / span) : (performance.now() / 2600) % 1;
    const notes = group.querySelectorAll(".words-music-note");
    notes.forEach((note: any, idx: number) => {
      const u = (progress * 1.15 + idx * 0.28) % 1;
      const pt = curve.getPointAtLength(u * len);
      const bob = Math.sin((u * 2.5 + idx * 0.4) * Math.PI * 2) * 5;
      const fade = u < 0.12 ? u / 0.12 : u > 0.88 ? (1 - u) / 0.12 : 1;
      note.setAttribute("transform", `translate(${pt.x.toFixed(1)}, ${(pt.y - 4 + bob).toFixed(1)})`);
      note.setAttribute("opacity", fade.toFixed(2));
    });
  }

  pickPrimaryArrow(groups: any[], t: number) {
    if (!wordsPanel.playing) {
      const picked = groups.find((g) => (wordsPanel.cue ? g.dataset.cueId === wordsPanel.cue : g.dataset.lineId === wordsPanel.line));
      if (picked) return picked;
    }
    const candidates = groups.filter((g) => {
      const s = Number(g.dataset.start), e = Number(g.dataset.end);
      return Number.isFinite(s) && t >= s - 0.12 && t <= e + 0.25;
    });
    if (candidates.length) {
      return candidates.reduce((best, g) => (Number(g.dataset.start) >= Number(best.dataset.start) ? g : best));
    }
    return groups.find((g) => g.dataset.lineId === wordsPanel.line) || null;
  }

  updateArrowGroup(group: any, primary: any, t: number) {
    const s = Number(group.dataset.start), e = Number(group.dataset.end);
    const isPrimary = group === primary;
    const flowing = isPrimary;
    const past = !isPrimary && Number.isFinite(e) && t > e + 0.2;
    group.classList.toggle("flowing", flowing);
    group.classList.toggle("primary", isPrimary);
    group.classList.toggle("past", past);
    group.classList.toggle("upcoming", !isPrimary && !past);
    for (const p of group.querySelectorAll("path")) p.classList.toggle("active", isPrimary);
    if (flowing) this.positionArrowNotes(group, t, s, e);
  }

  ensureNoteLoop() {
    if (this.noteLoopId) return;
    const step = () => {
      const view = $("view-design");
      const primary = view?.classList.contains("active") ? view.querySelector("svg.words-arrows .words-arrow-group.primary") : null;
      if (!primary) { this.noteLoopId = 0; return; }
      if (!wordsPanel.playing) {
        this.positionArrowNotes(primary, wordsPanel.time, Number(primary.dataset.start) || 0, Number(primary.dataset.end) || 1);
      }
      this.noteLoopId = requestAnimationFrame(step);
    };
    this.noteLoopId = requestAnimationFrame(step);
  }

  updateGraphicFrame(frame: any, t: number) {
    const cue = wordsPanel.read?.graphics?.find((c: any) => c.id === frame.dataset.cueId);
    const active = !!cue && t >= cue.start - 0.05 && t <= cue.end + 0.1;
    const past = !!cue && t > cue.end + 0.25;
    frame.classList.toggle("playing", active);
    frame.classList.toggle("past", past && !active);
    if (cue?.kind === "drawing") {
      const drawSpan = Math.max(0.15, (cue.draw_end ?? cue.end) - cue.start);
      const u = active ? wordsClamp((t - cue.start) / drawSpan) : (past ? 1 : 0);
      frame.querySelectorAll(".words-cue-preview path").forEach((path: any, i: number) => {
        const reset = active ? (1 - wordsClamp(u * 0.75 + (i + 1) * 0.25)).toFixed(3) : String(+(1 - [0.35, 0.7, 1][i] * (past ? 1 : 1)).toFixed(3));
        path.setAttribute("stroke-dashoffset", reset);
      });
    }
  }

  animateFlow(view: any, t: number) {
    if (!view) return;
    for (const line of view.querySelectorAll(".words-tl > .words-line")) {
      const s = Number(line.style.getPropertyValue("--at-s")), e = s + Number(line.style.getPropertyValue("--len-s"));
      const singing = t >= s - 0.1 && t <= e + 0.15;
      line.classList.toggle("singing-line", singing);
      line.classList.toggle("past", t > e + 0.2 && !singing);
    }
    for (const chip of view.querySelectorAll(".words-line > .word.lettered")) {
      const s = Number(chip.dataset.start), e = Number(chip.dataset.end);
      chip.classList.toggle("singing", Number.isFinite(s) && t >= s - 0.05 && t <= e + 0.18);
    }
    const groups = [...view.querySelectorAll("svg.words-arrows .words-arrow-group")];
    const primary = this.pickPrimaryArrow(groups, t);
    for (const group of groups) this.updateArrowGroup(group, primary, t);
    for (const frame of view.querySelectorAll(".words-roll > .words-graphic")) this.updateGraphicFrame(frame, t);
    if (primary) this.ensureNoteLoop();
  }

  now(t: number) {
    const view = $("view-design"), anim = wordsCurrent();
    view?.style.setProperty("--now-s", t.toFixed(3));
    if (anim && wordsPanel.line && wordsPanel.lanes) {
      const span = wordsLineSpan(anim, wordsPanel.line), from = span.start - 0.4;
      wordsPanel.lanes.style.setProperty("--at", `${(100 * wordsClamp((t - from) / (span.end - from))).toFixed(3)}%`);
    }
    this.animateFlow(view, t);
    if (wordsPanel.playing) this.follow(t);
  }

  rollGeometry(view: any) {
    const tl = view?.querySelector(".words-tl"), eye = view?.querySelector(".words-preview, .lyric-canvas");
    if (!tl || !eye || tl.clientHeight <= 0) return null;
    const seconds = Number(tl.style.getPropertyValue("--len-s")) || wordsSongSpan().end;
    const box = view.getBoundingClientRect(), eyeY = Math.max(120, eye.getBoundingClientRect().bottom - box.top + 18);
    view.style.setProperty("--words-playhead-y", `${Math.round(eyeY)}px`);
    return { tl, seconds, eyeY, tlTop: tl.getBoundingClientRect().top - box.top + view.scrollTop };
  }

  follow(t: number, smooth = false) {
    const view = $("view-design"), g = this.rollGeometry(view);
    if (!g) return;
    const target = g.tlTop + g.tl.clientHeight * t / g.seconds - g.eyeY;
    wordsPanel.programmaticScrollUntil = performance.now() + (smooth ? 450 : 90);
    view.scrollTo({ top: Math.max(0, target), behavior: smooth ? "smooth" : "instant" });
  }

  ensureBelowVideo(t: number) {
    this.follow(t, true);
  }

  syncViewScroll(view: any) {
    this.arrows(view);
    const now = performance.now();
    if (now < (wordsPanel.programmaticScrollUntil || 0) && now >= (wordsPanel.userWheelUntil || 0)) return;
    const g = this.rollGeometry(view);
    if (!g) return;
    const t = Math.max(0, Math.min(wordsSongSpan().end, ((view.scrollTop + g.eyeY - g.tlTop) / g.tl.clientHeight) * g.seconds));
    if (wordsPanel.playing) { wordsVideo("pause"); wordsPanel.playing = false; }
    wordsPanel.previewRange = null; wordsPanel.autoPlay = false;
    wordsPanel.userScrollingUntil = now + 220;
    wordsSeekSong(t);
    wordsFollowLivePlayback(t);
    this.now(t);
    wordsCanvasDraw(t);
  }
}

const wordsFlowController = new WordsFlowController();
(window as any).WordsFlowController = WordsFlowController;
(window as any).wordsFlowController = wordsFlowController;

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
// Tiny diagrams explain the choice before it is applied; motion plays only on hover/focus.
function wordsOptionPreview(kind: string, key: string) {
  return wordsStageController.optionPreview(kind, key);
}

function wordsBuildChoiceCard(kind: string, key: string, item: any, on: boolean, onclick: () => void) {
  return wordsStageController.buildChoiceCard(kind, key, item, on, onclick);
}

function wordsMeaningShelf(anim: any, lib: any, k: any, picked: string[], text: (v: any) => string) {
  return wordsStageController.meaningShelf(anim, lib, k, picked, text);
}

function wordsToggleStroke(k: any, lib: any, picked: string[], key: string, trackKind: string) {
  return wordsStageController.toggleStroke(k, lib, picked, key, trackKind);
}

function wordsStrokesShelf(anim: any, lib: any, k: any, picked: string[], text: (v: any) => string, trackKind: string) {
  return wordsStageController.strokesShelf(anim, lib, k, picked, text, trackKind);
}

function wordsChoiceRow(lib: any, kind: string, current: string, pick: (key: string) => void) {
  return wordsStageController.choiceRow(lib, kind, current, pick);
}

function wordsShelfTab(activeKey: string, key: string, label: string, value: string, onToggle: () => void) {
  return wordsStageController.shelfTab(activeKey, key, label, value, onToggle);
}

function wordsSelectionBar(anim: any, lib: any, k: any, picked: string[]) {
  return wordsStageController.selectionBar(anim, lib, k, picked);
}

function wordsUpperTracks(anim: any) {
  return wordsStageController.upperTracks(anim);
}

function wordsLayers(anim: any) {
  return wordsStageController.layers(anim);
}

async function wordsApply(use: boolean) {
  return wordsStageController.apply(use);
}

async function wordsFlushPendingEdits(anim: any) {
  return wordsStageController.flushPendingEdits(anim);
}

async function wordsSave(openStudio = false) {
  return wordsStageController.save(openStudio);
}

class WordsStageController {
  private renderLayoutPreview(add: (tag: string, attrs: Record<string, any>, text?: string) => SVGElement, key: string) {
    const places: Record<string, number[][]> = {
      karaoke: [[36, 35], [68, 35], [112, 35]], stack: [[60, 24], [99, 24], [80, 44]],
      cascade: [[28, 19], [73, 34], [122, 49]], scatter: [[32, 26, -9], [80, 44, 7], [126, 23, -5]],
      push: [[36, 35], [70, 35], [116, 35]], giant: [[27, 18], [60, 18], [87, 48]]
    };
    (places[key] || places.karaoke).forEach(([x, y, angle], i) => add("text", {
      x, y, "text-anchor": "middle",
      class: i === 2 ? "hero" : "", "font-size": key === "giant" && i === 2 ? 29 : 12,
      transform: `rotate(${angle || 0} ${x} ${y})`
    }, ["Sing", "your", "song"][i]));
    if (key === "push") add("path", { d: "M 17 22 V 12 H 30 M 130 12 H 143 V 22 M 17 42 V 52 H 30 M 130 52 H 143 V 42", class: "cue" });
  }

  private renderOverlayPreview(add: (tag: string, attrs: Record<string, any>, text?: string) => SVGElement, key: string) {
    add("path", { d: "M 0 43 Q 20 31 40 43 T 80 43 T 120 43 T 160 43 M 0 54 Q 20 42 40 54 T 80 54 T 120 54 T 160 54", class: "scene" });
    if (key === "godrays") for (const x of [30, 65, 100, 135]) add("path", { d: `M 20 0 L ${x} 64`, class: "light" });
    if (key === "lens-flare") for (const [x, y, r] of [[37, 18, 12], [73, 30, 7], [112, 44, 4]]) add("circle", { cx: x, cy: y, r, class: "light" });
    if (key === "ripples") for (const r of [10, 21, 32]) add("ellipse", { cx: 80, cy: 32, rx: r * 1.7, ry: r * 0.5, class: "cue" });
    if (["snow", "starfield", "paper-grain", "sparkle-lines"].includes(key)) {
      for (let i = 0; i < 18; i++) {
        const x = (i * 47 + 13) % 160, y = (i * 23 + 9) % 60;
        if (key === "sparkle-lines") add("path", { d: `M ${x - 3} ${y} h 6 M ${x} ${y - 3} v 6`, class: "cue" });
        else add("circle", { cx: x, cy: y, r: key === "paper-grain" ? 0.7 : key === "snow" ? 2 : 1, class: "speck" });
      }
    }
  }

  optionPreview(kind: string, key: string) {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 160 64"); svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("class", "words-option-preview"); svg.setAttribute("data-kind", kind); svg.setAttribute("data-choice", key);
    const add = (tag: string, attrs: Record<string, any>, text?: string) => {
      const node = document.createElementNS("http://www.w3.org/2000/svg", tag);
      for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
      if (text) node.textContent = text;
      svg.appendChild(node);
      return node;
    };
    if (kind === "layouts") {
      this.renderLayoutPreview(add, key);
    } else if (kind === "entrances") {
      add("text", { x: 80, y: 40, "text-anchor": "middle", "font-size": 25, class: "preview-word" }, "Sing");
      const cues: Record<string, string> = {
        "fade-up": "M 80 59 V 47 m -4 4 4 -4 4 4", slide: "M 10 34 H 43 m -5 -4 5 4 -5 4",
        "drop-in": "M 80 5 V 17 m -4 -4 4 4 4 -4", pop: "M 38 18 l 7 5 M 122 18 l -7 5 M 38 49 l 7 -5 M 122 49 l -7 -5",
        typewriter: "M 112 20 V 43", "blur-in": "M 37 24 V 40 M 31 27 V 37 M 123 24 V 40 M 129 27 V 37"
      };
      add("path", { d: cues[key] || "", class: "cue" });
    } else {
      this.renderOverlayPreview(add, key);
    }
    return svg;
  }

  buildChoiceCard(kind: string, key: string, item: any, on: boolean, onclick: () => void) {
    const card = wordsCard(on, item.name, item.does, onclick);
    if (["layouts", "entrances", "overlays"].includes(kind)) {
      card.classList.add("words-diagram-card");
      card.prepend(this.optionPreview(kind, key));
    }
    if (kind === "letterings") {
      card.classList.add("words-lettering-card");
      const s = item.style, sample = el("span", { class: "words-lettering-sample", "aria-hidden": "true", "data-weight": String(s.weight || 700), "data-italic": s.hero_italic ? "true" : undefined }, "Sing ", el("span", {}, item.name));
      sample.style.setProperty("--sample-font", `"${s.font}", sans-serif`);
      sample.style.setProperty("--sample-color", s.color);
      sample.style.setProperty("--sample-outline", s.outline || "transparent");
      sample.style.setProperty("--hero-font", `"${s.hero_font || s.font}", sans-serif`);
      sample.style.setProperty("--hero-color", s.hero_color);
      card.replaceChildren(sample);
    }
    return card;
  }

  meaningShelf(anim: any, lib: any, k: any, picked: string[], text: (v: any) => string) {
    const actsFor = (act: string | null) => {
      const acts = { ...k.acts };
      for (const id of picked) { if (act) acts[id] = act; else delete acts[id]; }
      return acts;
    };
    const groups: any[] = Object.values(Object.entries(k.acts || {}).reduce((acc: Record<string, any>, [id, act]) => {
      const key = `${text(id).toLowerCase().replace(/[^a-z]/g, "")}|${act}`;
      (acc[key] ??= { ids: [], act, word: text(id).replace(/[^\p{L}'-]/gu, "") }).ids.push(id);
      return acc;
    }, {}));
    return [
      el("div", { class: "words-meaning" }, ...groups.map((g) => el("button", {
        class: `chip${g.ids.some((id: string) => picked.includes(id)) ? " active" : ""}`,
        title: "Pick every time this word is sung",
        onclick: () => { wordsPanel.picked = g.ids; wordsPanel.line = anim.words.find((w: any) => w.word_id === g.ids[0])?.line_id; wordsRender(); }
      }, `${g.word}${g.ids.length > 1 ? ` ×${g.ids.length}` : ""} · ${lib.acts?.[g.act]?.name || g.act}`))),
      picked.length ? el("div", { class: "words-effects" },
        wordsCard(false, "No act", "Arrives like the other words", () => wordsKinetic({ acts: actsFor(null) })),
        ...Object.entries((lib.acts || {}) as Record<string, any>).map(([key, item]) => wordsCard(picked.every((id) => k.acts?.[id] === key), item.name, item.does, () => wordsKinetic({ acts: actsFor(key) }))))
        : el("p", { class: "muted" }, "Click a word in the lyrics on the left to give it an act.")
    ];
  }

  toggleStroke(k: any, lib: any, picked: string[], key: string, trackKind: string) {
    const existing = k.draws || [];
    const allHave = picked.every((id) => existing.some((d: any) => d.word_id === id && d.stroke === key));
    if (allHave) return existing.filter((d: any) => !(picked.includes(d.word_id) && d.stroke === key));
    const kept = existing.filter((d: any) => !(picked.includes(d.word_id) && !d.drawing && (d.track || lib.strokes?.[d.stroke]?.track || "graphics") === trackKind));
    return [...kept, ...picked.map((word_id) => ({ word_id, stroke: key, track: trackKind, place: key === "down" ? "beside" : "under" }))];
  }

  strokesShelf(anim: any, lib: any, k: any, picked: string[], text: (v: any) => string, trackKind: string) {
    return [
      el("div", { class: "words-meaning" }, ...(k.draws || []).map((d: any, i: number) => ({ ...d, index: i })).filter((d: any) => (d.track || lib.strokes?.[d.stroke]?.track || "graphics") === trackKind).map((d: any) => el("button", {
        class: "chip",
        title: `Edit ${d.drawing?.name || lib.strokes?.[d.stroke]?.name || d.stroke} for ${text(d.word_id)}`,
        "aria-label": `Edit drawing for ${text(d.word_id)}`,
        onclick: () => { wordsPanel.cue = `${anim.animation_id}-draw-${d.index}`; wordsPanel.picked = [d.word_id]; wordsRender(); }
      }, `${text(d.word_id)} · ${d.drawing?.name || lib.strokes?.[d.stroke]?.name || d.stroke}`))),
      picked.length ? el("div", { class: "words-effects" }, ...Object.entries((lib.strokes || {}) as Record<string, any>).filter(([, item]) => (item.track || "graphics") === trackKind).map(([key, item]) => {
        const card = wordsCard(picked.every((id) => k.draws?.some((d: any) => d.word_id === id && d.stroke === key)), item.name, item.does,
          () => wordsKinetic({ draws: this.toggleStroke(k, lib, picked, key, trackKind) }));
        const svg = wordsStroke(item); svg.setAttribute("class", "words-stroke-sample"); card.prepend(svg); return card;
      })) : null
    ];
  }

  choiceRow(lib: any, kind: string, current: string, pick: (key: string) => void) {
    return el("div", { class: `words-effects words-effects-${kind}` },
      ...Object.entries(lib[kind] || {}).map(([key, item]) => this.buildChoiceCard(kind, key, item, current === key, () => pick(key))));
  }

  shelfTab(activeKey: string, key: string, label: string, value: string, onToggle: () => void) {
    const on = activeKey === key;
    return el("button", {
      id: `words-track-${key}`,
      class: `chip words-shelf-tab${on ? " active" : ""}`,
      "aria-pressed": String(on),
      title: on ? `Hide ${label} track` : `Open ${label} track`,
      onclick: onToggle
    }, el("span", { class: "muted" }, `${label}: `), el("strong", {}, value));
  }

  private buildWordSelectionChip(anim: any, line: any, lib: any, k: any, picked: string[], w: any) {
    const act = k.acts?.[w.word_id], actName = act ? (lib.acts?.[act]?.name || act) : null;
    const on = picked.includes(w.word_id);
    return el("button", {
      class: `chip words-line-word${on ? " active" : ""}${act ? " has-act" : ""}`,
      "data-word-id": w.word_id,
      title: `${w.text} (${wordsClock(wordsAt(w))}) · Click to edit word act`,
      onclick: () => {
        wordsPanel.cue = null; wordsPanel.picked = [w.word_id]; wordsPanel.shelf = "acts";
        wordsPanel.followed = `${anim.animation_id}/${line.id}`;
        wordsPanel.time = wordsAt(w) + 0.12;
        wordsRender(); wordsSeekSong(wordsPanel.time);
      }
    }, actName ? `${w.text} · ${actName}` : w.text);
  }

  selectionBar(anim: any, lib: any, k: any, picked: string[]) {
    const lines = wordsLines(anim), lineIdx = Math.max(0, lines.findIndex((l) => l.id === wordsPanel.line));
    const line = lines[lineIdx] || lines[0];
    if (!line) return null;
    const span = wordsLineSpan(anim, line.id);
    const cue = wordsPanel.cue ? wordsPanel.read.graphics?.find((c: any) => c.id === wordsPanel.cue) : null;
    const badge = cue ? `Cue: ${cue.name}` : `${wordsLabel(anim)} · Line ${lineIdx + 1}`;
    const allPicked = line.words.length > 1 && line.words.every((w: any) => picked.includes(w.word_id));
    return el("div", { class: "words-selection-bar", role: "region", "aria-label": "Selected line and words" },
      el("div", { class: "words-selection-meta" },
        el("strong", { class: "words-selection-badge" }, badge),
        el("span", { class: "meta" }, `${wordsClock(span.start)} – ${wordsClock(span.end)}`)),
      el("div", { class: "words-selection-words" },
        ...line.words.map((w: any) => this.buildWordSelectionChip(anim, line, lib, k, picked, w)),
        line.words.length > 1 ? el("button", {
          class: `chip words-line-word${allPicked ? " active" : ""}`,
          title: "Select all words in this line",
          onclick: () => {
            wordsPanel.cue = null; wordsPanel.picked = line.words.map((w: any) => w.word_id); wordsPanel.shelf = "acts";
            wordsPanel.followed = `${anim.animation_id}/${line.id}`; wordsRender();
          }
        }, "All line") : null),
      el("div", { class: "words-selection-actions" },
        el("button", {
          id: "words-replay-line",
          title: "Play just this line's animation",
          onclick: () => wordsPreviewLine(anim, line.id, true)
        }, "▶ Replay line"),
        el("button", {
          id: "words-loop-line",
          "aria-pressed": String(wordsPanel.loopLine),
          title: "Loop this line continuously while trying animations",
          onclick: () => {
            wordsPanel.loopLine = !wordsPanel.loopLine;
            if (wordsPanel.loopLine) wordsPreviewLine(anim, line.id, true);
            else wordsVideo("pause");
            wordsRender();
          }
        }, wordsPanel.loopLine ? "⟳ Looping" : "⟳ Loop")));
  }

  upperTracks(anim: any) {
    const lib: any = wordsPanel.read?.kinetic_library || {}, k = anim?.kinetic;
    if (!k) return wordsBackdropPicker(anim);
    const curLettering = Object.entries((lib.letterings || {}) as Record<string, any>).find(([, l]) => Object.entries(l.style).every(([key, value]) => k.style?.[key] === value))?.[0] || "";
    const top = wordsPanel.topShelf;
    const pickTop = (key: string) => () => { wordsPanel.topShelf = wordsPanel.topShelf === key ? null : key; wordsRender(); };
    const sceneName = wordsPanel.show === "stage" || !anim.background_asset ? "Video" : (wordsAvailableBackdrops().find((b) => b.id === anim.background_asset)?.name || anim.background_asset);
    return el("section", { class: "words-layers words-upper-tracks", "aria-label": `${wordsLabel(anim)} scene and look tracks` },
      el("div", { class: "words-shelf-bar words-upper-bar", role: "toolbar", "aria-label": "Scene and look tracks" },
        this.shelfTab(top, "scene", "Scene", sceneName, pickTop("scene")),
        this.shelfTab(top, "lettering", "Style", lib.letterings?.[curLettering]?.name || "Custom", pickTop("lettering")),
        this.shelfTab(top, "layout", "Layout", lib.layouts?.[k.layout]?.name || k.layout, pickTop("layout")),
        this.shelfTab(top, "overlay", "Atmosphere", lib.overlays?.[k.overlay || "none"]?.name || (k.overlay || "None"), pickTop("overlay")),
        el("span", { class: "words-spacer" }),
        el("button", { class: "ghost", title: "Shuffle the lyric layout and entrance; keep drawings", onclick: () => wordsShuffle(true) }, "Shuffle")),
      top === "scene" ? wordsBackdropPicker(anim) : null,
      top === "lettering" ? this.choiceRow(lib, "letterings", curLettering, (key) => wordsKinetic({ style: lib.letterings[key].style })) : null,
      top === "layout" ? this.choiceRow(lib, "layouts", k.layout, (layout) => wordsKinetic({ layout })) : null,
      top === "overlay" ? this.choiceRow(lib, "overlays", k.overlay || "none", (overlay) => wordsKinetic({ overlay })) : null);
  }

  layers(anim: any) {
    const lib: any = wordsPanel.read.kinetic_library || {}, k = anim.kinetic;
    const picked = wordsPanel.picked.filter((id: string) => anim.words.some((w: any) => w.word_id === id));
    const text = (id: string) => anim.words.find((w: any) => w.word_id === id)?.text || id;
    const shelf = wordsPanel.shelf;
    const pickBottom = (key: string) => () => { wordsPanel.shelf = wordsPanel.shelf === key ? null : key; wordsRender(); };
    const graphicsCount = (k.draws || []).filter((d: any) => (d.track || lib.strokes?.[d.stroke]?.track || "graphics") === "graphics").length;
    const marksCount = (k.draws || []).filter((d: any) => (d.track || lib.strokes?.[d.stroke]?.track || "graphics") === "lettering").length;
    return el("section", { class: "words-layers words-lower-tracks", "aria-label": `${wordsLabel(anim)} motion and meaning tracks` },
      this.selectionBar(anim, lib, k, picked),
      el("div", { class: "words-shelf-bar words-lower-bar", role: "toolbar", "aria-label": "Motion and meaning tracks" },
        this.shelfTab(shelf, "arrive", "Arrive", lib.entrances?.[k.entrance]?.name || k.entrance, pickBottom("arrive")),
        this.shelfTab(shelf, "acts", "Meaning", `${Object.keys(k.acts || {}).length} words`, pickBottom("acts")),
        this.shelfTab(shelf, "cues", "Cues", `${graphicsCount} drawn`, pickBottom("cues")),
        this.shelfTab(shelf, "marks", "Marks", `${marksCount}`, pickBottom("marks")),
        el("span", { class: "words-spacer" }),
        el("button", { class: "ghost", "aria-label": "Draw cue on scene", onclick: () => wordsDrawCue(anim) }, "Draw cue")),
      shelf === "arrive" ? this.choiceRow(lib, "entrances", k.entrance, (entrance) => wordsKinetic({ entrance })) : null,
      shelf === "acts" ? el("div", { class: "words-shelf-body" }, ...this.meaningShelf(anim, lib, k, picked, text)) : null,
      shelf === "cues" ? el("div", { class: "words-shelf-body" }, ...this.strokesShelf(anim, lib, k, picked, text, "graphics"), wordsCueEditor(anim)) : null,
      shelf === "marks" ? el("div", { class: "words-shelf-body" }, ...this.strokesShelf(anim, lib, k, picked, text, "lettering")) : null);
  }

  async apply(use: boolean) {
    const anim = wordsCurrent();
    if (!anim) return;
    if (use && (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length)) return toast("Save the changes first");
    try {
      const result = await wordsCommand("words.apply", { animation_id: anim.animation_id, use });
      toast(use ? `${wordsLabel(anim)} now uses the designed lyrics in the video` : `${wordsLabel(anim)} has its original lyrics again`);
      if (result.changed) wordsPanel.stage = null;
      await wordsOpen();
    } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
  }

  async flushPendingEdits(anim: any) {
    const unsavedDoc = !wordsPanel.read?.saved;
    const hasEdits = unsavedDoc || wordsPanel.designs.size > 0 || Object.keys(wordsPanel.styleEdits).length > 0;
    if (!hasEdits) return true;
    const params: Record<string, any> = { animation_id: anim.animation_id, expected_source_revision: anim.source_revision, origin: "magicstudio-design" };
    if (unsavedDoc && anim.kinetic) params.kinetic = anim.kinetic;
    if (wordsPanel.designs.size) params.design = Object.fromEntries(wordsPanel.designs);
    if (Object.keys(wordsPanel.styleEdits).length) params.style = wordsPanel.styleEdits;
    const result = await wordsCommand("words.save", params);
    wordsPanel.designs.clear(); wordsPanel.styleEdits = {};
    if (result.conflict) { toast("Someone changed these lyrics first; reloaded the latest"); await wordsOpen(); return false; }
    return true;
  }

  async save(openStudio = false) {
    const anim = wordsCurrent();
    if (!anim) return;
    try {
      if (!(await this.flushPendingEdits(anim))) return;
      await wordsCommand("words.apply", { animation_id: anim.animation_id, use: true });
      if (!serverUrl() && typeof designCommand === "function") {
        await designCommand("video.server_start");
        if (typeof renderTarget === "function") renderTarget();
      }
      wordsPanel.stage = null;
      await wordsOpen();
      if (openStudio && typeof setView === "function") await setView("studio");
      toast(openStudio ? `Synced ${wordsLabel(anim)} · viewing live in HyperFrames Studio` : `Saved & synced ${wordsLabel(anim)} to HyperFrames video`);
    } catch (error) { toast(String(error)); }
  }
}

const wordsStageController = new WordsStageController();
(window as any).WordsStageController = WordsStageController;
(window as any).wordsStageController = wordsStageController;
