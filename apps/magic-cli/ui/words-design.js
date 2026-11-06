const wordsPanel = { mode: "lyrics", topShelf: "scene", shelf: "acts", inspectorTab: "word", activeBrush: null, brushSize: 8, brushColor: "#f59e0b", drawMode: false, searchQuery: "", easing: "spring-out", links: { color: null, front: false, pulse: true, hidden: false }, linkDrag: null, cue: null, cueDrag: null, previewRange: null, autoPlay: false, loopLine: false, stageDirty: false, pendingSeek: null, read: null, align: null, studio: null, selected: null, line: null, item: null, picked: [], designs: new Map, styleEdits: {}, stage: null, time: 0, playing: false, size: { width: 1920, height: 1080 }, lanes: null, view: "design", tab: "look", scene: true, sceneAt: null, followed: null, canvas: null, audio: null, drag: null, server: null, player: null, show: "still", offset: 0, timeLabel: null, effects: null };
try {
  const links = JSON.parse(localStorage.getItem("magicstudio.lyrics.links") || "{}");
  if (/^#[0-9a-f]{6}$/i.test(links.color))
    wordsPanel.links.color = links.color;
  for (const key of ["front", "pulse", "hidden"])
    if (typeof links[key] === "boolean")
      wordsPanel.links[key] = links[key];
} catch {}
const wordsTools = () => wordsPanel.read?.tools || wordsPanel.effects?.tools || {};
const wordsMoves = () => (wordsTools().moves || []).map((m) => [m.id, m.name, m.does, m.icon || "⚡"]);
const wordsLetterings = () => (wordsTools().letterings || []).map((l) => [l.id, l.name, l.does, l.font, l.weight, l.italic]);
const wordsStrokes = () => wordsTools().strokes || [];
const wordsChoreographies = () => wordsTools().choreographies || [];
const wordsEasings = () => wordsTools().easings || [];
const wordsEffectCraftTools = () => wordsTools().effectcraft || { text_presets: [], paint_presets: [], effects: [], animator_properties: [] };
const wordsFilmCraftTools = () => wordsTools().filmcraft || { effects: [], presets: [] };
const wordsCommand = (command, params = {}) => invoke("engine_command", { id: command, params });
const wordsSongPath = () => state.data?.status?.stages?.find((s) => s.id === "song")?.inputs?.[0]?.path;
const wordsCurrent = () => wordsPanel.read?.document?.animations?.find((a) => a.animation_id === wordsPanel.selected);
const wordsStudioSection = (anim) => wordsPanel.studio?.sections?.find((s) => s.section === anim?.scope?.section && s.occurrence === anim?.scope?.occurrence);
const wordsClip = (id, anim = wordsCurrent()) => wordsStudioSection(anim)?.clips.find((c) => c.word_id === id);
const wordsClock = (t) => `${Math.floor(t / 60)}:${(t % 60).toFixed(1).padStart(4, "0")}`;
function wordsLabel(anim) {
  return wordsDesignController.label(anim);
}
const wordsEnsureKinetic = (a) => a.kinetic ??= {
  layout: "karaoke",
  entrance: "pop",
  acts: {},
  draws: [],
  overlay: "none",
  band: "top",
  seed: 1,
  style: { font: "Fraunces", hero_font: "Fraunces", weight: 600, hero_weight: 700, hero_italic: true, size: 58, hero_size: 92, color: "#FFF6E3", hero_color: "#FFD27D", outline: "#143042" }
};
async function wordsBootstrapStudio(studio) {
  return wordsDesignController.bootstrapStudio(studio);
}
async function wordsOpen() {
  return wordsDesignController.open();
}
const wordsFontsLoaded = new Set;
async function wordsLoadFonts(fonts) {
  return wordsDesignController.loadFonts(fonts);
}
const wordsSongFont = (anim) => anim.style?.font || wordsPanel.read?.fonts?.[0]?.family || "sans-serif";
function wordsSelect(id) {
  return wordsDesignController.select(id);
}
function wordsEffects(anim) {
  return wordsDesignController.effects(anim);
}
async function wordsApplyRegistry(name) {
  return wordsDesignController.applyRegistry(name);
}
const wordsAt = (w) => wordsClip(w.word_id, wordsPanel.read?.document?.animations?.find((a) => a.words.includes(w)) || wordsCurrent())?.start ?? +w.start;
function wordsSpan(anim) {
  return wordsDesignController.span(anim);
}
function wordsLines(anim) {
  return wordsDesignController.lines(anim);
}
function wordsPlace(node, at, len) {
  return wordsDesignController.place(node, at, len);
}
function wordsRows(animations) {
  return wordsDesignController.rows(animations);
}
const wordsBand = (a, tag, extra = {}, nextStart = 1 / 0) => {
  const span = wordsSpan(a), end = Math.min(span.end, nextStart);
  return wordsPlace(el(tag, { class: `words-band${a.animation_id === wordsPanel.selected ? " active" : ""}`, "data-kind": a.scope?.section === "chorus" ? "chorus" : "verse", ...extra }, el("strong", {}, wordsLabel(a)), el("span", { class: "meta" }, wordsClock(span.start))), span.start, Math.max(0.2, end - span.start));
};
const wordsLineWords = (anim, lineId) => anim.words.filter((w) => w.line_id === lineId);
const wordsWorld = () => [wordsPanel.size?.width || 1920, wordsPanel.size?.height || 1080];
function wordsLineSpan(anim, lineId) {
  return wordsDesignController.lineSpan(anim, lineId);
}
function wordsLineWindow(anim, lineId = wordsPanel.line) {
  return wordsDesignController.lineWindow(anim, lineId);
}
function wordsPreviewLine(anim = wordsCurrent(), lineId = wordsPanel.line, play = true) {
  return wordsDesignController.previewLine(anim, lineId, play);
}
function wordsDesign(anim, lineId) {
  return wordsDesignController.design(anim, lineId);
}
function wordsDesignEdit(anim, lineId, change) {
  return wordsDesignController.designEdit(anim, lineId, change);
}
function wordsLayout(anim, lineId, kind) {
  return wordsDesignController.layout(anim, lineId, kind);
}
const wordsMeasurer = document.createElement("canvas").getContext("2d");
function wordsMeasure(anim, text, size, look) {
  return wordsDesignController.measure(anim, text, size, look);
}
const wordsEase = { out2: (x) => 1 - (1 - x) ** 2, out3: (x) => 1 - (1 - x) ** 3, sin: (x) => 0.5 - 0.5 * Math.cos(Math.PI * x) };
const wordsClamp = (x) => Math.max(0, Math.min(1, x));

class WordsDesignController {
  label(anim) {
    const s = anim.scope || {};
    const name = String(s.section || "lyrics").replace(/_(\d+)$/, " $1").replace(/_/g, " ");
    return `${name[0].toUpperCase()}${name.slice(1)}${s.section === "chorus" || s.occurrence > 1 ? ` ${s.occurrence}` : ""}`;
  }
  async bootstrapStudio(studio) {
    if (studio?.sections?.length)
      return studio;
    await wordsCommand("words.track").catch(() => {});
    return wordsCommand("words.studio_track", { preview_status: true }).catch((error) => ({ error: String(error).replace(/^.*?: /, "") }));
  }
  async open() {
    const view = $("view-design");
    if (!view)
      return;
    if (!wordsPanel.read)
      view.replaceChildren(el("p", { class: "panel-empty inline" }, "Loading the words…"));
    try {
      const [read, align, rawStudio, backgrounds] = await Promise.all([
        wordsCommand("words.read"),
        wordsCommand("words.align"),
        wordsCommand("words.studio_track", { preview_status: true }).catch((error) => ({ error: String(error).replace(/^.*?: /, "") })),
        wordsCommand("storyboard.backgrounds").catch((error) => ({ images: [], error: String(error).replace(/^.*?: /, "") }))
      ]);
      const studio = await this.bootstrapStudio(rawStudio);
      (read.document?.animations || []).forEach(wordsEnsureKinetic);
      wordsPanel.read = read;
      wordsPanel.backgrounds = backgrounds;
      wordsPanel.align = align;
      await this.loadFonts(read.fonts || []);
      wordsPanel.studio = studio;
      if (!wordsCurrent())
        wordsPanel.selected = read.document.animations[0]?.animation_id || null;
      wordsRender();
      wordsVideoRange();
    } catch (error) {
      view.replaceChildren(el("div", { class: "panel-empty boxed" }, el("p", {}, "Words starts from the artist's lyric sheet."), el("p", { class: "muted" }, String(error).replace(/^.*?: /, "")), el("p", { class: "muted" }, "Attach the original in Project › Song › Lyrics & notes.")));
    }
  }
  async loadFonts(fonts) {
    await Promise.all(fonts.filter((f) => !wordsFontsLoaded.has(f.path)).map(async (f) => {
      wordsFontsLoaded.add(f.path);
      try {
        document.fonts.add(await new FontFace(f.family, `url(${JSON.stringify(convertFileSrc(f.path))})`, { weight: f.weight || "400" }).load());
      } catch (error) {
        toast(`The video's font ${f.family} did not load: ${error}`);
      }
    }));
  }
  select(id) {
    if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) {
      if (!confirm("Discard unsaved lyric changes?"))
        return;
    }
    wordsPanel.designs.clear();
    wordsPanel.styleEdits = {};
    wordsPanel.line = null;
    wordsPanel.item = null;
    wordsPanel.selected = id;
    wordsRender();
    wordsVideoRange();
  }
  effects(anim) {
    const current = anim.effects?.text?.name;
    const registry = wordsPanel.effects?.kinds?.text?.effects || [];
    if (!wordsPanel.effects) {
      wordsPanel.effects = { loading: true };
      wordsCommand("words.effects", { kind: "text" }).catch((error) => ({ error: String(error).replace(/^.*?: /, "") })).then((found) => {
        wordsPanel.effects = found;
        if (state.view === "design")
          wordsRender();
      });
    }
    return el("div", { class: "words-effects" }, current ? el("button", { class: "effect", "data-effect": "", onclick: () => this.applyRegistry(null) }, el("strong", {}, `Using ${current}`), el("span", { class: "muted" }, "Click to remove this effect.")) : null, wordsPanel.effects?.error ? el("p", { class: "muted" }, wordsPanel.effects.error) : wordsPanel.effects?.loading ? el("p", { class: "muted" }, "Finding effects…") : null, ...registry.filter((e) => e.name !== current).map((e) => el("button", { class: "effect", "data-effect": e.name, title: (e.tags || []).join(" · "), onclick: () => this.applyRegistry(e.name) }, el("strong", {}, e.title || e.name), el("span", { class: "muted" }, e.description || ""))));
  }
  async applyRegistry(name) {
    const anim = wordsCurrent();
    if (!anim)
      return;
    if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length)
      return toast("Save or discard the lyric changes first");
    try {
      if (name) {
        toast(`Installing ${name}…`);
        await wordsCommand("words.effect_install", { name });
      }
      const result = await wordsCommand("words.save", {
        animation_id: anim.animation_id,
        expected_source_revision: anim.source_revision,
        origin: "magicstudio-design",
        effects: { text: name ? { name } : null }
      });
      if (result.conflict) {
        toast("Someone changed this section first; reloaded the latest");
        return this.open();
      }
      await wordsCommand("words.compose", { animation_id: anim.animation_id });
      wordsPanel.show = name ? "edited" : "full";
      wordsPanel.stage = null;
      toast(name ? `${name} on ${this.label(anim)}` : `Effect removed from ${this.label(anim)}`);
      await this.open();
    } catch (error) {
      toast(String(error).replace(/^.*?: /, ""));
    }
  }
  span(anim) {
    const video = wordsStudioSection(anim);
    return video ? { start: video.start, end: video.end } : { start: Math.min(...anim.words.map(wordsAt)), end: Math.max(...anim.words.map((w) => +w.end)) };
  }
  lines(anim) {
    const lines = [];
    for (const w of anim.words) {
      if (lines.at(-1)?.id !== w.line_id)
        lines.push({ id: w.line_id, words: [] });
      lines.at(-1).words.push(w);
    }
    return lines;
  }
  place(node, at, len) {
    node.style.setProperty("--at-s", at.toFixed(3));
    if (len != null)
      node.style.setProperty("--len-s", Math.max(0, len).toFixed(3));
    return node;
  }
  rows(animations) {
    return animations.flatMap((a, idx) => {
      const span = this.span(a), lines = this.lines(a), nextStart = animations[idx + 1] ? this.span(animations[idx + 1]).start : 1 / 0;
      const secEnd = Math.min(span.end, nextStart);
      return lines.map((line, j) => {
        const at = wordsAt(line.words[0]);
        return [a, j, line, at, Math.max(0.2, (lines[j + 1] ? wordsAt(lines[j + 1].words[0]) : secEnd) - at)];
      });
    });
  }
  lineSpan(anim, lineId) {
    const lines = this.lines(anim), k = Math.max(0, lines.findIndex((l) => l.id === lineId));
    const start = wordsAt(lines[k].words[0]);
    return { start, end: lines[k + 1] ? wordsAt(lines[k + 1].words[0]) : this.span(anim).end };
  }
  lineWindow(anim, lineId = wordsPanel.line) {
    const sec = this.span(anim), span = this.lineSpan(anim, lineId || this.lines(anim)[0]?.id);
    return { start: Math.max(sec.start, span.start - 0.15), end: Math.min(sec.end, Math.max(span.start + 0.6, span.end)) };
  }
  previewLine(anim = wordsCurrent(), lineId = wordsPanel.line, play = true) {
    if (!anim)
      return;
    const targetLine = lineId || this.lines(anim)[0]?.id;
    if (!targetLine)
      return;
    const span = this.lineSpan(anim, targetLine);
    wordsPanel.previewRange = this.lineWindow(anim, targetLine);
    wordsPanel.autoPlay = play;
    wordsPanel.time = play ? wordsPanel.previewRange.start : Math.max(span.start, span.end - 0.18);
    wordsVideoRange();
  }
  design(anim, lineId) {
    const pending = wordsPanel.designs.get(lineId);
    if (pending !== undefined)
      return pending ?? this.layout(anim, lineId, "cascade");
    const saved = anim.design?.[lineId];
    if (!saved?.items?.length)
      return this.layout(anim, lineId, "cascade");
    const isFlatTop = saved.items.every((it) => (it.y || 0) < 150 && !it.rotate);
    if (!isFlatTop)
      return saved;
    const upgraded = this.layout(anim, lineId, "cascade");
    if (saved.draws?.length)
      upgraded.draws = saved.draws.map((d) => ({ ...d, y: Math.max(d.y || 0, 650) }));
    return upgraded;
  }
  designEdit(anim, lineId, change) {
    const design = structuredClone(this.design(anim, lineId));
    change(design);
    wordsPanel.designs.set(lineId, design);
    wordsRender();
  }
  buildCascadeLayout(words, W, H, hero, item) {
    const k = Math.max(0, words.indexOf(hero));
    const before = words.slice(0, k), after = words.slice(k + 1);
    const out = [];
    before.slice(0, 4).forEach((w, i) => {
      const row = i < 2 ? 0 : 1, col = i % 2;
      const x = W * (row === 0 ? 0.16 + col * 0.22 : 0.22 + col * 0.24);
      const y = H * (row === 0 ? 0.16 + col * 0.03 : 0.31 + col * 0.03);
      out.push(item([w], x, y, H * (i === 1 ? 0.078 : 0.066), i === 1 ? "song-bold" : "song", i === 0 ? "pop" : "rise", i === 0 ? -4 : 0));
    });
    if (before.length > 4)
      out.push(item(before.slice(4), W * 0.56, H * 0.32, H * 0.06, "song", "east", 0));
    const heroItem = item([hero], W * 0.28, H * 0.48, H * 0.165, "serif-bold", "pop", -5);
    heroItem.color = "#f59e0b";
    heroItem.outline = "#2a1205";
    out.push(heroItem);
    if (after.length) {
      const mid = Math.ceil(after.length / 2);
      out.push(item(after.slice(0, mid), W * 0.46, H * 0.68, H * 0.064, "song-bold", "east", 0));
      if (after.length > mid)
        out.push(item(after.slice(mid), W * 0.64, H * 0.78, H * 0.062, "song", "settle", 2));
    }
    return { items: out, draws: [] };
  }
  layout(anim, lineId, kind) {
    const words = wordsLineWords(anim, lineId), [W, H] = wordsWorld();
    const letterings = wordsLetterings();
    const item = (ws, x, y, size, look = "song", motion = "quiet", rotate = 0) => {
      const [, , , font, weight, italic] = letterings.find((l) => l[0] === look) || ["song", "Song font", "", "song", 400, false];
      return {
        word_ids: ws.map((w) => w.word_id),
        x: Math.round(x),
        y: Math.round(y),
        size: Math.round(size),
        font,
        weight,
        italic,
        color: motion !== "quiet" ? anim.kinetic?.style?.hero_color || "#f59e0b" : anim.style?.color || "#ffffff",
        outline: motion !== "quiet" ? "#2a1205" : "#141c28",
        rotate,
        motion,
        accent: motion === "quiet" ? null : ws.at(-1).word_id
      };
    };
    const width = (ws, size, look = "song") => this.measure(anim, ws.map((w) => w.text).join(" "), size, look);
    const cleanLen = (w) => String(w?.text || "").replace(/[^a-zA-Z]/g, "").length;
    const hero = words.find((w) => anim.kinetic?.acts?.[w.word_id]) || words.reduce((a, b) => cleanLen(b) > cleanLen(a) ? b : a, words[0]);
    if (kind === "cascade")
      return this.buildCascadeLayout(words, W, H, hero, item);
    if (kind === "poster") {
      const k = words.indexOf(hero), small = H * 0.065;
      return {
        items: [
          ...k ? [item(words.slice(0, k), W * 0.26, H * 0.26, small, "song-bold", "rise", -3)] : [],
          item([hero], W * 0.3, H * 0.42, H * 0.18, "serif-italic", "pop", -5),
          ...k < words.length - 1 ? [item(words.slice(k + 1), W * 0.34, H * 0.66, small, "sans", "settle", 0)] : []
        ],
        draws: []
      };
    }
    if (kind === "stack") {
      const size = H * 0.088;
      return { items: words.slice(0, 6).map((w, i) => item([w], W * (0.24 + i * 0.04), H * 0.18 + i * size * 1.08, w === hero ? size * 1.45 : size, w === hero ? "serif-bold" : "song-bold", w === hero ? "pop" : "settle", w === hero ? -4 : 0)), draws: [] };
    }
    if (kind === "scatter") {
      const tilts = [-8, 5, -5, 6, -6, 4], heights = [0.22, 0.42, 0.26, 0.54, 0.32, 0.48], n = Math.max(1, words.length);
      return {
        items: words.map((w, i) => {
          const isHero = w === hero;
          const sz = isHero ? H * 0.165 : H * 0.072;
          return item([w], W * (0.16 + 0.64 * (i / n)), H * heights[i % heights.length], sz, isHero ? "serif-italic" : "song-bold", isHero ? "pop" : "rise", tilts[i % tilts.length]);
        }),
        draws: []
      };
    }
    const size = H * 0.072, gap = size * 0.38, total = words.reduce((sum, w) => sum + width([w], size) + gap, -gap);
    let x = Math.max(120, (W - total) / 2);
    return { items: words.map((w) => {
      const it = item([w], x, H * 0.42, size);
      x += width([w], size) + gap;
      return it;
    }), draws: [] };
  }
  expandItemWords(anim, words, it) {
    if ((it.word_ids?.length || 0) <= 1)
      return [it];
    let cx = it.x || 160;
    const gap = (it.size || 58) * 0.36, out = [];
    for (const wid of it.word_ids) {
      const w = words.find((x) => x.word_id === wid);
      if (!w)
        continue;
      out.push({ ...structuredClone(it), text: w.text, word_ids: [wid], x: Math.round(cx), accent: it.accent === wid ? wid : null });
      cx += this.measure(anim, w.text, it.size || 58, "song") + gap;
    }
    return out;
  }
  splitLineTokens(anim, lineId) {
    const words = wordsLineWords(anim, lineId);
    this.designEdit(anim, lineId, (d) => {
      d.items = (d.items || []).flatMap((it) => this.expandItemWords(anim, words, it));
    });
  }
  measure(anim, text, size, look) {
    const [, , , font, weight, italic] = wordsLetterings().find((l) => l[0] === look) || ["song", "Song font", "", "song", 400, false];
    wordsMeasurer.font = `${italic ? "italic " : ""}${weight} ${size}px ${font === "song" ? JSON.stringify(wordsSongFont(anim)) : font === "serif" ? "serif" : "sans-serif"}`;
    return wordsMeasurer.measureText(text).width;
  }
}
const wordsDesignController = new WordsDesignController;
window.WordsDesignController = WordsDesignController;
window.wordsDesignController = wordsDesignController;
function wordsItemMotionOffset(motion, settled, arc) {
  return wordsTimelineController.itemMotionOffset(motion, settled, arc);
}
function wordsItemPulse(local, a) {
  return wordsTimelineController.itemPulse(local, a);
}
function wordsBuildSleepSpans(text, local, a) {
  return wordsTimelineController.buildSleepSpans(text, local, a);
}
function wordsDrawLineItem(world, words, span, local, interactive, W, it, i) {
  return wordsTimelineController.drawLineItem(world, words, span, local, interactive, W, it, i);
}
function wordsDrawLine(world, anim, lineId, t, interactive) {
  return wordsTimelineController.drawLine(world, anim, lineId, t, interactive);
}
function wordsCanvas(anim) {
  return wordsTimelineController.canvas(anim);
}
function wordsSceneSeek(t) {
  return wordsTimelineController.sceneSeek(t);
}
function wordsCanvasDraw(t = wordsPanel.time) {
  return wordsTimelineController.canvasDraw(t);
}
const wordsFitter = new ResizeObserver((entries) => {
  const [W, H] = wordsWorld();
  for (const { target } of entries)
    target.style.setProperty("--scale", String(Math.min(target.clientWidth / W, target.clientHeight / H)));
});
const wordsPose = (anim, lineId) => wordsLineSpan(anim, lineId).end - 0.05;
function wordsPlay() {
  return wordsTimelineController.play();
}
function wordsSetChipVars(chip, font, weight, italic, color, scale) {
  return wordsTimelineController.setChipVars(chip, font, weight, italic, color, scale);
}
function wordsApplyItemChipStyle(chip, anim, it, wordId = "") {
  return wordsTimelineController.applyItemChipStyle(chip, anim, it, wordId);
}
function wordsApplyKineticChipStyle(chip, kin, wordId) {
  return wordsTimelineController.applyKineticChipStyle(chip, kin, wordId);
}
function wordsApplyChipStyle(chip, anim, w, it) {
  return wordsTimelineController.applyChipStyle(chip, anim, w, it);
}
function wordsTogglePickedWord(w, sameLine) {
  return wordsTimelineController.togglePickedWord(w, sameLine);
}
function wordsSwitchSection(id) {
  return wordsTimelineController.switchSection(id);
}
function wordsChipClick(anim, w, index, event) {
  return wordsTimelineController.chipClick(anim, w, index, event);
}
function wordsApplyPreviewWindow(range, pausedTime, ensureAt) {
  return wordsTimelineController.applyPreviewWindow(range, pausedTime, ensureAt);
}
function wordsSelectLine(anim, line) {
  return wordsTimelineController.selectLine(anim, line);
}
function wordsChip(anim, w) {
  return wordsTimelineController.chip(anim, w);
}
function wordsPickedItems(design) {
  return wordsTimelineController.pickedItems(design);
}
function wordsGaps(animations) {
  return wordsTimelineController.gaps(animations);
}
const wordsTicks = (end) => Array.from({ length: Math.ceil(end / 10) }, (_, i) => wordsPlace(el("span", { class: "words-tick" }, wordsClock(i * 10)), i * 10));
function wordsTimeline(animations, end) {
  return wordsTimelineController.timeline(animations, end);
}
function wordsRoll(animations, end) {
  return wordsTimelineController.roll(animations, end);
}
function wordsStroke(stroke) {
  return wordsTimelineController.stroke(stroke);
}
function wordsCueIntersects(cue, animationId, start, end, wordIds) {
  return wordsTimelineController.cueIntersects(cue, animationId, start, end, wordIds);
}
function wordsCueWindow(anim, cue, line) {
  return wordsTimelineController.cueWindow(anim, cue, line);
}
function wordsPreviewCue(cue) {
  return wordsTimelineController.previewCue(cue);
}
async function wordsRetimeCue(cue, start, end) {
  return wordsTimelineController.retimeCue(cue, start, end);
}
function wordsCueDrag(event, cue, edge) {
  return wordsTimelineController.cueDrag(event, cue, edge);
}
addEventListener("pointermove", (event) => {
  const d = wordsPanel.cueDrag;
  if (!d)
    return;
  const delta = (event.clientY - d.y) / d.scale;
  if (d.edge === "start") {
    d.cue.start = Math.max(0, Math.min(d.before.anchor + 600, Math.max(d.before.anchor - 600, d.before.start + delta)));
    d.cue.end = d.cue.start + d.before.end - d.before.start;
    if (d.before.draw_end != null)
      d.cue.draw_end = d.cue.start + d.before.draw_end - d.before.start;
  } else
    d.cue.end = Math.max(d.cue.start + 0.05, Math.min(d.cue.start + (d.cue.kind === "overlay" ? 600 : 30), d.before.end + delta));
  const frame = [...$("view-design").querySelectorAll(".words-graphic")].find((f) => f.dataset.cueId === d.cue.id);
  if (frame)
    wordsPlace(frame, d.cue.start, d.cue.end - d.cue.start);
  wordsArrows($("view-design"));
});
addEventListener("pointerup", () => {
  const d = wordsPanel.cueDrag;
  if (!d)
    return;
  wordsPanel.cueDrag = null;
  const { start, end } = d.cue;
  Object.assign(d.cue, d.before);
  wordsRetimeCue(d.before, start, end);
});
addEventListener("pointercancel", () => {
  const d = wordsPanel.cueDrag;
  if (!d)
    return;
  Object.assign(d.cue, d.before);
  wordsPanel.cueDrag = null;
  wordsRender();
});

class WordsTimelineController {
  itemMotionOffset(motion, settled, arc) {
    const map = {
      settle: [0, 6 * settled],
      fall: [0, 20 * settled],
      rise: [24 * settled, -22 * Math.sin(arc * Math.PI) - 18 * settled],
      east: [18 * settled, 0],
      west: [-18 * settled, 0],
      loop: [12 * Math.sin(arc * Math.PI * 2), 7 * (Math.cos(arc * Math.PI * 2) - 1)]
    };
    return map[motion] || [0, 0];
  }
  itemPulse(local, a) {
    if (local < a || local >= a + 0.43)
      return 1;
    return local < a + 0.13 ? 1 + 0.12 * (local - a) / 0.13 : 1.12 - 0.12 * (local - a - 0.13) / 0.3;
  }
  buildSleepSpans(text, local, a) {
    return [...text].map((ch, k) => {
      const u = wordsEase.sin(wordsClamp((local - a - k * 0.035) / 0.8)), letter = el("span", {}, ch);
      letter.style.setProperty("--ty", `${7 * u + 3 * Math.sin(k * 0.6) * Math.sin(u * Math.PI)}px`);
      letter.style.setProperty("--lr", `${-0.6 * u}deg`);
      return letter;
    });
  }
  itemHandles(world, W, it, i) {
    const scaleHandle = (corner) => el("span", {
      class: `lyric-handle lyric-handle-scale ${corner}`,
      "data-corner": corner,
      title: "Drag corner to scale word size",
      onpointerdown: (e) => {
        e.preventDefault();
        e.stopPropagation();
        wordsPanel.item = i;
        wordsPanel.drag = { mode: "scale", corner, i, x: e.clientX, y: e.clientY, startSize: it.size || 68, scale: world.getBoundingClientRect().width / W, moved: false };
      }
    });
    const rotHandle = el("span", {
      class: "lyric-handle lyric-handle-rot",
      title: "Drag to tilt word (-45° to +45°)",
      onpointerdown: (e) => {
        e.preventDefault();
        e.stopPropagation();
        wordsPanel.item = i;
        wordsPanel.drag = { mode: "rotate", i, x: e.clientX, y: e.clientY, startRot: it.rotate || 0, scale: world.getBoundingClientRect().width / W, moved: false };
      }
    });
    return [rotHandle, scaleHandle("nw"), scaleHandle("ne"), scaleHandle("sw"), scaleHandle("se")];
  }
  flowArrowsSvg(design, words, W, H) {
    const items = design.items || [];
    if (items.length < 2)
      return null;
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("class", "lyric-flow-arrows");
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    svg.setAttribute("aria-hidden", "true");
    const paths = [];
    for (let i = 0;i < items.length - 1; i++) {
      const a = items[i], b = items[i + 1];
      const aTxt = a.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ");
      const ax = (a.x || 0) + Math.max(60, aTxt.length * (a.size || 58) * 0.52);
      const ay = (a.y || 0) + (a.size || 58) * 0.55;
      const bx = (b.x || 0) - 12;
      const by = (b.y || 0) + (b.size || 58) * 0.55;
      const mx = (ax + bx) / 2, my = Math.min(ay, by) - 34;
      paths.push(`<circle cx="${ax.toFixed(1)}" cy="${ay.toFixed(1)}" r="5" class="lyric-flow-dot"/>`);
      paths.push(`<path d="M ${ax.toFixed(1)} ${ay.toFixed(1)} Q ${mx.toFixed(1)} ${my.toFixed(1)} ${bx.toFixed(1)} ${by.toFixed(1)}" class="lyric-flow-curve"/>`);
      paths.push(`<path d="M ${(bx - 12).toFixed(1)} ${(by - 7).toFixed(1)} L ${bx.toFixed(1)} ${by.toFixed(1)} L ${(bx - 12).toFixed(1)} ${(by + 7).toFixed(1)}" class="lyric-flow-head"/>`);
    }
    svg.innerHTML = paths.join("");
    return svg;
  }
  isItemSelected(interactive, design, it, i) {
    if (!interactive)
      return false;
    if (wordsPanel.item === i)
      return true;
    if (wordsPanel.item != null)
      return false;
    const byPicked = (it.word_ids || []).some((id) => wordsPanel.picked.includes(id));
    if (byPicked)
      return true;
    const anyPickedMatch = (design.items || []).some((x) => (x.word_ids || []).some((id) => wordsPanel.picked.includes(id)));
    if (anyPickedMatch)
      return false;
    const heroIdx = (design.items || []).findIndex((x) => (x.size || 0) >= 140 || x.motion === "pop");
    return i === (heroIdx >= 0 ? heroIdx : 0);
  }
  applyItemVars(node, it, local, settled, pulse, tx, ty) {
    if (it.outline) {
      node.dataset.outline = "";
      node.style.setProperty("--outline", it.outline);
    }
    const sy = it.motion === "pop" ? pulse : it.motion === "settle" ? 1 - 0.035 * settled : 1;
    for (const [key, value] of [
      ["--x", `${it.x}px`],
      ["--y", `${it.y}px`],
      ["--size", `${it.size}px`],
      ["--ink", it.color],
      ["--rot", `${it.rotate}deg`],
      ["--o", String(wordsEase.out2(wordsClamp((local + 0.4) / 0.4)))],
      ["--tx", `${tx}px`],
      ["--ty", `${ty}px`],
      ["--sx", String(it.motion === "pop" ? pulse : 1)],
      ["--sy", String(sy)]
    ])
      node.style.setProperty(key, value);
  }
  drawLineItem(world, words, span, local, interactive, W, it, i, design) {
    const text = (it.text || it.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ")).toUpperCase && (it.size || 0) >= 145 ? it.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ").toUpperCase() : it.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ");
    const accentWord = words.find((w) => w.word_id === it.accent);
    const a = (accentWord ? wordsAt(accentWord) : span.start) - span.start;
    const settled = wordsEase.out3(wordsClamp((local - a) / 0.65)), arc = wordsClamp((local - a) / 0.75);
    const pulse = this.itemPulse(local, a), [tx, ty] = this.itemMotionOffset(it.motion, settled, arc);
    const isSelected = this.isItemSelected(interactive, design || { items: [it] }, it, i);
    const isHero = (it.size || 0) >= 140 || it.motion === "pop";
    const node = el("div", {
      class: `lyric-item${isSelected ? " active" : ""}${isHero ? " hero-item" : ""}`,
      "data-weight": String(it.weight),
      "data-italic": String(it.italic),
      "data-font": it.font || "song"
    });
    this.applyItemVars(node, it, local, settled, pulse, tx, ty);
    node.append(...it.motion === "sleep" ? this.buildSleepSpans(text, local, a) : [el("span", {}, text)]);
    if (isSelected)
      node.append(...this.itemHandles(world, W, it, i));
    if (interactive)
      node.onpointerdown = (event) => {
        event.preventDefault();
        wordsPanel.item = i;
        wordsPanel.picked = [...it.word_ids];
        wordsPanel.drag = { mode: "move", i, x: event.clientX, y: event.clientY, from: [it.x, it.y], scale: world.getBoundingClientRect().width / W, moved: false };
        wordsRender();
      };
    return node;
  }
  drawLine(world, anim, lineId, t, interactive) {
    const [W, H] = wordsWorld(), design = wordsDesign(anim, lineId), span = wordsLineSpan(anim, lineId), words = wordsLineWords(anim, lineId);
    world.style.setProperty("--w", `${W}px`);
    world.style.setProperty("--h", `${H}px`);
    world.style.setProperty("--family", JSON.stringify(wordsSongFont(anim)));
    const local = wordsPanel.playing || !interactive ? t - span.start : Math.max(0.55, span.end - span.start - 0.08);
    const flow = interactive ? this.flowArrowsSvg(design, words, W, H) : null;
    const strokes = design.draws.map((d) => {
      const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("class", "lyric-draw");
      svg.setAttribute("viewBox", d.view_box);
      for (const [key, value] of [["--x", d.x], ["--y", d.y], ["--dw", d.w], ["--dh", d.h]])
        svg.style.setProperty(key, `${value}px`);
      svg.innerHTML = d.paths.map((p) => `<path d="${p.d}" pathLength="1" fill="none" stroke="${p.stroke}" stroke-width="${p.width}" stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="1" stroke-dashoffset="${1 - wordsClamp((local - p.from) / Math.max(0.01, p.to - p.from))}"/>`).join("");
      return svg;
    });
    const items = design.items.map((it, i) => this.drawLineItem(world, words, span, local, interactive, W, it, i, design));
    world.replaceChildren(...flow ? [flow] : [], ...strokes, ...items);
    requestAnimationFrame(() => {
      const view = $("view-design");
      if (view)
        wordsArrows(view);
    });
  }
  activeBackdrop(anim) {
    const list = typeof wordsAvailableBackdrops === "function" ? wordsAvailableBackdrops() : [];
    return list.find((b) => b.id === anim?.background_asset) || list[0] || null;
  }
  bindStageDraw(box, world, anim) {
    let pts = [];
    let liveSvg = null;
    box.onpointerdown = (e) => {
      if (!wordsPanel.drawMode || e.target.closest(".lyric-stage-modes"))
        return;
      e.preventDefault();
      box.setPointerCapture(e.pointerId);
      const [W, H] = wordsWorld(), r = world.getBoundingClientRect();
      const pt = (ev) => [
        Math.round(Math.max(0, Math.min(W, (ev.clientX - r.left) * W / Math.max(1, r.width)))),
        Math.round(Math.max(0, Math.min(H, (ev.clientY - r.top) * H / Math.max(1, r.height))))
      ];
      pts = [pt(e)];
      liveSvg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      liveSvg.setAttribute("class", "lyric-draw");
      liveSvg.setAttribute("viewBox", `0 0 ${W} ${H}`);
      for (const [k, v] of [["--x", 0], ["--y", 0], ["--dw", W], ["--dh", H]])
        liveSvg.style.setProperty(k, `${v}px`);
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("fill", "none");
      path.setAttribute("stroke", wordsPanel.brushColor || "#f59e0b");
      path.setAttribute("stroke-width", String(wordsPanel.brushSize || 8));
      path.setAttribute("stroke-linecap", "round");
      liveSvg.appendChild(path);
      world.appendChild(liveSvg);
      box.onpointermove = (ev) => {
        if (!liveSvg)
          return;
        const p = pt(ev), last = pts.at(-1);
        if (Math.hypot(p[0] - last[0], p[1] - last[1]) > 3) {
          pts.push(p);
          path.setAttribute("d", pts.map((c, idx) => `${idx ? "L" : "M"} ${c[0]} ${c[1]}`).join(" "));
        }
      };
      box.onpointerup = () => {
        box.onpointermove = null;
        box.onpointerup = null;
        if (pts.length > 1) {
          const lid = wordsPanel.line || wordsLines(anim)[0]?.id;
          const brush = wordsEffectCraftTools().paint_presets?.find((b) => b.id === wordsPanel.activeBrush);
          if (lid) {
            wordsDesignEdit(anim, lid, (d) => {
              d.draws.push({
                name: brush?.name || "Freehand Sketch",
                view_box: `0 0 ${W} ${H}`,
                x: 0,
                y: 0,
                w: W,
                h: H,
                paths: [{
                  d: pts.map((c, idx) => `${idx ? "L" : "M"} ${c[0]} ${c[1]}`).join(" "),
                  stroke: wordsPanel.brushColor || "#f59e0b",
                  width: wordsPanel.brushSize || brush?.diameter || 8,
                  from: 0,
                  to: 0.75
                }]
              });
            });
          }
        } else if (liveSvg) {
          liveSvg.remove();
        }
        liveSvg = null;
      };
    };
  }
  canvas(anim) {
    const world = el("div", { class: "lyric-world" });
    const bg = wordsPanel.scene ? this.activeBackdrop(anim) : null;
    const bgImg = bg ? el("img", { class: "lyric-scene-bg", src: convertFileSrc(bg.path), alt: bg.name }) : null;
    const stageModes = el("div", { class: "lyric-stage-modes" }, el("button", {
      class: `lyric-mode-pill${!wordsPanel.drawMode ? " active" : ""}`,
      title: "Drag, scale & tilt word tokens directly on the 16:9 stage",
      onclick: () => {
        wordsPanel.drawMode = false;
        wordsPanel.inspectorTab = "word";
        const lid = wordsPanel.line || wordsLines(anim)[0]?.id;
        if (lid)
          wordsDesignController.splitLineTokens(anim, lid);
        else
          wordsRender();
      }
    }, "✥ Kinetic Motion"), el("button", {
      class: `lyric-mode-pill draw${wordsPanel.drawMode ? " active" : ""}`,
      title: "Draw freehand motion overlays & EffectCraft paint strokes directly on the 16:9 stage",
      onclick: () => {
        wordsPanel.drawMode = !wordsPanel.drawMode;
        if (wordsPanel.drawMode)
          wordsPanel.inspectorTab = "draw";
        wordsRender();
      }
    }, "✎ Draw Motion Overlay"), el("button", {
      class: "lyric-mode-pill warp",
      title: "Cycle spatial trajectory choreography across the stage",
      onclick: () => {
        const lid = wordsPanel.line || wordsLines(anim)[0]?.id;
        if (!lid)
          return;
        const order = (wordsChoreographies().map((c) => c.id).length ? wordsChoreographies().map((c) => c.id) : ["cascade", "scatter", "poster", "stack", "plain"]);
        wordsPanel.choreoIdx = ((wordsPanel.choreoIdx || 0) + 1) % order.length;
        wordsPanel.designs.set(lid, wordsDesignController.layout(anim, lid, order[wordsPanel.choreoIdx]));
        wordsRender();
      }
    }, "⤴ Warp Path"));
    const hint = el("div", { class: "lyric-stage-hint" }, wordsPanel.drawMode ? "Draw Mode Active: Click & drag on stage to sketch freehand SVG motion overlays" : "Drag any word to adjust spatial trajectory · Click token below to choreograph");
    const box = el("div", { class: `lyric-canvas${bgImg ? " has-scene-bg" : ""}${wordsPanel.drawMode ? " draw-mode" : ""}`, "aria-label": "Line designer" }, bgImg, world, stageModes, hint);
    this.bindStageDraw(box, world, anim);
    wordsPanel.canvas = { box, world, bgImg };
    wordsFitter.observe(box);
    return box;
  }
  sceneSeek(_t) {}
  canvasDraw(t = wordsPanel.time) {
    const anim = wordsCurrent(), canvas = wordsPanel.canvas;
    if (!anim || !canvas || !wordsPanel.line)
      return;
    if (canvas.bgImg && wordsPanel.scene) {
      const bg = this.activeBackdrop(anim);
      if (bg && canvas.bgImg.getAttribute("src") !== convertFileSrc(bg.path))
        canvas.bgImg.setAttribute("src", convertFileSrc(bg.path));
    }
    this.drawLine(canvas.world, anim, wordsPanel.line, t, true);
  }
  syncPlayTime(t) {
    wordsPanel.time = t;
    wordsPanel.playing = true;
    const scrub = $("words-scrub");
    if (scrub) {
      scrub.value = String(t);
      scrub.defaultValue = scrub.value;
    }
    if (wordsPanel.timeLabel)
      wordsPanel.timeLabel.textContent = `${wordsClock(t)} / ${wordsClock(wordsSongSpan().end)}`;
    this.canvasDraw(t);
    wordsNow(t);
  }
  checkPlayRange(audio, t) {
    const range = wordsPanel.previewRange;
    if (!range || t < range.end)
      return true;
    if (wordsPanel.loopLine) {
      audio.currentTime = range.start;
      return true;
    }
    audio.pause();
    wordsPanel.playing = false;
    return false;
  }
  play(forceStart = false) {
    const anim = wordsCurrent();
    if (!anim || !wordsPanel.line)
      return;
    const path = wordsSongPath();
    if (!path)
      return toast("Add the song in Project › Song to hear it here");
    wordsPanel.audio ??= new Audio;
    const audio = wordsPanel.audio, span = wordsLineSpan(anim, wordsPanel.line);
    if (!forceStart && !audio.paused) {
      audio.pause();
      wordsPanel.playing = false;
      return;
    }
    if (audio.getAttribute("src") !== convertFileSrc(path))
      audio.src = convertFileSrc(path);
    audio.currentTime = wordsPanel.previewRange ? wordsPanel.previewRange.start : Math.max(0, span.start - 0.4);
    audio.play();
    const tick = () => {
      if (audio.paused || audio.ended || state.view !== "design") {
        audio.pause();
        wordsPanel.playing = false;
        return;
      }
      const t = audio.currentTime;
      if (!this.checkPlayRange(audio, t))
        return;
      if (!wordsPanel.previewRange) {
        const animations = wordsPanel.read.document.animations;
        const here = animations.find((a) => {
          const s = wordsSpan(a);
          return t >= s.start - 0.4 && t < s.end;
        });
        const unsaved = wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length;
        if (here && here.animation_id !== wordsPanel.selected && !unsaved) {
          wordsPanel.selected = here.animation_id;
          wordsPanel.line = null;
          wordsPanel.item = null;
          wordsPanel.followed = `${here.animation_id}/playing`;
          wordsRender();
        }
        const current = wordsCurrent(), line = wordsLines(current).findLast((l) => wordsAt(l.words[0]) <= t + 0.4);
        if (line && line.id !== wordsPanel.line) {
          wordsPanel.line = line.id;
          wordsPanel.item = null;
          wordsPanel.followed = `${current.animation_id}/${line.id}`;
          wordsRender();
        }
      }
      this.syncPlayTime(audio.currentTime);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }
  setChipVars(chip, font, weight, italic, color, scale) {
    chip.dataset.weight = String(weight);
    if (italic)
      chip.dataset.italic = "true";
    chip.style.setProperty("--word-font", font);
    chip.style.setProperty("--word-color", color);
    chip.style.setProperty("--word-scale", String(scale));
  }
  applyItemChipStyle(chip, anim, it, wordId) {
    const font = it.font === "song" || !it.font ? wordsSongFont(anim) : it.font;
    const color = /^#[0-9a-f]{6}$/i.test(it.color) ? it.color : "currentColor";
    const scale = Math.min(1.7, Math.max(0.8, (it.size || 110) / 110)).toFixed(2);
    if (it.motion && it.motion !== "quiet" && (!it.accent || it.accent === wordId))
      chip.dataset.hero = "true";
    this.setChipVars(chip, font, it.weight || 400, it.italic, color, scale);
  }
  applyKineticChipStyle(chip, kin, wordId) {
    const st = kin.style || {}, hero = !!kin.acts?.[wordId];
    const font = (hero ? st.hero_font : null) || st.font || "sans-serif";
    const weight = (hero ? st.hero_weight : null) || st.weight || 700;
    const color = (hero ? st.hero_color : null) || st.color || "currentColor";
    const scale = hero ? Math.min(1.7, (st.hero_size || 110) / (st.size || 64)) : 1;
    if (hero)
      chip.dataset.hero = "true";
    this.setChipVars(chip, font, weight, hero && st.hero_italic, color, scale);
  }
  applyChipStyle(chip, anim, w, it) {
    if (it)
      this.applyItemChipStyle(chip, anim, it, w.word_id);
    else if (anim.kinetic)
      this.applyKineticChipStyle(chip, anim.kinetic, w.word_id);
    if (wordsPanel.designs.has(w.line_id))
      chip.dataset.changed = "";
  }
  togglePickedWord(w, sameLine) {
    if (!sameLine)
      return [w.word_id];
    return wordsPanel.picked.includes(w.word_id) ? wordsPanel.picked.filter((id) => id !== w.word_id) : [...wordsPanel.picked, w.word_id];
  }
  switchSection(id) {
    if (id === wordsPanel.selected)
      return true;
    if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) {
      if (!confirm("Discard unsaved lyric changes?"))
        return false;
    }
    wordsPanel.designs.clear();
    wordsPanel.styleEdits = {};
    wordsPanel.selected = id;
    return true;
  }
  chipClick(anim, w, index, event) {
    event.stopPropagation();
    const sameLine = anim.animation_id === wordsPanel.selected && wordsPanel.line === w.line_id;
    if (!this.switchSection(anim.animation_id))
      return;
    wordsVideo("pause");
    wordsPanel.playing = false;
    wordsPanel.autoPlay = false;
    const nextPicked = this.togglePickedWord(w, sameLine && event.shiftKey);
    wordsPanel.picked = nextPicked.length ? nextPicked : [w.word_id];
    if (wordsPanel.mode === "timing")
      wordsPanel.mode = "lyrics";
    wordsPanel.cue = null;
    wordsPanel.shelf = "acts";
    wordsPanel.line = w.line_id;
    wordsPanel.item = index;
    wordsPanel.followed = `${anim.animation_id}/${w.line_id}`;
    wordsPanel.previewRange = wordsLineWindow(anim, w.line_id);
    wordsPanel.time = wordsAt(w) + 0.12;
    wordsRender();
    wordsSeekSong(wordsPanel.time);
    requestAnimationFrame(() => wordsEnsureBelowVideo(wordsLineSpan(anim, w.line_id).start));
  }
  applyPreviewWindow(range, pausedTime, ensureAt) {
    wordsPanel.previewRange = range;
    wordsPanel.autoPlay = wordsPanel.loopLine;
    wordsPanel.time = wordsPanel.loopLine ? range.start : pausedTime;
    wordsRender();
    if (wordsPanel.loopLine)
      wordsVideoRange();
    else
      wordsSeekSong(wordsPanel.time);
    requestAnimationFrame(() => wordsEnsureBelowVideo(ensureAt));
  }
  selectLine(anim, line) {
    if (!this.switchSection(anim.animation_id))
      return;
    wordsVideo("pause");
    wordsPanel.playing = false;
    const hero = line.words.find((w) => anim.kinetic?.acts?.[w.word_id]) || line.words[0];
    if (wordsPanel.mode === "timing")
      wordsPanel.mode = "lyrics";
    wordsPanel.line = line.id;
    wordsPanel.item = null;
    wordsPanel.cue = null;
    wordsPanel.picked = hero ? [hero.word_id] : [];
    if (!wordsPanel.shelf || wordsPanel.shelf === "cues")
      wordsPanel.shelf = "arrive";
    wordsPanel.followed = `${anim.animation_id}/${line.id}`;
    const span = wordsLineSpan(anim, line.id), lastWord = line.words.at(-1);
    const paused = Math.min(wordsSpan(anim).end - 0.35, span.end - 0.12, (lastWord ? wordsAt(lastWord) : span.start) + 0.18);
    this.applyPreviewWindow(wordsLineWindow(anim, line.id), paused, span.start);
  }
  chip(anim, w) {
    const design = wordsDesign(anim, w.line_id), index = design.items.findIndex((it) => it.word_ids.includes(w.word_id)), it = design.items[index];
    const isPicked = wordsPanel.picked.includes(w.word_id) || !wordsPanel.picked.length && wordsPanel.item === index;
    const on = anim.animation_id === wordsPanel.selected && wordsPanel.line === w.line_id && isPicked;
    const move = it && it.motion !== "quiet" ? wordsMoves().find((m) => m[0] === it.motion)?.[1] : "";
    const at = wordsAt(w), clip = wordsClip(w.word_id, anim), dur = clip?.duration ?? Math.max(0.2, (+w.end || at + 0.35) - at);
    const chip = el("button", {
      class: `word lettered${on ? " active" : ""}`,
      "data-word-id": w.word_id,
      "data-start": at.toFixed(3),
      "data-end": (at + dur).toFixed(3),
      title: `${w.text} · ${wordsClock(at)}${move ? ` · ${move}` : ""} · Click to edit word act (Shift-click adds words)`,
      onclick: (event) => this.chipClick(anim, w, index, event)
    }, w.text);
    this.applyChipStyle(chip, anim, w, it);
    return chip;
  }
  pickedItems(design) {
    const picked = design.items.flatMap((it, i) => it.word_ids.some((id) => wordsPanel.picked.includes(id)) ? [i] : []);
    return picked.length ? picked : wordsPanel.item == null ? [] : [wordsPanel.item];
  }
  gaps(animations) {
    const spans = animations.map(wordsSpan).sort((a, b) => a.start - b.start), gaps = [];
    let at = 0;
    for (const s of spans) {
      if (s.start - at > 1.5)
        gaps.push([at, s.start]);
      at = Math.max(at, s.end);
    }
    return gaps.map(([a, b]) => wordsPlace(el("div", { class: "words-gap", title: `No singing from ${wordsClock(a)} to ${wordsClock(b)}` }, el("span", {}, "Instrumental"), el("span", { class: "meta" }, `${wordsClock(a)} – ${wordsClock(b)}`)), a, b - a));
  }
  buildLineCard(a, j, line, at, len) {
    const q = (wordsPanel.searchQuery || "").trim().toLowerCase();
    const matchesSearch = !q || line.words.some((w) => String(w.text || "").toLowerCase().includes(q));
    const design = wordsDesign(a, line.id);
    const active = a.animation_id === wordsPanel.selected && wordsPanel.line === line.id;
    const bg = this.activeBackdrop(a);
    const moveList = wordsMoves();
    const moves = [...new Set((design.items || []).filter((it) => it.motion && it.motion !== "quiet").map((it) => moveList.find((m) => m[0] === it.motion)?.[1] || it.motion))];
    const strokes = [...new Set((design.draws || []).map((d) => d.name))];
    const pills = [
      ...moves.slice(0, 2).map((m, idx) => el("span", { class: `words-line-pill ${idx ? "trail-pill" : "move-pill"}` }, `${idx ? "⇉" : "\uD83D\uDD25"} ${m}`)),
      ...strokes.slice(0, 1).map((s) => el("span", { class: "words-line-pill stroke-pill" }, `✎ ${s}`))
    ];
    return wordsPlace(el("div", {
      class: `words-line${j ? "" : " first"}${active ? " active" : ""}${matchesSearch ? "" : " search-dimmed"}`,
      "data-kind": a.scope?.section === "chorus" ? "chorus" : "verse",
      "data-animation-id": a.animation_id,
      "data-line-id": line.id,
      tabindex: "0",
      title: "Click line to focus and choreograph its words on the 16:9 stage",
      onkeydown: (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          this.selectLine(a, line);
        }
      },
      onclick: () => this.selectLine(a, line)
    }, el("div", { class: "words-line-card-row" }, bg ? el("div", { class: "words-line-thumb-wrap" }, el("img", { class: "words-line-thumb", src: convertFileSrc(bg.path), alt: bg.name, loading: "lazy" }), el("span", { class: "words-line-ratio" }, "16:9")) : null, el("div", { class: "words-line-body" }, el("div", { class: "words-line-top" }, el("span", { class: "words-line-time" }, `[${wordsClock(at)}]`), el("div", { class: "words-line-words" }, ...line.words.map((w) => this.chip(a, w)))), el("div", { class: "words-line-bottom" }, ...pills, bg ? el("span", { class: "words-line-scene" }, bg.name) : null)), el("span", { class: "words-line-tether", "aria-hidden": "true", title: "Tethered to 16:9 stage & stem timeline" }, "⚓"))), at, len);
  }
  timeline(animations, end) {
    return wordsPlace(el("div", { class: "words-tl words-roll", "aria-label": "Lyrics on the song timeline" }, ...wordsTicks(end), ...this.gaps(animations), ...animations.map((a, idx) => wordsBand(a, "button", { "data-section": a.animation_id, onclick: () => {
      const first = wordsLines(a)[0];
      if (first)
        this.selectLine(a, first);
    } }, animations[idx + 1] ? wordsSpan(animations[idx + 1]).start : 1 / 0)), ...wordsRows(animations).map(([a, j, line, at, len]) => this.buildLineCard(a, j, line, at, len)), el("span", { class: "words-tl-head" })), 0, end);
  }
  buildRollFrame(cue, lane) {
    const thumb = (progress) => {
      const image = cue.kind === "drawing" ? this.stroke(cue.stroke) : wordsOptionPreview("overlays", cue.overlay);
      image.setAttribute("class", "words-cue-preview");
      if (cue.kind === "drawing") {
        const path = image.querySelector("path");
        path.setAttribute("pathLength", "1");
        path.setAttribute("stroke-dasharray", "1");
        path.setAttribute("stroke-dashoffset", String(1 - progress));
      }
      return image;
    };
    const handle = (edge, label) => cue.index == null && cue.kind !== "overlay" ? null : el("button", {
      class: `words-cue-handle ${edge}`,
      "aria-label": `${label} ${cue.name}`,
      title: `${label} · drag or use ↑ ↓ (Shift for fine timing)`,
      onpointerdown: (event) => this.cueDrag(event, cue, edge),
      onclick: (event) => event.stopPropagation()
    }, edge === "start" ? "●" : "━");
    const frame = wordsPlace(el("div", {
      class: `words-graphic${wordsPanel.cue === cue.id || cue.line_id === wordsPanel.line ? " active" : ""}`,
      "data-cue-id": cue.id,
      "data-line-id": cue.line_id || "",
      role: "button",
      tabindex: "0",
      "aria-label": `${cue.name} at ${wordsClock(cue.start)}`,
      title: `${cue.name}${cue.word ? ` · “${cue.word}”` : ""} · ${wordsClock(cue.start)} – ${wordsClock(cue.end)} · Click to replay this connection`,
      onclick: () => this.previewCue(cue),
      onkeydown: (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          this.previewCue(cue);
        }
      }
    }, el("div", { class: "words-cue-film" }, thumb(0.35), thumb(0.7), thumb(1)), handle("start", "Move"), handle("end", "Trim end of"), el("span", { class: "words-cue-caption" }, cue.name)), cue.start, cue.end - cue.start);
    frame.style.setProperty("--cue-lane", String(lane));
    frame.style.setProperty("--cue-ink", cue.color || "var(--accent)");
    return frame;
  }
  synthRollCues(animations) {
    const out = [];
    for (const a of animations) {
      for (const line of wordsLines(a)) {
        const span = wordsLineSpan(a, line.id), d = wordsDesign(a, line.id);
        const hero = (d.items || []).find((it) => (it.size || 0) >= 140 || it.motion === "pop") || d.items?.[0];
        if (!hero)
          continue;
        const label = hero.word_ids.map((id) => line.words.find((w) => w.word_id === id)?.text).join(" ");
        out.push({
          id: `${a.animation_id}-${line.id}-stem`,
          animation_id: a.animation_id,
          line_id: line.id,
          kind: "drawing",
          name: label,
          word: label,
          word_ids: hero.word_ids,
          start: span.start,
          end: Math.max(span.start + 1.2, span.end - 0.15),
          color: hero.color || "#35e1c0",
          stroke: wordsStrokes()[1] || wordsStrokes()[0] || { view_box: "0 0 280 30", d: "M 5 14 Q 61 5 125 13 T 269 10" }
        });
      }
    }
    return out;
  }
  roll(animations, end) {
    const raw = wordsPanel.read.graphics || [];
    const cues = raw.length ? raw : this.synthRollCues(animations);
    const laneEnds = [];
    const placed = cues.map((cue) => {
      let lane = laneEnds.findIndex((t) => t <= cue.start);
      if (lane < 0)
        lane = laneEnds.length;
      laneEnds[lane] = cue.end;
      return { cue, lane };
    });
    const frames = placed.map(({ cue, lane }) => this.buildRollFrame(cue, lane));
    const roll = wordsPlace(el("div", { class: "words-tl words-roll", "aria-label": "Motion graphics on the song timeline" }, ...wordsTicks(end), ...frames, el("span", { class: "words-tl-head" })), 0, end);
    roll.style.setProperty("--cue-lanes", String(Math.max(1, laneEnds.length)));
    return roll;
  }
  stroke(stroke) {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg"), path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    svg.setAttribute("viewBox", stroke.view_box);
    svg.setAttribute("aria-hidden", "true");
    path.setAttribute("d", stroke.d);
    path.setAttribute("fill", "none");
    path.setAttribute("stroke", "currentColor");
    path.setAttribute("stroke-width", "2");
    path.setAttribute("vector-effect", "non-scaling-stroke");
    path.setAttribute("stroke-linecap", "round");
    svg.appendChild(path);
    return svg;
  }
  cueIntersects(cue, animationId, start, end, wordIds) {
    return cue.animation_id === animationId && (cue.word_ids?.length ? cue.word_ids.some((id) => wordIds.includes(id)) : cue.start < end && cue.end > start);
  }
  cueWindow(anim, cue, line) {
    const sec = wordsSpan(anim);
    return { start: Math.max(sec.start, Math.min(cue.start, line.start) - 0.15), end: Math.min(sec.end, Math.max(cue.end, line.end)) };
  }
  previewCue(cue) {
    const anim = wordsPanel.read.document.animations.find((a) => a.animation_id === cue.animation_id);
    if (!anim || !this.switchSection(anim.animation_id))
      return;
    wordsVideo("pause");
    wordsPanel.playing = false;
    wordsPanel.line = cue.line_id || wordsLines(anim)[0]?.id;
    const fallbackWord = wordsLines(anim).find((l) => l.id === wordsPanel.line)?.words[0]?.word_id;
    wordsPanel.picked = cue.word_ids?.length ? cue.word_ids : fallbackWord ? [fallbackWord] : [];
    if (wordsPanel.mode === "timing")
      wordsPanel.mode = "lyrics";
    wordsPanel.cue = cue.id;
    wordsPanel.shelf = "cues";
    if (cue.kind === "overlay")
      wordsPanel.topShelf = "overlay";
    wordsPanel.followed = `${anim.animation_id}/${wordsPanel.line}`;
    const range = this.cueWindow(anim, cue, wordsLineSpan(anim, wordsPanel.line));
    this.applyPreviewWindow(range, Math.min(cue.end - 0.2, (cue.draw_end ?? cue.end) - 0.05), cue.start);
  }
  async retimeCue(cue, start, end) {
    const anim = wordsCurrent();
    if (!anim?.kinetic || cue.index == null && cue.kind !== "overlay")
      return;
    start = Math.max(0, start);
    end = Math.max(start + 0.05, end);
    wordsPanel.previewRange = this.cueWindow(anim, { ...cue, start, end }, wordsLineSpan(anim, cue.line_id || wordsLines(anim)[0]?.id));
    wordsPanel.autoPlay = false;
    if (cue.kind === "overlay")
      return wordsKinetic({ overlay_offset: +(start - cue.anchor).toFixed(2), overlay_duration: +(end - start).toFixed(2) });
    const duration = Math.min((cue.draw_end ?? end) - cue.start, end - start);
    const draws = anim.kinetic.draws.map((d, i) => i === cue.index ? { ...d, offset: +(start - cue.anchor).toFixed(2), duration: +duration.toFixed(2), hold: +(end - start - duration).toFixed(2) } : d);
    await wordsKinetic({ draws });
  }
  cueDrag(event, cue, edge) {
    event.preventDefault();
    event.stopPropagation();
    wordsVideo("pause");
    if (cue.animation_id !== wordsPanel.selected)
      wordsSelect(cue.animation_id);
    if (cue.animation_id !== wordsPanel.selected)
      return;
    const view = $("view-design");
    view.setPointerCapture(event.pointerId);
    wordsPanel.cueDrag = { cue, before: { ...cue }, edge, y: event.clientY, scale: parseFloat(getComputedStyle(view).getPropertyValue("--song-s")) || 18 };
  }
}
const wordsTimelineController = new WordsTimelineController;
window.WordsTimelineController = WordsTimelineController;
window.wordsTimelineController = wordsTimelineController;
