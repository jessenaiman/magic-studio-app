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

function wordsLabel(anim) {
  const s = anim.scope || {};
  const name = String(s.section || "lyrics").replace(/_(\d+)$/, " $1").replace(/_/g, " ");
  return `${name[0].toUpperCase()}${name.slice(1)}${s.section === "chorus" || s.occurrence > 1 ? ` ${s.occurrence}` : ""}`;
}

const wordsEnsureKinetic = (a) => (a.kinetic ??= { layout: "karaoke", entrance: "pop", acts: {}, draws: [], overlay: "none", band: "top", seed: 1,
  style: { font: "Fraunces", hero_font: "Fraunces", weight: 600, hero_weight: 700, hero_italic: true, size: 58, hero_size: 92, color: "#FFF6E3", hero_color: "#FFD27D", outline: "#143042" } });

async function wordsBootstrapStudio(studio) {
  if (!serverUrl() && typeof designCommand === "function") await designCommand("video.server_start").catch(() => {});
  if (studio?.sections?.length) return studio;
  await wordsCommand("words.track").catch(() => {});
  return wordsCommand("words.studio_track").catch((error) => ({ error: String(error).replace(/^.*?: /, "") }));
}

async function wordsOpen() {
  const view = $("view-design");
  if (!view) return;
  if (!wordsPanel.read) view.replaceChildren(el("p", { class: "panel-empty inline" }, "Loading the words…"));
  try {
    const [read, align, rawStudio, backgrounds] = await Promise.all([
      wordsCommand("words.read"),
      wordsCommand("words.align"),
      wordsCommand("words.studio_track").catch((error) => ({ error: String(error).replace(/^.*?: /, "") })),
      wordsCommand("storyboard.backgrounds").catch(error=>({images:[],error:String(error).replace(/^.*?: /, "")})),
    ]);
    const studio = await wordsBootstrapStudio(rawStudio);
    (read.document?.animations || []).forEach(wordsEnsureKinetic);
    wordsPanel.read = read;
    wordsPanel.backgrounds = backgrounds;
    wordsPanel.align = align;
    await wordsLoadFonts(read.fonts || []);
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

// The video's own lettering, loaded once so the designer letters in the same type.
const wordsFontsLoaded = new Set();
async function wordsLoadFonts(fonts) {
  await Promise.all(fonts.filter((f) => !wordsFontsLoaded.has(f.path)).map(async (f) => {
    wordsFontsLoaded.add(f.path);
    try { document.fonts.add(await new FontFace(f.family, `url(${JSON.stringify(convertFileSrc(f.path))})`, { weight: f.weight || "400" }).load()); }
    catch (error) { toast(`The video's font ${f.family} did not load: ${error}`); }
  }));
}
const wordsSongFont = (anim) => anim.style?.font || wordsPanel.read?.fonts?.[0]?.family || "sans-serif";

function wordsSelect(id) {
  if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) {
    if (!confirm("Discard unsaved lyric changes?")) return;
  }
  wordsPanel.designs.clear(); wordsPanel.styleEdits = {}; wordsPanel.line = null; wordsPanel.item = null; wordsPanel.selected = id;
  wordsRender();
  wordsVideoRange();
}

// The registry's text effects for the section, as cards under the lanes.
function wordsEffects(anim) {
  const current = anim.effects?.text?.name;
  const registry = wordsPanel.effects?.kinds?.text?.effects || [];
  if (!wordsPanel.effects) {
    wordsPanel.effects = { loading: true };
    wordsCommand("words.effects", { kind: "text" }).catch((error) => ({ error: String(error).replace(/^.*?: /, "") }))
      .then((found) => { wordsPanel.effects = found; if (state.view === "design") wordsRender(); });
  }
  return el("div", { class: "words-effects" },
    current ? el("button", { class: "effect", "data-effect": "", onclick: () => wordsApplyRegistry(null) },
      el("strong", {}, `Using ${current}`), el("span", { class: "muted" }, "Click to remove this effect.")) : null,
    wordsPanel.effects?.error ? el("p", { class: "muted" }, wordsPanel.effects.error)
      : wordsPanel.effects?.loading ? el("p", { class: "muted" }, "Finding effects…") : null,
    ...registry.filter((e) => e.name !== current).map((e) => el("button", { class: "effect", "data-effect": e.name, title: (e.tags || []).join(" · "), onclick: () => wordsApplyRegistry(e.name) },
      el("strong", {}, e.title || e.name), el("span", { class: "muted" }, e.description || ""))));
}

// Put a registry effect on the section's text track (null takes it off): install it if needed,
// save it with the expected revision, compose the section and show the edited result.
async function wordsApplyRegistry(name) {
  const anim = wordsCurrent();
  if (!anim) return;
  if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) return toast("Save or discard the lyric changes first");
  try {
    if (name) { toast(`Installing ${name}…`); await wordsCommand("words.effect_install", { name }); }
    const result = await wordsCommand("words.save", { animation_id: anim.animation_id, expected_source_revision: anim.source_revision,
      origin: "magicstudio-design", effects: { text: name ? { name } : null } });
    if (result.conflict) { toast("Someone changed this section first; reloaded the latest"); return wordsOpen(); }
    await wordsCommand("words.compose", { animation_id: anim.animation_id });
    wordsPanel.show = name ? "edited" : "full";
    wordsPanel.stage = null;
    toast(name ? `${name} on ${wordsLabel(anim)}` : `Effect removed from ${wordsLabel(anim)}`);
    await wordsOpen();
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
}

// The song clock of the page. A word sits at its clip in the video (else the sheet's own time); a
// section spans its stretch of the video (else its words).
const wordsAt = (w) => wordsClip(w.word_id, wordsPanel.read?.document?.animations?.find((a) => a.words.includes(w)) || wordsCurrent())?.start ?? +w.start;
function wordsSpan(anim) {
  const video = wordsStudioSection(anim);
  return video ? { start: video.start, end: video.end } : { start: Math.min(...anim.words.map(wordsAt)), end: Math.max(...anim.words.map((w) => +w.end)) };
}
function wordsLines(anim) {
  const lines = [];
  for (const w of anim.words) { if (lines.at(-1)?.id !== w.line_id) lines.push({ id: w.line_id, words: [] }); lines.at(-1).words.push(w); }
  return lines;
}
// A timeline item's song time and length, as data; the timeline rule turns seconds into height.
function wordsPlace(node: any, at: number, len?: number) {
  node.style.setProperty("--at-s", at.toFixed(3));
  if (len != null) node.style.setProperty("--len-s", Math.max(0, len).toFixed(3));
  return node;
}
// Every sung line of every section, as [section, line index, line, start, length].
function wordsRows(animations) {
  return animations.flatMap((a, idx) => {
    const span = wordsSpan(a), lines = wordsLines(a), nextStart = animations[idx + 1] ? wordsSpan(animations[idx + 1]).start : Infinity;
    const secEnd = Math.min(span.end, nextStart);
    return lines.map((line, j) => { const at = wordsAt(line.words[0]); return [a, j, line, at, Math.max(0.2, (lines[j + 1] ? wordsAt(lines[j + 1].words[0]) : secEnd) - at)]; });
  });
}
const wordsBand = (a, tag, extra = {}, nextStart = Infinity) => { const span = wordsSpan(a), end = Math.min(span.end, nextStart);
  return wordsPlace(el(tag, { class: `words-band${a.animation_id === wordsPanel.selected ? " active" : ""}`, "data-kind": a.scope?.section === "chorus" ? "chorus" : "verse", ...extra },
    el("strong", {}, wordsLabel(a)), el("span", { class: "meta" }, wordsClock(span.start))), span.start, Math.max(0.2, end - span.start)); };

// The lines of a section that hold a given word, and a line's words.
const wordsLineWords = (anim, lineId) => anim.words.filter((w) => w.line_id === lineId);
// The composition's size: the video's, once the player reports it.
const wordsWorld = () => [wordsPanel.size?.width || 1920, wordsPanel.size?.height || 1080];
// A line's span on the song clock: from its first sung word to the next line (or the section's end).
function wordsLineSpan(anim, lineId) {
  const lines = wordsLines(anim), k = Math.max(0, lines.findIndex((l) => l.id === lineId));
  const start = wordsAt(lines[k].words[0]);
  return { start, end: lines[k + 1] ? wordsAt(lines[k + 1].words[0]) : wordsSpan(anim).end };
}
function wordsLineWindow(anim, lineId = wordsPanel.line) {
  const sec = wordsSpan(anim), span = wordsLineSpan(anim, lineId || wordsLines(anim)[0]?.id);
  return { start: Math.max(sec.start, span.start - 0.15), end: Math.min(sec.end, Math.max(span.start + 0.6, span.end)) };
}
function wordsPreviewLine(anim = wordsCurrent(), lineId = wordsPanel.line, play = true) {
  if (!anim) return;
  const targetLine = lineId || wordsLines(anim)[0]?.id;
  if (!targetLine) return;
  const span = wordsLineSpan(anim, targetLine);
  wordsPanel.previewRange = wordsLineWindow(anim, targetLine);
  wordsPanel.autoPlay = play;
  wordsPanel.time = play ? wordsPanel.previewRange.start : Math.max(span.start, span.end - 0.18);
  wordsVideoRange();
}
// The design a line shows: an unsaved edit, else the saved one, else a plain line.
function wordsDesign(anim, lineId) {
  const pending = wordsPanel.designs.get(lineId);
  return pending !== undefined ? pending ?? wordsLayout(anim, lineId, "plain") : anim.design?.[lineId] ?? wordsLayout(anim, lineId, "plain");
}
function wordsDesignEdit(anim, lineId, change) {
  const design = structuredClone(wordsDesign(anim, lineId));
  change(design);
  wordsPanel.designs.set(lineId, design);
  wordsRender();
}

// Ready-made layouts for a whole line. Sizes and places are fractions of the composition, so a
// layout fits any video; every word stays its own item unless the layout groups context words.
function wordsLayout(anim, lineId, kind) {
  const words = wordsLineWords(anim, lineId), [W, H] = wordsWorld();
  const item = (ws, x, y, size, look = "song", motion = "quiet") => {
    const [, , , font, weight, italic] = WORDS_LETTERING.find((l) => l[0] === look);
    return { word_ids: ws.map((w) => w.word_id), x: Math.round(x), y: Math.round(y), size: Math.round(size), font, weight, italic, color: anim.style?.color || "#1b2a33",
      outline: null, rotate: 0, motion, accent: motion === "quiet" ? null : ws.at(-1).word_id };
  };
  const width = (ws, size, look = "song") => wordsMeasure(anim, ws.map((w) => w.text).join(" "), size, look);
  if (kind === "poster") {
    // The longest word carries the line; the words around it stay small.
    const hero = words.reduce((a, b) => (b.text.length > a.text.length ? b : a)), k = words.indexOf(hero), small = H * 0.04;
    return { items: [
      ...(k ? [item(words.slice(0, k), W * 0.45, H * 0.1, small, "sans")] : []),
      item([hero], W * 0.44, H * 0.15, H * 0.15, "serif-italic", "pop"),
      ...(k < words.length - 1 ? [item(words.slice(k + 1), W * 0.45 + width([hero], H * 0.15, "serif-italic") * 0.6, H * 0.33, small, "sans")] : [])], draws: [] };
  }
  if (kind === "stack") {
    const size = H * 0.085;
    return { items: words.map((w, i) => item([w], W * 0.08, H * 0.12 + i * size * 1.1, size, i === words.length - 1 ? "serif-bold" : "song", i === words.length - 1 ? "settle" : "quiet")), draws: [] };
  }
  // plain: one line along the bottom, each word in place
  const size = H * 0.06, gap = size * 0.35, total = words.reduce((sum, w) => sum + width([w], size) + gap, -gap);
  let x = (W - total) / 2;
  return { items: words.map((w) => { const it = item([w], x, H * 0.82, size); x += width([w], size) + gap; return it; }), draws: [] };
}

// A word's drawn width, measured in the lettering it will use.
const wordsMeasurer = document.createElement("canvas").getContext("2d");
function wordsMeasure(anim, text, size, look) {
  const [, , , font, weight, italic] = WORDS_LETTERING.find((l) => l[0] === look);
  wordsMeasurer.font = `${italic ? "italic " : ""}${weight} ${size}px ${font === "song" ? JSON.stringify(wordsSongFont(anim)) : font === "serif" ? "serif" : "sans-serif"}`;
  return wordsMeasurer.measureText(text).width;
}
const wordsEase = { out2: (x) => 1 - (1 - x) ** 2, out3: (x) => 1 - (1 - x) ** 3, sin: (x) => 0.5 - 0.5 * Math.cos(Math.PI * x) };
const wordsClamp = (x) => Math.max(0, Math.min(1, x));

function wordsItemMotionOffset(motion, settled, arc) {
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

function wordsItemPulse(local, a) {
  if (local < a || local >= a + 0.43) return 1;
  return local < a + 0.13 ? 1 + 0.12 * (local - a) / 0.13 : 1.12 - 0.12 * (local - a - 0.13) / 0.3;
}

function wordsBuildSleepSpans(text, local, a) {
  return [...text].map((ch, k) => {
    const u = wordsEase.sin(wordsClamp((local - a - k * 0.035) / 0.8)), letter = el("span", {}, ch);
    letter.style.setProperty("--ty", `${7 * u + 3 * Math.sin(k * 0.6) * Math.sin(u * Math.PI)}px`);
    letter.style.setProperty("--lr", `${-0.6 * u}deg`);
    return letter;
  });
}

function wordsDrawLineItem(world, words, span, local, interactive, W, it, i) {
  const text = it.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ");
  const accentWord = words.find((w) => w.word_id === it.accent);
  const a = (accentWord ? wordsAt(accentWord) : span.start) - span.start;
  const settled = wordsEase.out3(wordsClamp((local - a) / 0.65)), arc = wordsClamp((local - a) / 0.75);
  const pulse = wordsItemPulse(local, a);
  const [tx, ty] = wordsItemMotionOffset(it.motion, settled, arc);
  const node = el("div", { class: `lyric-item${interactive && wordsPanel.item === i ? " active" : ""}`, "data-weight": String(it.weight), "data-italic": String(it.italic), "data-font": it.font || "song" });
  if (it.outline) { node.dataset.outline = ""; node.style.setProperty("--outline", it.outline); }
  const sy = it.motion === "pop" ? pulse : (it.motion === "settle" ? 1 - 0.035 * settled : 1);
  for (const [key, value] of [["--x", `${it.x}px`], ["--y", `${it.y}px`], ["--size", `${it.size}px`], ["--ink", it.color], ["--rot", `${it.rotate}deg`],
    ["--o", String(wordsEase.out2(wordsClamp((local + 0.4) / 0.4)))], ["--tx", `${tx}px`], ["--ty", `${ty}px`],
    ["--sx", String(it.motion === "pop" ? pulse : 1)], ["--sy", String(sy)]]) node.style.setProperty(key, value);
  node.append(...(it.motion === "sleep" ? wordsBuildSleepSpans(text, local, a) : [el("span", {}, text)]));
  if (interactive) node.onpointerdown = (event) => {
    event.preventDefault();
    wordsPanel.item = i; wordsPanel.drag = { i, x: event.clientX, y: event.clientY, from: [it.x, it.y], scale: world.getBoundingClientRect().width / W, moved: false };
    wordsRender();
  };
  return node;
}

// One frame of a designed line at song time t, drawn into a composition-sized world. Every value
// the stylesheet needs goes in as a custom property; the motion follows the Pterodactyl lettering.
function wordsDrawLine(world, anim, lineId, t, interactive) {
  const [W, H] = wordsWorld(), design = wordsDesign(anim, lineId), span = wordsLineSpan(anim, lineId), words = wordsLineWords(anim, lineId);
  world.style.setProperty("--w", `${W}px`); world.style.setProperty("--h", `${H}px`);
  world.style.setProperty("--family", JSON.stringify(wordsSongFont(anim)));
  const local = t - span.start;
  const strokes = design.draws.map((d) => {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("class", "lyric-draw");
    svg.setAttribute("viewBox", d.view_box);
    for (const [key, value] of [["--x", d.x], ["--y", d.y], ["--dw", d.w], ["--dh", d.h]]) svg.style.setProperty(key, `${value}px`);
    svg.innerHTML = d.paths.map((p) => `<path d="${p.d}" pathLength="1" fill="none" stroke="${p.stroke}" stroke-width="${p.width}" stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="1" stroke-dashoffset="${1 - wordsClamp((local - p.from) / Math.max(0.01, p.to - p.from))}"/>`).join("");
    return svg;
  });
  const items = design.items.map((it, i) => wordsDrawLineItem(world, words, span, local, interactive, W, it, i));
  world.replaceChildren(...strokes, ...items);
}

// The designer's stage: the selected line on plain paper, the song under it.
function wordsCanvas(anim) {
  const world = el("div", { class: "lyric-world" });
  const scene = wordsPanel.scene && serverUrl() ? el("iframe", { class: "lyric-scene", title: "The scene behind the lyrics", tabindex: "-1" }) : null;
  const box = el("div", { class: "lyric-canvas", "aria-label": "Line designer" }, scene, world);
  wordsPanel.canvas = { box, world, scene };
  wordsFitter.observe(box);
  if (scene) wordsCommand("words.lyrics_view", { show: "scene" }).then((view) => {
    const server = new URL(serverUrl()), project = server.hash.split("/")[1] || "hyperframes";
    scene.addEventListener("load", () => wordsSceneSeek(wordsPanel.time));
    scene.setAttribute("src", `${server.origin}/api/projects/${project}/preview/comp/${view.src}`);
  }).catch((error) => toast(String(error).replace(/^.*?: /, "")));
  return box;
}
// The scene shows the video's picture at a song time, without its lyric clips.
function wordsSceneSeek(t) {
  wordsPanel.canvas?.scene?.contentWindow?.postMessage({ source: "hf-parent", type: "control", action: "seek", timeSeconds: Math.max(0, t), seekMode: "commit" }, "*");
}
function wordsCanvasDraw(t = wordsPanel.time) {
  const anim = wordsCurrent(), canvas = wordsPanel.canvas;
  if (!anim || !canvas || !wordsPanel.line) return;
  wordsDrawLine(canvas.world, anim, wordsPanel.line, t, true);
  if (!wordsPanel.playing || Math.abs(t - (wordsPanel.sceneAt ?? -9)) > 0.25) { wordsSceneSeek(t); wordsPanel.sceneAt = t; }
}
// A world fits its box whole: scaled by width or height, whichever is tighter, and kept fitted as
// the box changes size (the page settles after the first frame, and windows resize).
const wordsFitter = new ResizeObserver((entries) => {
  const [W, H] = wordsWorld();
  for (const { target } of entries) target.style.setProperty("--scale", String(Math.min(target.clientWidth / W, target.clientHeight / H)));
});
// The settled look of a line: every move done and every stroke drawn.
const wordsPose = (anim, lineId) => wordsLineSpan(anim, lineId).end - 0.05;

// Play the song from the line, on through the whole song: the stage follows each line as it is sung
// and the lyrics and film roll roll up past it, like film. A section with unsaved changes stays put.
function wordsPlay() {
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
    const here = animations.find((a) => { const s = wordsSpan(a); return t >= s.start - 0.4 && t < s.end; });
    const unsaved = wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length;
    if (here && here.animation_id !== wordsPanel.selected && !unsaved) { wordsPanel.selected = here.animation_id; wordsPanel.line = null; wordsPanel.item = null; wordsPanel.followed = `${here.animation_id}/playing`; wordsRender(); }
    const current = wordsCurrent(), line = wordsLines(current).findLast((l) => wordsAt(l.words[0]) <= t + 0.4);
    if (line && line.id !== wordsPanel.line) { wordsPanel.line = line.id; wordsPanel.item = null; wordsPanel.followed = `${current.animation_id}/${line.id}`; wordsRender(); }
    wordsPanel.time = t; wordsPanel.playing = true;
    wordsCanvasDraw(t); wordsNow(t);
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

function wordsSetChipVars(chip, font, weight, italic, color, scale) {
  chip.dataset.weight = String(weight);
  if (italic) chip.dataset.italic = "true";
  chip.style.setProperty("--word-font", font);
  chip.style.setProperty("--word-color", color);
  chip.style.setProperty("--word-scale", String(scale));
}

function wordsApplyItemChipStyle(chip, anim, it) {
  const font = it.font === "song" || !it.font ? wordsSongFont(anim) : it.font;
  const color = /^#[0-9a-f]{6}$/i.test(it.color) ? it.color : "currentColor";
  const scale = Math.min(1.7, Math.max(0.8, (it.size || 110) / 110)).toFixed(2);
  wordsSetChipVars(chip, font, it.weight || 400, it.italic, color, scale);
}

function wordsApplyKineticChipStyle(chip, kin, wordId) {
  const st = kin.style || {}, hero = !!kin.acts?.[wordId];
  const font = (hero ? st.hero_font : null) || st.font || "sans-serif";
  const weight = (hero ? st.hero_weight : null) || st.weight || 700;
  const color = (hero ? st.hero_color : null) || st.color || "currentColor";
  const scale = hero ? Math.min(1.7, (st.hero_size || 110) / (st.size || 64)) : 1;
  if (hero) chip.dataset.hero = "true";
  wordsSetChipVars(chip, font, weight, hero && st.hero_italic, color, scale);
}

function wordsApplyChipStyle(chip, anim, w, it) {
  if (it) wordsApplyItemChipStyle(chip, anim, it);
  if (anim.kinetic) wordsApplyKineticChipStyle(chip, anim.kinetic, w.word_id);
  if (wordsPanel.designs.has(w.line_id)) chip.dataset.changed = "";
}

function wordsTogglePickedWord(w, sameLine) {
  if (!sameLine) return [w.word_id];
  return wordsPanel.picked.includes(w.word_id)
    ? wordsPanel.picked.filter((id) => id !== w.word_id)
    : [...wordsPanel.picked, w.word_id];
}

function wordsSwitchSection(id) {
  if (id === wordsPanel.selected) return true;
  if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) {
    if (!confirm("Discard unsaved lyric changes?")) return false;
  }
  wordsPanel.designs.clear(); wordsPanel.styleEdits = {};
  wordsPanel.selected = id;
  return true;
}

function wordsChipClick(anim, w, index, event) {
  event.stopPropagation();
  const sameLine = anim.animation_id === wordsPanel.selected && wordsPanel.line === w.line_id;
  if (!wordsSwitchSection(anim.animation_id)) return;
  wordsVideo("pause"); wordsPanel.playing = false; wordsPanel.autoPlay = false;
  const nextPicked = wordsTogglePickedWord(w, sameLine && event.shiftKey);
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

function wordsApplyPreviewWindow(range, pausedTime, ensureAt) {
  wordsPanel.previewRange = range;
  wordsPanel.autoPlay = wordsPanel.loopLine;
  wordsPanel.time = wordsPanel.loopLine ? range.start : pausedTime;
  wordsRender();
  if (wordsPanel.loopLine) wordsVideoRange(); else wordsSeekSong(wordsPanel.time);
  requestAnimationFrame(() => wordsEnsureBelowVideo(ensureAt));
}

function wordsSelectLine(anim, line) {
  if (!wordsSwitchSection(anim.animation_id)) return;
  wordsVideo("pause"); wordsPanel.playing = false;
  const hero = line.words.find((w) => anim.kinetic?.acts?.[w.word_id]) || line.words[0];
  if (wordsPanel.mode === "timing") wordsPanel.mode = "lyrics";
  wordsPanel.line = line.id; wordsPanel.item = null; wordsPanel.cue = null;
  wordsPanel.picked = hero ? [hero.word_id] : [];
  if (!wordsPanel.shelf || wordsPanel.shelf === "cues") wordsPanel.shelf = "arrive";
  wordsPanel.followed = `${anim.animation_id}/${line.id}`;
  const span = wordsLineSpan(anim, line.id), lastWord = line.words.at(-1);
  const paused = Math.min(wordsSpan(anim).end - 0.35, span.end - 0.12, (lastWord ? wordsAt(lastWord) : span.start) + 0.18);
  wordsApplyPreviewWindow(wordsLineWindow(anim, line.id), paused, span.start);
}

// Lyric tags: each sung word of the line, laid on a grid; a hero word shows its lettering and move.
function wordsChip(anim, w) {
  const design = wordsDesign(anim, w.line_id), index = design.items.findIndex((it) => it.word_ids.includes(w.word_id)), it = design.items[index];
  const isPicked = wordsPanel.picked.includes(w.word_id) || (!wordsPanel.picked.length && wordsPanel.item === index);
  const on = anim.animation_id === wordsPanel.selected && wordsPanel.line === w.line_id && isPicked;
  const move = it && it.motion !== "quiet" ? WORDS_MOVES.find((m) => m[0] === it.motion)?.[1] : "";
  const at = wordsAt(w), clip = wordsClip(w.word_id, anim), dur = clip?.duration ?? Math.max(0.2, (+w.end || at + 0.35) - at);
  const chip = el("button", { class: `word lettered${on ? " active" : ""}`, "data-word-id": w.word_id,
    "data-start": at.toFixed(3), "data-end": (at + dur).toFixed(3),
    title: `${w.text} · ${wordsClock(at)}${move ? ` · ${move}` : ""} · Click to edit word act (Shift-click adds words)`,
    onclick: (event) => wordsChipClick(anim, w, index, event) }, w.text);
  wordsApplyChipStyle(chip, anim, w, it);
  return chip;
}

// The design items the picked words belong to (or the one clicked item).
function wordsPickedItems(design) {
  const picked = design.items.flatMap((it, i) => it.word_ids.some((id) => wordsPanel.picked.includes(id)) ? [i] : []);
  return picked.length ? picked : wordsPanel.item == null ? [] : [wordsPanel.item];
}

// Stretches of the song with no sung line (the intro and the breaks between sections): instrumentals.
function wordsGaps(animations) {
  const spans = animations.map(wordsSpan).sort((a, b) => a.start - b.start), gaps = [];
  let at = 0;
  for (const s of spans) { if (s.start - at > 1.5) gaps.push([at, s.start]); at = Math.max(at, s.end); }
  return gaps.map(([a, b]) => wordsPlace(el("div", { class: "words-gap", title: `No singing from ${wordsClock(a)} to ${wordsClock(b)}` },
    el("span", {}, "Instrumental"), el("span", { class: "meta" }, `${wordsClock(a)} – ${wordsClock(b)}`)), a, b - a));
}

const wordsTicks = (end) => Array.from({ length: Math.ceil(end / 10) }, (_, i) => wordsPlace(el("span", { class: "words-tick" }, wordsClock(i * 10)), i * 10));

// Left: the lyrics on the song clock, each section a band and each sung line a film bar on the roll.
function wordsTimeline(animations, end) {
  return wordsPlace(el("div", { class: "words-tl words-roll", "aria-label": "Lyrics on the song timeline" },
    ...wordsTicks(end),
    ...wordsGaps(animations),
    ...animations.map((a, idx) => wordsBand(a, "button", { "data-section": a.animation_id, onclick: () => { const first = wordsLines(a)[0]; if (first) wordsSelectLine(a, first); } }, animations[idx + 1] ? wordsSpan(animations[idx + 1]).start : Infinity)),
    ...wordsRows(animations).map(([a, j, line, at, len]) => wordsPlace(el("div", { class: `words-line${j ? "" : " first"}${a.animation_id === wordsPanel.selected && wordsPanel.line === line.id ? " active" : ""}`,
      "data-kind": a.scope?.section === "chorus" ? "chorus" : "verse", "data-animation-id": a.animation_id, "data-line-id": line.id, tabindex: "0",
      title: "Click line to focus and edit its entrance or words below the preview",
      onkeydown: (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); wordsSelectLine(a, line); } },
      onclick: () => wordsSelectLine(a, line) },
      ...line.words.map((w) => wordsChip(a, w))), at, len)),
    el("span", { class: "words-tl-head" })), 0, end);
}

// Right: independently timed visual teaching cues. Concurrent cues occupy separate lanes.
function wordsRoll(animations, end) {
  const cues = wordsPanel.read.graphics || [], laneEnds = [];
  const placed = cues.map((cue) => {
    let lane = laneEnds.findIndex((t) => t <= cue.start);
    if (lane < 0) lane = laneEnds.length;
    laneEnds[lane] = cue.end; return { cue, lane };
  });
  const frames = placed.map(({ cue, lane }) => {
    const thumb = (progress) => {
      const image = cue.kind === "drawing" ? wordsStroke(cue.stroke) : wordsOptionPreview("overlays", cue.overlay);
      image.setAttribute("class", "words-cue-preview");
      if (cue.kind === "drawing") {
        const path = image.querySelector("path"); path.setAttribute("pathLength", "1");
        path.setAttribute("stroke-dasharray", "1"); path.setAttribute("stroke-dashoffset", String(1-progress));
      }
      return image;
    };
    const handle = (edge, label) => cue.index == null && cue.kind !== "overlay" ? null : el("button", { class: `words-cue-handle ${edge}`, "aria-label": `${label} ${cue.name}`, title: `${label} · drag or use ↑ ↓ (Shift for fine timing)`,
      onpointerdown: (event) => wordsCueDrag(event, cue, edge), onclick: (event) => event.stopPropagation(),
      onkeydown: (event) => {
        if (!["ArrowUp", "ArrowDown"].includes(event.key)) return;
        event.preventDefault(); event.stopPropagation();
        if (cue.animation_id !== wordsPanel.selected) wordsSelect(cue.animation_id);
        const delta = (event.key === "ArrowUp" ? -1 : 1) * (event.shiftKey ? .01 : .1);
        const start = edge === "start" ? Math.max(0, cue.start+delta) : cue.start;
        wordsRetimeCue(cue,start,edge === "start" ? start+cue.end-cue.start : Math.max(start+.05,cue.end+delta));
      } }, edge === "start" ? "●" : "━");
    const frame = wordsPlace(el("div", { class: `words-graphic${wordsPanel.cue === cue.id ? " active" : ""}`, "data-cue-id": cue.id, role: "button", tabindex: "0",
      "aria-label": `${cue.name} at ${wordsClock(cue.start)}`, title: `${cue.name}${cue.word ? ` · “${cue.word}”` : ""} · ${wordsClock(cue.start)} – ${wordsClock(cue.end)} · Click to replay this connection`,
      onclick: () => wordsPreviewCue(cue), onkeydown: (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); wordsPreviewCue(cue); } } },
      el("div", { class: "words-cue-film" }, thumb(.35), thumb(.7), thumb(1)),
      handle("start", "Move"), handle("end", "Trim end of"),
      el("span", { class: "words-cue-caption" }, cue.name)), cue.start, cue.end - cue.start);
    frame.style.setProperty("--cue-lane", String(lane));
    frame.style.setProperty("--cue-ink", cue.color || "var(--accent)");
    return frame;
  });
  const roll = wordsPlace(el("div", { class: "words-tl words-roll", "aria-label": "Motion graphics on the song timeline" },
    ...wordsTicks(end),
    ...frames, !cues.length ? el("p", { class: "words-graphics-empty" }, "Pick a word, then add a drawing to show its meaning here.") : null,
    el("span", { class: "words-tl-head" })), 0, end);
  roll.style.setProperty("--cue-lanes", String(Math.max(1, laneEnds.length)));
  return roll;
}

// The same library path is shown in the picker, motion-graphics track and rendered video.
function wordsStroke(stroke) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg"), path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  svg.setAttribute("viewBox", stroke.view_box); svg.setAttribute("aria-hidden", "true");
  path.setAttribute("d", stroke.d); path.setAttribute("fill", "none"); path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "2"); path.setAttribute("vector-effect", "non-scaling-stroke"); path.setAttribute("stroke-linecap", "round");
  svg.appendChild(path); return svg;
}

function wordsCueIntersects(cue, animationId, start, end, wordIds) {
  return cue.animation_id === animationId && (cue.word_ids?.length
    ? cue.word_ids.some((id) => wordIds.includes(id)) : cue.start < end && cue.end > start);
}
function wordsCueWindow(anim, cue, line) {
  const sec = wordsSpan(anim);
  return { start: Math.max(sec.start, Math.min(cue.start, line.start) - .15), end: Math.min(sec.end, Math.max(cue.end, line.end)) };
}
function wordsPreviewCue(cue) {
  const anim = wordsPanel.read.document.animations.find((a) => a.animation_id === cue.animation_id);
  if (!anim || !wordsSwitchSection(anim.animation_id)) return;
  wordsVideo("pause"); wordsPanel.playing = false;
  wordsPanel.line = cue.line_id || wordsLines(anim)[0]?.id;
  const fallbackWord = wordsLines(anim).find((l) => l.id === wordsPanel.line)?.words[0]?.word_id;
  wordsPanel.picked = cue.word_ids?.length ? cue.word_ids : (fallbackWord ? [fallbackWord] : []);
  if (wordsPanel.mode === "timing") wordsPanel.mode = "lyrics";
  wordsPanel.cue = cue.id;
  wordsPanel.shelf = "cues";
  if (cue.kind === "overlay") wordsPanel.topShelf = "overlay";
  wordsPanel.followed = `${anim.animation_id}/${wordsPanel.line}`;
  const range = wordsCueWindow(anim, cue, wordsLineSpan(anim, wordsPanel.line));
  wordsApplyPreviewWindow(range, Math.min(cue.end - 0.2, (cue.draw_end ?? cue.end) - 0.05), cue.start);
}
async function wordsRetimeCue(cue, start, end) {
  const anim = wordsCurrent();
  if (!anim?.kinetic || (cue.index == null && cue.kind !== "overlay")) return;
  start = Math.max(0,start);
  end = Math.max(start + .05, end);
  wordsPanel.previewRange = wordsCueWindow(anim, { ...cue, start, end }, wordsLineSpan(anim, cue.line_id || wordsLines(anim)[0]?.id));
  wordsPanel.autoPlay = false;
  // An overlay is timed from its anchor (the engine's overlay_offset / overlay_duration).
  if (cue.kind === "overlay") return wordsKinetic({ overlay_offset: +(start - cue.anchor).toFixed(2), overlay_duration: +(end - start).toFixed(2) });
  const duration = Math.min((cue.draw_end ?? end) - cue.start, end - start);
  const draws = anim.kinetic.draws.map((d, i) => i === cue.index ? { ...d, offset: +(start - cue.anchor).toFixed(2), duration: +duration.toFixed(2), hold: +(end - start - duration).toFixed(2) } : d);
  await wordsKinetic({ draws });
}
function wordsCueDrag(event, cue, edge) {
  event.preventDefault(); event.stopPropagation(); wordsVideo("pause");
  if (cue.animation_id !== wordsPanel.selected) wordsSelect(cue.animation_id);
  if (cue.animation_id !== wordsPanel.selected) return;
  const view = $("view-design");
  view.setPointerCapture(event.pointerId);
  wordsPanel.cueDrag = { cue, before: { ...cue }, edge, y: event.clientY, scale: parseFloat(getComputedStyle(view).getPropertyValue("--song-s")) || 18 };
}
addEventListener("pointermove", (event) => {
  const d = wordsPanel.cueDrag;
  if (!d) return;
  const delta = (event.clientY - d.y) / d.scale;
  if (d.edge === "start") {
    d.cue.start = Math.max(0, Math.min(d.before.anchor + 600, Math.max(d.before.anchor - 600, d.before.start + delta)));
    d.cue.end = d.cue.start + d.before.end - d.before.start;
    if (d.before.draw_end != null) d.cue.draw_end = d.cue.start + d.before.draw_end - d.before.start;
  } else d.cue.end = Math.max(d.cue.start + .05, Math.min(d.cue.start + (d.cue.kind === "overlay" ? 600 : 30), d.before.end + delta));
  const frame = [...$("view-design").querySelectorAll(".words-graphic")].find((f) => f.dataset.cueId === d.cue.id);
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


function wordsNearestAnchor(time) {
  let nearest=null;
  for(const anim of wordsPanel.read?.document?.animations||[])for(const word of anim.words) {
    const at=wordsAt(word);
    if(!Number.isFinite(at))continue;
    if(!nearest||Math.abs(at-time)<nearest.distance)nearest={anim,word,at,distance:Math.abs(at-time)};
  }
  return nearest;
}
async function wordsApplyChangedSections(sections, project) {
  for (const id of sections) {
    if (wordsPanel.read.in_video?.includes(id)) await wordsCommand("words.apply", { animation_id: id, _expected_project: project });
  }
}

async function wordsRelinkCue(cue, target) {
  if (!target || cue.index == null) return;
  const source = wordsPanel.read.document.animations.find((a) => a.animation_id === cue.animation_id), project = state.project;
  if (!source) return;
  wordsVideo("pause");
  try {
    const result = await wordsCommand("words.link", { animation_id: source.animation_id, index: cue.index, word_id: target.word.word_id, target_animation_id: target.anim.animation_id,
      expected_source_revision: source.source_revision, expected_target_revision: target.anim.source_revision, _expected_project: project });
    if (result.conflict) { toast("These lyrics changed elsewhere; reloaded before moving the link"); return wordsOpen(); }
    await wordsApplyChangedSections(result.changed_sections, project);
    if (state.project !== project) return;
    wordsPanel.selected = result.animation_id; wordsPanel.line = target.word.line_id; wordsPanel.picked = [target.word.word_id];
    wordsPanel.cue = `${result.animation_id}-draw-${result.index}`; wordsPanel.stage = null; wordsPanel.previewRange = null; await wordsOpen();
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); if (state.project === project) await wordsOpen(); }
}
function wordsAnchorDrag(event,cue) {
  event.preventDefault();event.stopPropagation();wordsVideo("pause");
  const view=$("view-design");view.setPointerCapture(event.pointerId);wordsPanel.linkDrag={cue,target:null};
}
addEventListener("pointermove",event=>{
  const drag=wordsPanel.linkDrag;if(!drag)return;
  const view=$("view-design"),tl=view.querySelector("#words-lyrics .words-tl"),r=tl.getBoundingClientRect();
  const scale=parseFloat(getComputedStyle(view).getPropertyValue("--song-s"))||18;
  drag.target=event.clientX>=r.left-24&&event.clientX<=r.right+36?wordsNearestAnchor((event.clientY-r.top)/scale):null;
  wordsArrows(view);
});
addEventListener("pointerup",()=>{
  const drag=wordsPanel.linkDrag;if(!drag)return;wordsPanel.linkDrag=null;
  if(drag.target)wordsRelinkCue(drag.cue,drag.target);else wordsArrows($("view-design"));
});
addEventListener("pointercancel",()=>{if(wordsPanel.linkDrag){wordsPanel.linkDrag=null;wordsArrows($("view-design"));}});

function wordsLinkAnchorHandle(cue, x1, y1) {
  const handle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
  for (const [key, value] of Object.entries({ cx: x1, cy: y1, r: 7, class: "words-link-anchor", role: "button", tabindex: 0, "aria-label": `Relink ${cue.name} from ${cue.word || "this word"}` })) {
    handle.setAttribute(key, String(value));
  }
  handle.onpointerdown = (event) => wordsAnchorDrag(event, cue);
  handle.onclick = (event) => { event.stopPropagation(); wordsPreviewCue(cue); };
  handle.onkeydown = (event) => {
    if (event.key === "Enter") { event.preventDefault(); wordsPreviewCue(cue); return; }
    if (!["ArrowUp", "ArrowDown"].includes(event.key)) return;
    event.preventDefault();
    const all = wordsPanel.read.document.animations.flatMap((anim) => anim.words.map((word) => ({ anim, word, at: wordsAt(word) }))).filter((x) => Number.isFinite(x.at)).sort((a, b) => a.at - b.at);
    const index = all.findIndex((x) => x.anim.animation_id === cue.animation_id && x.word.word_id === cue.word_ids?.[0]);
    wordsRelinkCue(cue, all[index + (event.key === "ArrowUp" ? -1 : 1)]);
  };
  return handle;
}

function wordsCurveControlY(view, box, y1, y2, active) {
  const preview = view.querySelector(".words-preview, .lyric-canvas");
  if (!preview || !active) return [y1, y2];
  const floor = preview.getBoundingClientRect().bottom - box.top + view.scrollTop + 18;
  return [Math.max(y1, floor), Math.max(y2, floor)];
}

function wordsArrowNode(view, box, scale, cue, frame, line) {
  const start = Number(line.style.getPropertyValue("--at-s")), end = start + Number(line.style.getPropertyValue("--len-s"));
  const chips = [...line.querySelectorAll("[data-word-id]")];
  if (!wordsCueIntersects(cue, line.dataset.animationId, start, end, chips.map((c) => c.dataset.wordId))) return null;
  const sourceChip = chips.find((c) => cue.word_ids?.includes(c.dataset.wordId));
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
  const [cy1, cy2] = wordsCurveControlY(view, box, y1, y2, active);
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
  group.append(curve, head, notes);
  if (cue.index != null && wordsPanel.mode === "timing") group.appendChild(wordsLinkAnchorHandle(cue, x1, y1));
  return group;
}

// Connections describe meaning and timing, never matching row numbers.
function wordsArrows(view) {
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
    const cue = wordsPanel.read.graphics.find((c) => c.id === frame.dataset.cueId);
    if (!cue) continue;
    for (const line of lines) {
      const node = wordsArrowNode(view, box, scale, cue, frame, line);
      if (node) svg.appendChild(node);
    }
  }
  view.appendChild(svg);
  wordsAnimateFlow(view, wordsPanel.time);
}

// Centre, under the stage: the line's minimal lanes. Timing only; the Studio owns clip length.
function wordsLanes(anim) {
  if (!wordsPanel.line) return null;
  const span = wordsLineSpan(anim, wordsPanel.line), from = span.start - 0.4, length = span.end - from;
  const words = wordsLineWords(anim, wordsPanel.line), design = wordsDesign(anim, wordsPanel.line);
  const at = (t) => `${(100 * (t - from) / length).toFixed(3)}%`;
  const block = (text, start, duration, onclick) => {
    const node = el("button", { class: "lane-block", title: `${text} · ${wordsClock(start)}`, onclick }, text);
    node.style.setProperty("--start", at(start)); node.style.setProperty("--len", `${(100 * Math.max(0.15, duration) / length).toFixed(3)}%`);
    return node;
  };
  const lane = (key, name, blocks) => el("div", { class: "lane", "data-track": key }, el("span", { class: "lane-name" }, name),
    el("div", { class: "lane-track" }, ...blocks, el("span", { class: "lane-head" })));
  const accent = (it) => words.find((w) => w.word_id === it.accent);
  wordsPanel.lanes = el("div", { class: "words-lanes", "aria-label": "Line timing" },
    lane("text", "Sung words", words.map((w) => { const clip = wordsClip(w.word_id); return block(w.text, wordsAt(w), clip?.duration ?? (+w.end - +w.start), () => { wordsPanel.time = wordsAt(w); wordsCanvasDraw(); wordsNow(wordsPanel.time); }); })),
    lane("motion", "Accents", design.items.flatMap((it, i) => it.motion === "quiet" || !accent(it) ? [] : [block(WORDS_MOVES.find((m) => m[0] === it.motion)?.[1] || it.motion, wordsAt(accent(it)), 0.65, () => { wordsPanel.item = i; wordsPanel.tab = "move"; wordsRender(); })])),
    lane("draw", "Drawings", design.draws.flatMap((d) => d.paths.map((p) => block(d.name, span.start + p.from, p.to - p.from, () => { wordsPanel.tab = "draw"; wordsRender(); })))));
  return wordsPanel.lanes;
}

function wordsPositionArrowNotes(group, t, start, end) {
  const curve = group.querySelector(".words-arrow-curve");
  if (!curve || typeof curve.getTotalLength !== "function") return;
  const len = curve.getTotalLength();
  if (!len) return;
  const span = Math.max(0.4, end - start);
  const progress = wordsPanel.playing ? wordsClamp((t - start) / span) : (performance.now() / 2600) % 1;
  const notes = group.querySelectorAll(".words-music-note");
  notes.forEach((note, idx) => {
    const u = (progress * 1.15 + idx * 0.28) % 1;
    const pt = curve.getPointAtLength(u * len);
    const bob = Math.sin((u * 2.5 + idx * 0.4) * Math.PI * 2) * 5;
    const fade = u < 0.12 ? u / 0.12 : u > 0.88 ? (1 - u) / 0.12 : 1;
    note.setAttribute("transform", `translate(${pt.x.toFixed(1)}, ${(pt.y - 4 + bob).toFixed(1)})`);
    note.setAttribute("opacity", fade.toFixed(2));
  });
}

function wordsPickPrimaryArrow(groups, t) {
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

function wordsUpdateArrowGroup(group, primary, t) {
  const s = Number(group.dataset.start), e = Number(group.dataset.end);
  const isPrimary = group === primary;
  const flowing = isPrimary;
  const past = !isPrimary && Number.isFinite(e) && t > e + 0.2;
  group.classList.toggle("flowing", flowing);
  group.classList.toggle("primary", isPrimary);
  group.classList.toggle("past", past);
  group.classList.toggle("upcoming", !isPrimary && !past);
  for (const p of group.querySelectorAll("path")) p.classList.toggle("active", isPrimary);
  if (flowing) wordsPositionArrowNotes(group, t, s, e);
}

let wordsNoteLoopId = 0;
function wordsEnsureNoteLoop() {
  if (wordsNoteLoopId) return;
  const step = () => {
    const view = $("view-design");
    const primary = view?.classList.contains("active") ? view.querySelector("svg.words-arrows .words-arrow-group.primary") : null;
    if (!primary) { wordsNoteLoopId = 0; return; }
    if (!wordsPanel.playing) {
      wordsPositionArrowNotes(primary, wordsPanel.time, Number(primary.dataset.start) || 0, Number(primary.dataset.end) || 1);
    }
    wordsNoteLoopId = requestAnimationFrame(step);
  };
  wordsNoteLoopId = requestAnimationFrame(step);
}

function wordsUpdateGraphicFrame(frame, t) {
  const cue = wordsPanel.read?.graphics?.find((c) => c.id === frame.dataset.cueId);
  const active = !!cue && t >= cue.start - 0.05 && t <= cue.end + 0.1;
  const past = !!cue && t > cue.end + 0.25;
  frame.classList.toggle("playing", active);
  frame.classList.toggle("past", past && !active);
  if (cue?.kind === "drawing") {
    const drawSpan = Math.max(0.15, (cue.draw_end ?? cue.end) - cue.start);
    const u = active ? wordsClamp((t - cue.start) / drawSpan) : (past ? 1 : 0);
    frame.querySelectorAll(".words-cue-preview path").forEach((path, i) => {
      const reset = active ? (1 - wordsClamp(u * 0.75 + (i + 1) * 0.25)).toFixed(3) : String(+(1 - [0.35, 0.7, 1][i] * (past ? 1 : 1)).toFixed(3));
      path.setAttribute("stroke-dashoffset", reset);
    });
  }
}

function wordsAnimateFlow(view, t) {
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
  const primary = wordsPickPrimaryArrow(groups, t);
  for (const group of groups) wordsUpdateArrowGroup(group, primary, t);
  for (const frame of view.querySelectorAll(".words-roll > .words-graphic")) wordsUpdateGraphicFrame(frame, t);
  if (primary) wordsEnsureNoteLoop();
}

// The playhead, on every timeline of the page; while playing, the page keeps it in view.
function wordsNow(t) {
  const view = $("view-design"), anim = wordsCurrent();
  view?.style.setProperty("--now-s", t.toFixed(3));
  if (anim && wordsPanel.line && wordsPanel.lanes) {
    const span = wordsLineSpan(anim, wordsPanel.line), from = span.start - 0.4;
    wordsPanel.lanes.style.setProperty("--at", `${(100 * wordsClamp((t - from) / (span.end - from))).toFixed(3)}%`);
  }
  wordsAnimateFlow(view, t);
  if (wordsPanel.playing) wordsFollow(t);
}
function wordsRollGeometry(view) {
  const tl = view?.querySelector(".words-tl"), eye = view?.querySelector(".words-preview, .lyric-canvas");
  if (!tl || !eye || tl.clientHeight <= 0) return null;
  const seconds = Number(tl.style.getPropertyValue("--len-s")) || wordsSongSpan().end;
  const box = view.getBoundingClientRect(), eyeY = Math.max(120, eye.getBoundingClientRect().bottom - box.top + 18);
  view.style.setProperty("--words-playhead-y", `${Math.round(eyeY)}px`);
  return { tl, seconds, eyeY, tlTop: tl.getBoundingClientRect().top - box.top + view.scrollTop };
}
// The lyrics and the film roll move with the song: the moment t and the white playhead bar sit right
// below the video preview so the active lyric line and its musical connection curve run below the stage.
function wordsFollow(t, smooth = false) {
  const view = $("view-design"), g = wordsRollGeometry(view);
  if (!g) return;
  const target = g.tlTop + g.tl.clientHeight * t / g.seconds - g.eyeY;
  wordsPanel.programmaticScrollUntil = performance.now() + (smooth ? 450 : 90);
  view.scrollTo({ top: Math.max(0, target), behavior: smooth ? "smooth" : "instant" });
}
function wordsEnsureBelowVideo(t) {
  wordsFollow(t, true);
}

function wordsSyncViewScroll(view) {
  wordsArrows(view);
  const now = performance.now();
  if (now < (wordsPanel.programmaticScrollUntil || 0) && now >= (wordsPanel.userWheelUntil || 0)) return;
  const g = wordsRollGeometry(view);
  if (!g) return;
  const t = Math.max(0, Math.min(wordsSongSpan().end, ((view.scrollTop + g.eyeY - g.tlTop) / g.tl.clientHeight) * g.seconds));
  if (wordsPanel.playing) { wordsVideo("pause"); wordsPanel.playing = false; }
  wordsPanel.previewRange = null; wordsPanel.autoPlay = false;
  wordsPanel.userScrollingUntil = now + 220;
  wordsSeekSong(t);
  wordsFollowLivePlayback(t);
  wordsNow(t);
  wordsCanvasDraw(t);
}

const wordsCard = (on, title, does, onclick) => el("button", { class: "effect", role: "radio", "aria-checked": String(!!on), title: `${title} · ${does}`, "aria-label": title, onclick },
  el("strong", {}, title), el("span", { class: "muted" }, does));

// What the centre offers for the pick: Look, Move or Draw, for the whole line or one item.
function wordsOptions(anim) {
  const lineId = wordsPanel.line, design = wordsDesign(anim, lineId), it = design.items[wordsPanel.item], words = wordsLineWords(anim, lineId);
  const itemText = (x) => x.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ");
  const edit = (change) => wordsDesignEdit(anim, lineId, change);
  const tab = wordsPanel.tab;
  if (tab === "look" && !it) return [el("div", { class: "group-label" }, "Line designs"), el("div", { class: "words-effects" },
    wordsCard(false, "Plain line", "Every word along the bottom, quiet", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "plain")))),
    wordsCard(false, "Poster", "The longest word big and italic, the rest small around it", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "poster")))),
    wordsCard(false, "Stack", "One word per row, the last one bold and settling", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "stack")))),
    anim.design?.[lineId] || wordsPanel.designs.has(lineId) ? wordsCard(false, "Clear the design", "Back to a plain line when saved", () => { wordsPanel.designs.set(lineId, null); wordsRender(); }) : null)];
  const picks = wordsPickedItems(design);
  const each = (d, change) => picks.forEach((i) => change(d.items[i]));
  if (tab === "look") {
    const look = WORDS_LETTERING.find((l) => l[3] === (it.font || "song") && l[4] === it.weight && l[5] === it.italic)?.[0];
    const range = (label, key, min, max, unit = "") => {
      const input = el("input", { type: "range", min: String(min), max: String(max), step: "1", "aria-label": label,
        oninput: (e) => {
          const held = wordsPanel.designs.get(lineId) ?? structuredClone(design);
          each(held, (x) => { x[key] = Number(e.target.value); }); wordsPanel.designs.set(lineId, held); wordsCanvasDraw();
        }, onchange: () => wordsRender() });
      input.value = input.defaultValue = String(it[key]);
      return el("label", { class: "field-row" }, `${label} ${unit}`, input);
    };
    const colour = el("input", { type: "color", "aria-label": "Colour", onchange: (e) => edit((d) => each(d, (x) => { x.color = e.target.value; })) });
    colour.value = colour.defaultValue = /^#[0-9a-f]{6}$/i.test(it.color) ? it.color : "#000000";
    // An outline keeps a word readable over the scene.
    const outline = el("input", { type: "color", "aria-label": "Outline colour", onchange: (e) => edit((d) => each(d, (x) => { x.outline = e.target.value; })) });
    outline.value = outline.defaultValue = /^#[0-9a-f]{6}$/i.test(it.outline || "") ? it.outline : "#173b43";
    const outlined = el("input", { type: "checkbox", "aria-label": "Outline on", onchange: (e) => edit((d) => each(d, (x) => { x.outline = e.target.checked ? outline.value : null; })) });
    outlined.checked = outlined.defaultChecked = !!it.outline;
    return [el("div", { class: "group-label" }, `Lettering for “${picks.map((i) => itemText(design.items[i])).join(" ")}”`), el("div", { class: "words-effects" },
      ...WORDS_LETTERING.map(([key, name, does, font, weight, italic]) => wordsCard(look === key, name, does, () => edit((d) => each(d, (x) => Object.assign(x, { font, weight, italic })))))),
      el("div", { class: "words-head" }, el("label", { class: "field-row" }, "Colour", colour), el("label", { class: "field-row" }, outlined, "Outline", outline),
        range("Size", "size", 8, 600, "px"), range("Tilt", "rotate", -45, 45, "°"))];
  }
  if (tab === "move" && !it) return [el("div", { class: "group-label" }, "Moves in this line"), el("div", { class: "words-effects" },
    ...design.items.map((x, i) => wordsCard(false, `“${itemText(x)}”`, x.motion === "quiet" ? "Quiet" : `${WORDS_MOVES.find((m) => m[0] === x.motion)?.[1]} on “${words.find((w) => w.word_id === x.accent)?.text ?? itemText(x)}”`,
      () => { wordsPanel.item = i; wordsRender(); })))];
  if (tab === "move") {
    const accent = el("select", { "aria-label": "Accent word", onchange: (e) => edit((d) => { d.items[wordsPanel.item].accent = e.target.value || null; }) },
      el("option", { value: "" }, "the line's start"), ...it.word_ids.map((id) => el("option", { value: id }, `“${words.find((w) => w.word_id === id)?.text}”`)));
    accent.value = it.accent || "";
    return [el("div", { class: "group-label" }, `How “${itemText(it)}” moves`), el("div", { class: "words-effects" },
      ...WORDS_MOVES.map(([key, name, does]) => wordsCard(it.motion === key, name, does, () => edit((d) => {
        const x = d.items[wordsPanel.item]; x.motion = key; if (key !== "quiet" && !x.accent) x.accent = x.word_ids.at(-1);
      })))),
      el("div", { class: "words-head" }, el("label", { class: "field-row" }, "Lands on", accent))];
  }
  // draw: the line's strokes, then the library
  const near = it || design.items[0];
  return [el("div", { class: "group-label" }, "Drawings in this line · click one to remove it"), el("div", { class: "words-effects" },
    ...design.draws.map((d, i) => wordsCard(true, d.name, `draws ${wordsClock(wordsLineSpan(anim, lineId).start + d.paths[0].from)}`, () => edit((x) => { x.draws.splice(i, 1); }))),
    design.draws.length ? null : el("p", { class: "muted" }, "No drawings yet.")),
    el("div", { class: "group-label" }, `Add a drawing ${it ? `by “${itemText(it)}”` : "to this line"}`), el("div", { class: "words-effects" },
      ...WORDS_STROKES.map((s) => wordsCard(false, s.name, s.does, () => edit((d) => {
        const accentWord = words.find((w) => w.word_id === near.accent), start = (accentWord ? wordsAt(accentWord) : wordsLineSpan(anim, lineId).start) - wordsLineSpan(anim, lineId).start;
        d.draws.push({ name: s.name, view_box: s.view_box, x: Math.round(near.x), y: Math.round(near.y + near.size * 1.15), w: s.w, h: s.h,
          paths: [{ d: s.d, stroke: near.color, width: 6, from: Math.max(0, +start.toFixed(2)), to: +(Math.max(0, start) + s.draw).toFixed(2) }] });
      }))))];
}

const WORDS_VIEW_ICONS = {
  timing:'<svg viewBox="0 0 24 24"><path d="M4 5v14M20 5v14M4 9h16M4 15h16"/><circle cx="9" cy="9" r="2"/><circle cx="16" cy="15" r="2"/></svg>',
  lyrics:'<svg viewBox="0 0 24 24"><path d="M4 5h16M12 5v14M8 19h8"/></svg>',
  motion:'<svg viewBox="0 0 24 24"><path d="M3 17c4-13 8 6 13-8l4-4M14 5h6v6"/></svg>'
};
function wordsModeTabs() {
  return el("div",{id:"words-view-modes",class:"segmented",role:"tablist","aria-label":"Lyrics view"},
    ...Object.entries(WORDS_VIEW_ICONS).map(([mode,html])=>el("button",{role:"tab","data-words-mode":mode,"aria-selected":String(wordsPanel.mode===mode),
      "aria-label":`${mode[0].toUpperCase()+mode.slice(1)} view`,title:`${mode[0].toUpperCase()+mode.slice(1)} controls`,onclick:()=>wordsSetMode(mode)},
      el("span",{class:"words-view-icon",html}),mode[0].toUpperCase()+mode.slice(1))));
}
function wordsSetMode(mode) {
  if(!Object.hasOwn(WORDS_VIEW_ICONS,mode))return;
  if(mode!=="motion"){document.querySelector(".words-sketch")?.remove();document.querySelector(".words-sketch-cancel")?.remove();}
  wordsPanel.mode=mode;
  if (mode === "motion") { wordsPanel.topShelf = "overlay"; wordsPanel.shelf = "cues"; }
  else if (mode === "lyrics" && !wordsPanel.topShelf) { wordsPanel.topShelf = "scene"; wordsPanel.shelf = "acts"; }
  wordsRender();
}
function wordsPersistLinks() {
  try { localStorage.setItem("magicstudio.lyrics.links", JSON.stringify(wordsPanel.links)); } catch {}
  wordsArrows($("view-design"));
}

function wordsLinksToggleButton() {
  const shown = !wordsPanel.links.hidden;
  const label = shown ? "Hide lyric-to-cue links" : "Show lyric-to-cue links";
  return el("button", {
    class: "icon words-links-toggle",
    "aria-label": label,
    title: label,
    "aria-pressed": String(shown),
    html: '<svg viewBox="0 0 24 24"><path d="M4 17c4-10 8 4 16-10"/><circle cx="4" cy="17" r="2"/><circle cx="20" cy="7" r="2"/></svg>',
    onclick: (e) => {
      wordsPanel.links.hidden = !wordsPanel.links.hidden;
      e.currentTarget.setAttribute("aria-pressed", String(!wordsPanel.links.hidden));
      wordsPersistLinks();
    }
  });
}

function wordsLinkControls() {
  const links = wordsPanel.links;
  const colour = el("input", { type: "color", "aria-label": "Connection colour", title: "Connection colour", oninput: (e) => { links.color = e.target.value; e.target.defaultValue = e.target.value; wordsPersistLinks(); } });
  colour.value = colour.defaultValue = links.color || "#70c4ff";
  const toggle = (key, label, icon) => el("button", { class: "icon", "aria-label": label, title: label, "aria-pressed": String(links[key]), html: icon, onclick: (e) => { links[key] = !links[key]; e.currentTarget.setAttribute("aria-pressed", String(links[key])); wordsPersistLinks(); } });
  return el("div", { id: "words-link-controls", class: "transport" }, el("span", { class: "group-label" }, "Connections"), colour,
    wordsLinksToggleButton(),
    toggle("front", "Bring connections in front", '<svg viewBox="0 0 24 24"><rect x="4" y="8" width="12" height="12"/><path d="M8 4h12v12M11 13l8-8M13 5h6v6"/></svg>'),
    toggle("pulse", "Slow connection colour blend", '<svg viewBox="0 0 24 24"><path d="M3 12c3-8 6 8 9 0s6 8 9 0"/></svg>'));
}
function wordsMotionActions(anim) {
  return el("div",{class:"transport","aria-label":"Add visual cue"},
    el("button",{class:"start","aria-label":"Draw cue on scene",onclick:()=>wordsDrawCue(anim)},"Draw cue"),
    el("button",{class:"ghost","aria-label":"Add scene effect",onclick:()=>{wordsPanel.motionAdd="effect";wordsRender();}},"Add effect"));
}
function wordsDrawingEdit(anim,cue) {
  if(cue.index==null)return null;
  const draw=anim.kinetic.draws[cue.index];
  const change=patch=>wordsKinetic({draws:anim.kinetic.draws.map((d,i)=>i===cue.index?{...d,...patch}:d)});
  const field=(key,label)=>{const input=el("input",{type:"number",placeholder:"Auto","aria-label":`${label} of drawing`,onchange:e=>{
    if(e.target.value!==""&&Number.isFinite(Number(e.target.value))){e.target.defaultValue=e.target.value;change({[key]:Number(e.target.value)});}}});
    input.value=input.defaultValue=draw[key]??"";return el("label",{class:"field-row"},label,input);};
  const color=el("input",{type:"color","aria-label":"Drawing colour",onchange:e=>{e.target.defaultValue=e.target.value;change({color:e.target.value});}});
  color.value=color.defaultValue=draw.color||"#e4572e";
  return el("div",{class:"words-drawing-edit"},field("x","X"),field("y","Y"),field("width","Size"),color,
    el("button",{class:"ghost","aria-label":"Redraw selected cue",onclick:()=>wordsDrawCue(anim,cue.index)},"Redraw"));
}
function wordsDrawCue(anim,replaceIndex=null) {
  const stage=$("words-preview");if(!stage||stage.querySelector(".words-sketch"))return;
  const previous=replaceIndex==null?null:anim.kinetic?.draws?.[replaceIndex];
  const word=anim.words.find(w=>w.word_id===(previous?.word_id||wordsPanel.picked[0]))||wordsLineWords(anim,wordsPanel.line)[0];
  if(!word)return toast("Select a lyric line to attach the drawing");
  const project=state.project,size=wordsPanel.size||{width:1920,height:1080};wordsVideo("pause");
  const overlay=document.createElementNS("http://www.w3.org/2000/svg","svg"),path=document.createElementNS(overlay.namespaceURI,"path");
  overlay.setAttribute("viewBox",`0 0 ${size.width} ${size.height}`);overlay.setAttribute("class","words-sketch");
  overlay.setAttribute("tabindex","0");overlay.setAttribute("aria-label","Draw a cue on the scene; Escape cancels");
  path.setAttribute("fill","none");path.setAttribute("stroke",previous?.color||"#e4572e");path.setAttribute("stroke-width","7");path.setAttribute("stroke-linecap","round");overlay.appendChild(path);
  const cancel=el("button",{class:"words-sketch-cancel","aria-label":"Cancel drawing",onclick:()=>{overlay.remove();cancel.remove();}},"Cancel drawing");
  stage.appendChild(overlay);stage.appendChild(cancel);overlay.focus();
  let points=[],active=false,pending=false,stored=false;
  const savedIndex=replaceIndex??anim.kinetic?.draws?.length??0;
  const point=e=>{const r=overlay.getBoundingClientRect();return [Math.round(Math.max(0,Math.min(size.width,(e.clientX-r.left)*size.width/r.width))),Math.round(Math.max(0,Math.min(size.height,(e.clientY-r.top)*size.height/r.height)))];};
  const paint=()=>path.setAttribute("d",points.map((p,i)=>`${i?"L":"M"}${p[0]} ${p[1]}`).join(" "));
  overlay.onpointerdown=e=>{if(pending)return;e.preventDefault();overlay.setPointerCapture(e.pointerId);points=[point(e)];active=true;paint();};
  overlay.onpointermove=e=>{if(!active||points.length>=500)return;const p=point(e),last=points.at(-1);if(Math.hypot(p[0]-last[0],p[1]-last[1])>2){points.push(p);paint();}};
  const save=async()=>{
    if(points.length<2)return;
    if(state.project!==project||wordsPanel.selected!==anim.animation_id)return toast("Selection changed; drawing was not saved");
    pending=true;
    const drawing={...previous,word_id:word.word_id,track:"graphics",drawing:{name:"Drawing",view_box:`0 0 ${size.width} ${size.height}`,d:path.getAttribute("d")},
      x:0,y:0,width:size.width,color:previous?.color||"#e4572e",duration:previous?.duration||1.2,hold:previous?.hold??1};
    const draws=[...(anim.kinetic?.draws||[])];if(replaceIndex==null)draws.push(drawing);else draws[replaceIndex]=drawing;
    try {
      const kinetic={layout:"karaoke",entrance:"fade-up",overlay:"none",...anim.kinetic,draws};
      if(!stored){
        const result=await wordsCommand("words.save",{animation_id:anim.animation_id,expected_source_revision:anim.source_revision,kinetic,_expected_project:project});
        if(result.conflict)throw new Error("This section changed; cancel and reload before drawing again");
        anim.source_revision=result.animation.source_revision;anim.kinetic=kinetic;stored=true;
      }
      if(wordsPanel.read.in_video?.includes(anim.animation_id))await wordsCommand("words.apply",{animation_id:anim.animation_id,_expected_project:project});
      wordsPanel.cue=`${anim.animation_id}-draw-${savedIndex}`;wordsPanel.stage=null;wordsPanel.motionAdd=null;
      overlay.remove();cancel.remove();await wordsOpen();
    }catch(error){pending=false;cancel.textContent="Retry save";cancel.setAttribute("aria-label","Retry drawing save");cancel.onclick=save;toast(String(error));}
  };
  overlay.onpointerup=()=>{if(!active)return;active=false;save();};
  overlay.onpointercancel=()=>{active=false;points=[];paint();};
  overlay.onkeydown=e=>{if(e.key==="Escape"){overlay.remove();cancel.remove();}};
}

// The picked cue (drawing or effect): exact Start and End seconds and a replay, in Timing and Motion.
function wordsCueEditor(anim) {
  const cue=wordsPanel.read.graphics?.find(c=>c.id===wordsPanel.cue&&c.animation_id===anim.animation_id);
  if(!cue)return el("p",{class:"muted"},"Select a drawing or effect clip on the right. Drag its top to move it, its lower edge to trim it, or type exact seconds here.");
  const field=(label,value,apply)=>{const input=el("input",{type:"number",step:"0.01",min:"0","aria-label":`${label} of ${cue.name} (seconds)`,
    onchange:e=>{const v=Number(e.target.value);if(e.target.value!=="" && Number.isFinite(v) && v>=0){e.target.defaultValue=e.target.value;apply(v);}}});input.value=input.defaultValue=value.toFixed(2);return el("label",{class:"field-row"},label,input);};
  return el("div",{class:"words-timing-summary","data-cue-kind":cue.kind},el("strong",{},cue.name),
    field("Start",cue.start,v=>wordsRetimeCue(cue,v,v+cue.end-cue.start)),field("End",cue.end,v=>wordsRetimeCue(cue,cue.start,v)),
    el("button",{class:"icon","aria-label":"Replay linked learning moment",title:"Replay the lyric and this cue together",html:ICONS.play,onclick:()=>wordsPreviewCue(cue)}),
    wordsDrawingEdit(anim,cue),
    el("button",{class:"icon","aria-label":"Delete selected visual cue",title:"Delete this cue",onclick:()=>{wordsPanel.cue=null;wordsKinetic(cue.kind==="overlay"?{overlay:"none"}:{draws:anim.kinetic.draws.filter((_,i)=>i!==cue.index)});},html:'<svg viewBox="0 0 24 24"><path d="M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7"/></svg>'}),
    el("p",{class:"muted"},cue.word?`Linked to “${cue.word}” · drag the left point to choose another word.`:"This atmosphere effect spans the section; adjust Start/End or pick another Atmosphere above."));
}
function wordsTimingPanel(anim) {
  return el("section",{class:"words-timing","aria-label":"Timing controls"},wordsLinkControls(),wordsCueEditor(anim));
}
function wordsInnerControls(anim) {
  if(wordsPanel.mode==="timing")return [wordsTimingPanel(anim)];
  if(anim.kinetic)return [wordsLayers(anim)];
  wordsPanel.tab=wordsPanel.mode==="motion"?"draw":"look";
  return [wordsLanes(anim),...wordsOptions(anim)];
}

function wordsDesignerToolbar(anim) {
  const segmented = (label, current, options, pick) => el("div", { class: "segmented", role: "tablist", "aria-label": label },
    ...options.map(([key, name]) => el("button", { role: "tab", "aria-selected": String(current === key), onclick: () => pick(key) }, name)));
  const inVideo = wordsPanel.read.in_video?.includes(anim.animation_id);
  return el("div", { class: "words-toolbar" },
    el("button", { class: "icon", "aria-label": "Play this line", title: "Play from this line", onclick: wordsPlay, html: ICONS.play }),
    el("span", { class: "meta" }, wordsClock(wordsLineSpan(anim, wordsPanel.line).start)),
    segmented("Behind the lyrics", wordsPanel.scene ? "scene" : "paper", [["scene", "Scene"], ["paper", "Paper"]], (key) => { wordsPanel.scene = key === "scene"; wordsRender(); }),
    el("span", { class: "words-spacer" }),
    el("button", { class: "ghost", title: inVideo ? "Put the video's original lyrics back for this section" : "Replace this section's lyrics in the video with the designed lines", onclick: () => wordsApply(!inVideo) },
      inVideo ? "In the video · Restore original" : "Use in the video"));
}

function wordsHeaderMeta(anim) {
  const lines = wordsLines(anim);
  if (!lines.some((l) => l.id === wordsPanel.line)) {
    wordsPanel.line = lines[0]?.id || null;
    wordsPanel.item = null;
    if (!wordsPanel.playing) wordsPanel.time = wordsPanel.line ? wordsLineSpan(anim, wordsPanel.line).start + 0.35 : 0;
  }
  const activeLine = lines.find((l) => l.id === wordsPanel.line) || lines[0];
  if (activeLine && !wordsPanel.picked.some((id) => anim.words.some((w) => w.word_id === id))) {
    const hero = activeLine.words.find((w) => anim.kinetic?.acts?.[w.word_id]) || activeLine.words[0];
    if (hero) wordsPanel.picked = [hero.word_id];
  }
  const design = wordsPanel.line ? wordsDesign(anim, wordsPanel.line) : null, it = design?.items[wordsPanel.item];
  const lineNo = lines.findIndex((l) => l.id === wordsPanel.line) + 1;
  const itemText = it ? it.word_ids.map((id) => anim.words.find((w) => w.word_id === id)?.text).join(" ") : "";
  return { lineNo, description: `${wordsLabel(anim)} · line ${lineNo}${it ? ` · ${itemText}` : ""}` };
}

function wordsMountMain(view, anim, animations, end, parts) {
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

function wordsRender() {
  const view = $("view-design"), anim = wordsCurrent(), animations = wordsPanel.read?.document?.animations || [];
  if (!anim) { view.replaceChildren(el("p", { class: "panel-empty inline" }, "No lyric sections yet.")); return; }
  const { lineNo, description } = wordsHeaderMeta(anim);
  const end = Math.max(...animations.map((a) => wordsSpan(a).end)) + 4;
  const changes = wordsPanel.designs.size + Object.keys(wordsPanel.styleEdits).length;
  const saveLabel = changes ? `Save ${changes} change${changes > 1 ? "s" : ""} & sync Studio` : "Save & view in HyperFrames Studio";
  const save = el("button", { id: "words-save", class: "icon", title: saveLabel, "aria-label": saveLabel, onclick: () => wordsSave(!changes),
    html: '<svg viewBox="0 0 24 24"><path d="M5 3h12l4 4v14H3V3h2zM7 3v6h9V3M7 21v-8h10v8"/></svg>' });
  const designer = wordsPanel.view === "design" && wordsPanel.line && !anim.kinetic;
  wordsMountMain(view, anim, animations, end, [
    new PanelHeader({ id: "words-main-head", title: `${wordsLabel(anim)} · ${lineNo}`, description,
      actions: [wordsModeTabs(), wordsLinksToggleButton(), save] }).element,
    wordsPanel.read.reference_changed ? el("p", { class: "not-linked" }, "The timing reference changed since this was saved; check the words before saving.") : null,
    wordsUpperTracks(anim),
    designer ? wordsCanvas(anim) : wordsStage(anim),
    designer ? wordsDesignerToolbar(anim) : null,
    el("div", { id: "words-workspace-controls" }, ...wordsInnerControls(anim))]);
  view.dataset.wordsMode = wordsPanel.mode;
  view.onwheel = (event) => {
    if (event.target.closest?.("#words-workspace-controls, .words-backgrounds")) return;
    wordsPanel.userWheelUntil = performance.now() + 260;
    wordsPanel.programmaticScrollUntil = 0;
    if (wordsPanel.playing) { wordsVideo("pause"); wordsPanel.playing = false; }
  };
  view.onscroll = () => wordsSyncViewScroll(view);
  wordsViewFitter.observe(view); if ($("magic-workstation")) wordsViewFitter.observe($("magic-workstation")); wordsWidth(view);
  requestAnimationFrame(() => {
    wordsCanvasDraw(); wordsFitClock(view);
    const pick = `${wordsPanel.selected}/${wordsPanel.line}`;
    if (pick !== wordsPanel.followed && wordsPanel.line && !wordsPanel.playing) {
      wordsPanel.followed = pick;
      wordsFollow(wordsPanel.time || wordsLineSpan(anim, wordsPanel.line).start, false);
    }
    wordsArrows(view);
  });
  wordsNow(wordsPanel.time);
  wordsFit();
}

// The song clock grows until every line's tags fit in its stretch of time; timing never changes.
function wordsFitClock(view) {
  view.style.removeProperty("--song-s");
  const base = parseFloat(getComputedStyle(view).getPropertyValue("--song-s-px")) || 14;
  let scale = base;
  for (const row of view.querySelectorAll(".words-tl > .words-line")) {
    const seconds = Number(row.style.getPropertyValue("--len-s"));
    if (seconds > 0) scale = Math.max(scale, (row.scrollHeight + parseFloat(getComputedStyle(row).marginTop || "0") + 4) / seconds);
  }
  if (scale > base) view.style.setProperty("--song-s", `${Math.ceil(scale)}px`);
}

function wordsSizeInput(anim) {
  const input = el("input", { type: "range", min: "60", max: "220", step: "2", "aria-label": "Word size",
    oninput: (e) => { wordsPanel.styleEdits.size = Number(e.target.value); if (wordsPanel.styleEdits.size === (anim.style?.size ?? 120)) delete wordsPanel.styleEdits.size; wordsRender(); } });
  input.value = String(wordsPanel.styleEdits.size ?? anim.style?.size ?? 120);
  input.defaultValue = String(anim.style?.size ?? 120); // only a moved slider is an unsaved edit
  return input;
}

// Dragging a word on the stage moves it; the edit is held until Save.
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
addEventListener("keydown", (event) => {
  if (state.view !== "design" || event.key !== "Escape" || wordsPanel.item == null || /INPUT|SELECT|TEXTAREA/.test(event.target.tagName)) return;
  wordsPanel.item = null; wordsRender();
});

// The section's other style settings words.save accepts: font, colours and background.
function wordsStyleFields(anim) {
  const field = (key, label, type, fallback) => {
    const saved = anim.style?.[key] ?? fallback;
    const input = el("input", { type, "aria-label": label, oninput: (e) => {
      if (e.target.value === saved) delete wordsPanel.styleEdits[key]; else wordsPanel.styleEdits[key] = e.target.value; } ,
      onchange: () => wordsRender() });
    input.value = wordsPanel.styleEdits[key] ?? saved;
    input.defaultValue = saved; // only a changed field is an unsaved edit
    return el("label", { class: "field-row" }, label, input);
  };
  return [field("font", "Font", "text", ""), field("color", "Colour", "color", "#ffffff"), field("emphasis_color", "Emphasis", "color", "#ffd078"),
    field("outline_color", "Outline", "color", "#173b43"), field("background", "Background", "text", "")];
}

function wordsUseButton(anim) {
  const used = wordsPanel.read.in_video?.includes(anim.animation_id);
  const label = used ? "Restore original video lyrics" : "Use this lyric design in the video";
  return el("button", { class: "icon", title: label, "aria-label": label, "aria-pressed": String(!!used), onclick: () => wordsApply(!used),
    html: '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 12 3 3 7-7"/></svg>' });
}

function wordsAvailableBackdrops() {
  const seen = new Set(), list = [];
  for (const img of [...(wordsPanel.backgrounds?.images || []), ...(wordsPanel.read?.images || [])]) {
    const id = img.asset_id || img.id;
    if (!id || !img.path || seen.has(id)) continue;
    seen.add(id);
    list.push({ id, name: img.name || id, path: img.path });
  }
  return list;
}

async function wordsPickBackdrop(anim, assetId) {
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
    document.querySelector("#words-main .words-backdrop")?.replaceWith(wordsBackdropPicker(anim));
    await wordsShow(wordsPanel.show);
    wordsPanel.time = heldTime;
    wordsSeekSong(heldTime);
    if (wasPlaying) wordsVideo("play");
  } catch (error) { toast(String(error)); }
}

function wordsBackdropPicker(anim) {
  const images = wordsAvailableBackdrops();
  const isFullVideo = wordsPanel.show === "stage" || (!anim.background_custom && !anim.background_asset);
  const videoBtn = el("button", {
    class: "words-background words-background-mode",
    title: "Play full animated video scene",
    "aria-label": "Use full video scene",
    "aria-pressed": String(isFullVideo),
    onclick: () => wordsPickBackdrop(anim, null)
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
        onclick: () => wordsPickBackdrop(anim, image.id)
      }, el("img", { src: convertFileSrc(image.path), alt: image.name, loading: "lazy" })))));
}

function wordsSongSpan() {
  const animations = wordsPanel.read?.document?.animations || [];
  const studioEnds = (wordsPanel.studio?.sections || []).map((s) => Number(s.end) || 0);
  const animEnds = animations.map((a) => wordsSpan(a).end || 0);
  return { start: 0, end: Math.max(10, ...studioEnds, ...animEnds) };
}

function wordsToggleSongPlay() {
  if (wordsPanel.playing) { wordsVideo("pause"); return; }
  wordsPanel.previewRange = null;
  const full = wordsSongSpan(), o = wordsPanel.offset;
  wordsVideo("set-play-range", { startSeconds: 0, endSeconds: Math.max(1, full.end - o) });
  if (wordsPanel.time >= full.end - 0.5) wordsSeekSong(0);
  wordsVideo("play");
}

function wordsStageEmptyMessage(url) {
  if (!url) return "Start the Studio server to play the video here.";
  return wordsPanel.studio?.error
    ? `The video's lyric clips could not be read: ${wordsPanel.studio.error}`
    : "The video has no lyric clips for this section.";
}

// The stage is the finished video, kept across re-renders so editing never reloads it.
function wordsStage(anim) {
  const section = wordsStudioSection(anim) || wordsSpan(anim), songSpan = wordsSongSpan();
  const url = serverUrl();
  wordsPanel.server = url || null;
  if (!section || !url) {
    wordsPanel.stage = null;
    return el("p", { class: "panel-empty inline" }, wordsStageEmptyMessage(url));
  }
  if (wordsPanel.stage && wordsPanel.show !== "edited") {
    wordsPanel.stage.dataset.animation = anim.animation_id;
    if (wordsPanel.stageDirty) wordsShow(wordsPanel.show);
    return wordsPanel.stage;
  }
  const scrub = el("input", { type: "range", id: "words-scrub", min: "0", max: String(songSpan.end), step: "0.01", "aria-label": "Video time",
    oninput: (e) => {
      e.target.defaultValue = e.target.value;
      const t = Number(e.target.value);
      if (wordsPanel.playing) { wordsVideo("pause"); wordsPanel.playing = false; }
      wordsPanel.previewRange = null; wordsPanel.autoPlay = false;
      wordsPanel.userScrollingUntil = performance.now() + 220;
      wordsSeekSong(t);
      wordsFollowLivePlayback(t);
      wordsNow(t);
      wordsFollow(t, false);
    } });
  scrub.value = scrub.defaultValue = String(wordsPanel.time || section.start);
  const server = new URL(url), project = server.hash.split("/")[1] || "hyperframes";
  wordsPanel.player = `${server.origin}/api/projects/${project}/preview`;
  const frame = el("iframe", { id: "words-video-frame", title: "Lyric and drawing preview", allow: "autoplay" });
  frame.addEventListener("load", wordsVideoRange);
  const view = (show, label) => el("button", { role: "tab", "data-show": show, "aria-selected": String(wordsPanel.show === show), onclick: () => wordsShow(show) }, label);
  wordsPanel.stage = el("div", { class: "words-preview", "data-animation": anim.animation_id },
    anim.effects?.text ? el("div", { class: "segmented", role: "tablist", "aria-label": "What the stage shows" },
      view("still", "Scene image"), view("edited", `Edited · ${anim.effects.text.name}`)) : null,
    el("div", { class: "stage", id: "words-preview" }, frame),
    el("div", { class: "transport" },
      wordsPanel.playButton = el("button", { class: "icon", "aria-label": "Play song", title: "Play or pause the entire song", onclick: wordsToggleSongPlay, html: ICONS.play }),
      scrub,
      wordsPanel.timeLabel = el("span", { class: "meta" }, `${wordsClock(wordsPanel.time || 0)} / ${wordsClock(songSpan.end)}`), wordsUseButton(anim)));
  if (wordsPanel.show === "edited" && !anim.effects?.text) wordsPanel.show = "still";
  wordsShow(wordsPanel.show, frame);
  return wordsPanel.stage;
}

async function wordsResolveShowUrl(show, anim) {
  if (show === "edited") {
    return `${wordsPanel.player}/comp/${(await wordsCommand("words.compose", { animation_id: anim.animation_id })).src}`;
  }
  if (!["lyrics", "stage", "still"].includes(show)) return wordsPanel.player;
  const effectiveShow = show === "still" && !anim.background_asset && !wordsAvailableBackdrops().length ? "stage" : show;
  return `${wordsPanel.player}/comp/${(await wordsCommand("words.lyrics_view", { show: effectiveShow, animation_id: anim.animation_id })).src}`;
}

// The stage shows the scene's backdrop with the music and the lyrics (words.lyrics_view show=stage), the whole video,
// or the section as edited (its composition from words.compose).
async function wordsShow(show, frame = wordsPanel.stage?.querySelector("#words-video-frame") || $("words-video-frame")) {
  const anim = wordsCurrent();
  wordsPanel.show = show;
  wordsPanel.stage?.querySelectorAll('[role="tab"]').forEach((tab) => tab.setAttribute("aria-selected", String(tab.dataset.show === show)));
  if (!frame || !wordsPanel.player || !anim) return;
  wordsPanel.offset = show === "edited" ? anim.song_anchor || 0 : 0;
  try {
    const src = await wordsResolveShowUrl(show, anim);
    const cleanCurrent = (frame.getAttribute("src") || "").split("?")[0];
    if (cleanCurrent !== src || wordsPanel.stageDirty) {
      wordsPanel.stageDirty = false;
      frame.setAttribute("src", `${src}?v=${Date.now()}`);
    } else wordsVideoRange();
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
}

// Seek the stage to a song time, whichever view it shows.
function wordsSeekSong(t) {
  wordsPanel.time = t;
  wordsPanel.pendingSeek = t;
  const scrub = $("words-scrub");
  if (scrub) { scrub.value = String(t); scrub.defaultValue = scrub.value; }
  if (wordsPanel.timeLabel) wordsPanel.timeLabel.textContent = `${wordsClock(t)} / ${wordsClock(wordsSongSpan().end)}`;
  wordsVideo("seek", { timeSeconds: Math.max(0, t - wordsPanel.offset), seekMode: "commit" });
}

// On each status refresh: the stage appears once the video server is up, and goes when it stops.
function wordsStatusRefresh() {
  const url = serverUrl() || null;
  if (url === wordsPanel.server || !wordsCurrent()) return;
  wordsPanel.stage = null;
  wordsRender();
  wordsVideoRange();
}

// One control message to the video's preview player.
function wordsVideo(action, params = {}) {
  $("words-video-frame")?.contentWindow?.postMessage({ source: "hf-parent", type: "control", action, ...params }, "*");
}

// Set the player's range to the full song by default, or to previewRange when replaying a specific cue.
function wordsVideoRange() {
  const anim = wordsCurrent();
  if (!anim) return;
  const full = wordsSongSpan(), range = wordsPanel.previewRange || full;
  const o = wordsPanel.offset;
  wordsVideo("set-play-range", { startSeconds: Math.max(0, range.start - o), endSeconds: Math.max(1, range.end - o) });
  if (!wordsPanel.playing) wordsSeekSong(wordsPanel.autoPlay && wordsPanel.previewRange ? range.start : (wordsPanel.time || wordsSpan(anim).start || 0));
  if (wordsPanel.autoPlay) wordsVideo("play");
}

// The video's own size, as data; the stage rule scales the frame to fit.
function wordsFit(size = wordsPanel.size) {
  const frame = $("words-video-frame"), box = $("words-preview");
  if (!frame || !box || !size) return;
  wordsPanel.size = size;
  frame.style.setProperty("--w", `${size.width}px`); frame.style.setProperty("--h", `${size.height}px`);
  const scale = Math.min(box.clientWidth / size.width, box.clientHeight / size.height);
  frame.style.setProperty("--scale", String(scale));
  frame.style.setProperty("--frame-x", `${(box.clientWidth - size.width * scale) / 2}px`);
  frame.style.setProperty("--frame-y", `${(box.clientHeight - size.height * scale) / 2}px`);
}

function wordsFollowLivePlayback(song) {
  const animations = wordsPanel.read?.document?.animations || [];
  const unsaved = wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length;
  if (unsaved || wordsPanel.previewRange) return;
  const here = animations.find((a) => { const s = wordsSpan(a); return song >= s.start - 0.3 && song < s.end + 0.1; });
  const view = $("view-design");
  if (here && here.animation_id !== wordsPanel.selected) {
    wordsPanel.selected = here.animation_id;
    view?.querySelectorAll(".words-band[data-section]").forEach((b) => b.classList.toggle("active", b.dataset.section === here.animation_id));
    document.querySelector("#words-main .words-upper-tracks")?.replaceWith(wordsUpperTracks(here));
    $("words-workspace-controls")?.replaceChildren(...wordsInnerControls(here));
  }
  const current = wordsCurrent();
  if (!current) return;
  const line = wordsLines(current).findLast((l) => wordsAt(l.words[0]) <= song + 0.25) || wordsLines(current)[0];
  if (line && line.id !== wordsPanel.line) {
    wordsPanel.line = line.id;
    view?.querySelectorAll(".words-tl > .words-line").forEach((r) => r.classList.toggle("active", r.dataset.animationId === current.animation_id && r.dataset.lineId === line.id));
    const lineNo = wordsLines(current).findIndex((l) => l.id === line.id) + 1;
    const headTitle = $("words-main-head")?.querySelector(".title");
    if (headTitle) headTitle.textContent = `${wordsLabel(current)} · ${lineNo}`;
  }
}

function wordsHandlePreviewBoundary(song, isPlaying) {
  const range = wordsPanel.previewRange;
  if (!range || song < range.end - 0.04) return false;
  if (wordsPanel.loopLine) {
    wordsSeekSong(range.start);
    if (!isPlaying) wordsVideo("play");
    return true;
  }
  if (isPlaying) {
    wordsVideo("pause");
    wordsSeekSong(Math.max(range.start, range.end - 0.18));
    return true;
  }
  return false;
}

function wordsSyncPendingSeek(song, isPlaying) {
  if (wordsPanel.pendingSeek == null) return false;
  if (Math.abs(song - wordsPanel.pendingSeek) <= 0.35 || isPlaying) {
    wordsPanel.pendingSeek = null;
    if (wordsPanel.autoPlay && !isPlaying) wordsVideo("play");
    return false;
  }
  wordsSeekSong(wordsPanel.pendingSeek);
  if (wordsPanel.autoPlay) wordsVideo("play");
  return true;
}

function wordsHandleStateMessage(data) {
  const song = data.currentTime + wordsPanel.offset;
  if (!data.isPlaying && performance.now() < (wordsPanel.userScrollingUntil || 0)) return;
  if (wordsSyncPendingSeek(song, data.isPlaying)) return;
  wordsPanel.time = song; wordsPanel.playing = !!data.isPlaying;
  if (wordsHandlePreviewBoundary(song, data.isPlaying)) return;
  if (data.isPlaying) wordsPanel.autoPlay = false;
  if (data.isPlaying || !wordsPanel.previewRange) wordsFollowLivePlayback(song);
  const scrub = $("words-scrub");
  if (scrub) { scrub.value = String(song); scrub.defaultValue = scrub.value; }
  if (wordsPanel.timeLabel) wordsPanel.timeLabel.textContent = `${wordsClock(song)} / ${wordsClock(wordsSongSpan().end)}`;
  wordsNow(song);
  wordsPanel.playButton?.setAttribute("aria-label", data.isPlaying ? "Pause song" : "Play song");
}

// The player reports its time, play state and stage size; the transport follows it.
addEventListener("message", (event) => {
  const frame = $("words-video-frame");
  if (!frame || event.source !== frame.contentWindow || event.data?.source !== "hf-preview") return;
  const data = event.data;
  if (data.type === "stage-size") { wordsFit({ width: data.width, height: data.height }); wordsVideoRange(); }
  if (data.type === "state") wordsHandleStateMessage(data);
});
// The view's own width (not the window's) picks its layout; see #view-design.narrow / .tight.
function wordsWidth(view = $("view-design")) {
  if (!view) return;
  view.style.setProperty("--words-available", `${Math.max(220, innerHeight-Math.max(0,view.getBoundingClientRect().top)-48)}px`);
  view.classList.toggle("narrow", view.clientWidth < 1100);
  view.classList.toggle("tight", view.clientWidth < 600);
}
let wordsObservedWidth: string | number = -1;
const wordsViewFitter = new ResizeObserver(() => {
  const view=$("view-design");if(!view)return;
  const width=`${view.clientWidth}:${innerHeight}:${view.getBoundingClientRect().top}`;
  if(width===wordsObservedWidth)return; wordsObservedWidth=width;
  requestAnimationFrame(()=>{const view=$("view-design");if(view?.classList.contains("active")){wordsWidth(view);wordsFit();wordsFitClock(view);wordsArrows(view);}});
});
addEventListener("resize", () => { wordsWidth(); wordsFit(); const view = $("view-design"); if (view) wordsArrows(view); });
// Leaving the Design view stops the video, so nothing plays behind another view.
new MutationObserver(() => {
  if ($("view-design")?.classList.contains("active")) return;
  if (wordsPanel.playing) wordsVideo("pause");
})
  .observe(document.getElementById("view-design") || document.body, { attributes: true, attributeFilter: ["class"] });

// Put the section's designed lines into the video, or take them back out.
// Kinetic sections: the layers under the stage, each a row of library cards. A pick saves the section and,
// when it is in the video, composes it again so the stage shows the change.
async function wordsKinetic(change) {
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
// Shuffle (or first make kinetic) one section from the project's lyric recipe, with a fresh seed.
async function wordsShuffle(kinetic = true) {
  const anim = wordsCurrent();
  if (!anim) return;
  try {
    if(kinetic && anim.kinetic) {
      const lib=wordsPanel.read.kinetic_library||{};
      const pick=(items,current)=>{const keys=Object.keys(items||{});return keys[Math.floor(Math.random()*keys.length)]||current;};
      return wordsKinetic({layout:pick(lib.layouts,anim.kinetic.layout),entrance:pick(lib.entrances,anim.kinetic.entrance),seed:Math.floor(Math.random()*1000)});
    }
    await wordsCommand("words.design", { section: anim.animation_id, seed: Math.floor(Math.random() * 1000), kinetic });
    wordsPanel.stage = null; await wordsOpen();
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
}
// Tiny diagrams explain the choice before it is applied; motion plays only on hover/focus.
function wordsOptionPreview(kind, key) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 160 64"); svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", "words-option-preview"); svg.setAttribute("data-kind", kind); svg.setAttribute("data-choice", key);
  const add = (tag: string, attrs: Record<string, any>, text?: string) => {
    const node = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
    if (text) node.textContent = text; svg.appendChild(node); return node;
  };
  if (kind === "layouts") {
    const places = { karaoke: [[36,35],[68,35],[112,35]], stack: [[60,24],[99,24],[80,44]],
      cascade: [[28,19],[73,34],[122,49]], scatter: [[32,26,-9],[80,44,7],[126,23,-5]],
      push: [[36,35],[70,35],[116,35]], giant: [[27,18],[60,18],[87,48]] };
    (places[key] || places.karaoke).forEach(([x,y,angle], i) => add("text", { x, y, "text-anchor": "middle",
      class: i === 2 ? "hero" : "", "font-size": key === "giant" && i === 2 ? 29 : 12,
      transform: `rotate(${angle || 0} ${x} ${y})` }, ["Sing", "your", "song"][i]));
    if (key === "push") add("path", {d:"M 17 22 V 12 H 30 M 130 12 H 143 V 22 M 17 42 V 52 H 30 M 130 52 H 143 V 42", class:"cue"});
  } else if (kind === "entrances") {
    add("text", {x:80,y:40,"text-anchor":"middle","font-size":25,class:"preview-word"}, "Sing");
    const cues = { "fade-up":"M 80 59 V 47 m -4 4 4 -4 4 4", slide:"M 10 34 H 43 m -5 -4 5 4 -5 4",
      "drop-in":"M 80 5 V 17 m -4 -4 4 4 4 -4", pop:"M 38 18 l 7 5 M 122 18 l -7 5 M 38 49 l 7 -5 M 122 49 l -7 -5",
      typewriter:"M 112 20 V 43", "blur-in":"M 37 24 V 40 M 31 27 V 37 M 123 24 V 40 M 129 27 V 37" };
    add("path", {d:cues[key] || "",class:"cue"});
  } else {
    add("path", {d:"M 0 43 Q 20 31 40 43 T 80 43 T 120 43 T 160 43 M 0 54 Q 20 42 40 54 T 80 54 T 120 54 T 160 54",class:"scene"});
    if (key === "godrays") for (const x of [30,65,100,135]) add("path",{d:`M 20 0 L ${x} 64`,class:"light"});
    if (key === "lens-flare") for (const [x,y,r] of [[37,18,12],[73,30,7],[112,44,4]]) add("circle",{cx:x,cy:y,r,class:"light"});
    if (key === "ripples") for (const r of [10,21,32]) add("ellipse",{cx:80,cy:32,rx:r*1.7,ry:r*.5,class:"cue"});
    if (["snow","starfield","paper-grain","sparkle-lines"].includes(key)) for (let i=0;i<18;i++) {
      const x=(i*47+13)%160,y=(i*23+9)%60;
      if (key === "sparkle-lines") add("path",{d:`M ${x-3} ${y} h 6 M ${x} ${y-3} v 6`,class:"cue"});
      else add("circle",{cx:x,cy:y,r:key === "paper-grain" ? .7 : key === "snow" ? 2 : 1,class:"speck"});
    }
  }
  return svg;
}

function wordsBuildChoiceCard(kind, key, item, on, onclick) {
  const card = wordsCard(on, item.name, item.does, onclick);
  if (["layouts", "entrances", "overlays"].includes(kind)) {
    card.classList.add("words-diagram-card");
    card.prepend(wordsOptionPreview(kind, key));
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

function wordsMeaningShelf(anim: any, lib: any, k: any, picked: string[], text: (v: any) => string) {
  const actsFor = (act) => { const acts = { ...k.acts }; for (const id of picked) { if (act) acts[id] = act; else delete acts[id]; } return acts; };
  const groups: any[] = Object.values(Object.entries(k.acts || {}).reduce((acc: Record<string, any>, [id, act]) => {
    const key = `${text(id).toLowerCase().replace(/[^a-z]/g, "")}|${act}`;
    (acc[key] ??= { ids: [], act, word: text(id).replace(/[^\p{L}'-]/gu, "") }).ids.push(id);
    return acc;
  }, {}));
  return [
    el("div", { class: "words-meaning" }, ...groups.map((g) => el("button", {
      class: `chip${g.ids.some((id) => picked.includes(id)) ? " active" : ""}`,
      title: "Pick every time this word is sung",
      onclick: () => { wordsPanel.picked = g.ids; wordsPanel.line = anim.words.find((w) => w.word_id === g.ids[0])?.line_id; wordsRender(); }
    }, `${g.word}${g.ids.length > 1 ? ` ×${g.ids.length}` : ""} · ${lib.acts?.[g.act]?.name || g.act}`))),
    picked.length ? el("div", { class: "words-effects" },
      wordsCard(false, "No act", "Arrives like the other words", () => wordsKinetic({ acts: actsFor(null) })),
      ...Object.entries((lib.acts || {}) as Record<string, any>).map(([key, item]) => wordsCard(picked.every((id) => k.acts?.[id] === key), item.name, item.does, () => wordsKinetic({ acts: actsFor(key) }))))
      : el("p", { class: "muted" }, "Click a word in the lyrics on the left to give it an act.")
  ];
}

function wordsToggleStroke(k, lib, picked, key, trackKind) {
  const existing = k.draws || [];
  const allHave = picked.every((id) => existing.some((d) => d.word_id === id && d.stroke === key));
  if (allHave) return existing.filter((d) => !(picked.includes(d.word_id) && d.stroke === key));
  const kept = existing.filter((d) => !(picked.includes(d.word_id) && !d.drawing && (d.track || lib.strokes?.[d.stroke]?.track || "graphics") === trackKind));
  return [...kept, ...picked.map((word_id) => ({ word_id, stroke: key, track: trackKind, place: key === "down" ? "beside" : "under" }))];
}

function wordsStrokesShelf(anim, lib, k, picked, text, trackKind) {
  return [
    el("div", { class: "words-meaning" }, ...(k.draws || []).map((d, i) => ({ ...d, index: i })).filter((d) => (d.track || lib.strokes?.[d.stroke]?.track || "graphics") === trackKind).map((d) => el("button", {
      class: "chip",
      title: `Edit ${d.drawing?.name || lib.strokes?.[d.stroke]?.name || d.stroke} for ${text(d.word_id)}`,
      "aria-label": `Edit drawing for ${text(d.word_id)}`,
      onclick: () => { wordsPanel.cue = `${anim.animation_id}-draw-${d.index}`; wordsPanel.picked = [d.word_id]; wordsRender(); }
    }, `${text(d.word_id)} · ${d.drawing?.name || lib.strokes?.[d.stroke]?.name || d.stroke}`))),
    picked.length ? el("div", { class: "words-effects" }, ...Object.entries((lib.strokes || {}) as Record<string, any>).filter(([, item]) => (item.track || "graphics") === trackKind).map(([key, item]) => {
      const card = wordsCard(picked.every((id) => k.draws?.some((d) => d.word_id === id && d.stroke === key)), item.name, item.does,
        () => wordsKinetic({ draws: wordsToggleStroke(k, lib, picked, key, trackKind) }));
      const svg = wordsStroke(item); svg.setAttribute("class", "words-stroke-sample"); card.prepend(svg); return card;
    })) : null
  ];
}

function wordsChoiceRow(lib, kind, current, pick) {
  return el("div", { class: `words-effects words-effects-${kind}` },
    ...Object.entries(lib[kind] || {}).map(([key, item]) => wordsBuildChoiceCard(kind, key, item, current === key, () => pick(key))));
}

function wordsShelfTab(activeKey, key, label, value, onToggle) {
  const on = activeKey === key;
  return el("button", {
    id: `words-track-${key}`,
    class: `chip words-shelf-tab${on ? " active" : ""}`,
    "aria-pressed": String(on),
    title: on ? `Hide ${label} track` : `Open ${label} track`,
    onclick: onToggle
  }, el("span", { class: "muted" }, `${label}: `), el("strong", {}, value));
}

function wordsSelectionBar(anim, lib, k, picked) {
  const lines = wordsLines(anim), lineIdx = Math.max(0, lines.findIndex((l) => l.id === wordsPanel.line));
  const line = lines[lineIdx] || lines[0];
  if (!line) return null;
  const span = wordsLineSpan(anim, line.id);
  const cue = wordsPanel.cue ? wordsPanel.read.graphics?.find((c) => c.id === wordsPanel.cue) : null;
  const badge = cue ? `Cue: ${cue.name}` : `${wordsLabel(anim)} · Line ${lineIdx + 1}`;
  const allPicked = line.words.length > 1 && line.words.every((w) => picked.includes(w.word_id));
  return el("div", { class: "words-selection-bar", role: "region", "aria-label": "Selected line and words" },
    el("div", { class: "words-selection-meta" },
      el("strong", { class: "words-selection-badge" }, badge),
      el("span", { class: "meta" }, `${wordsClock(span.start)} – ${wordsClock(span.end)}`)),
    el("div", { class: "words-selection-words" },
      ...line.words.map((w) => {
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
      }),
      line.words.length > 1 ? el("button", {
        class: `chip words-line-word${allPicked ? " active" : ""}`,
        title: "Select all words in this line",
        onclick: () => {
          wordsPanel.cue = null; wordsPanel.picked = line.words.map((w) => w.word_id); wordsPanel.shelf = "acts";
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

function wordsUpperTracks(anim) {
  const lib: any = wordsPanel.read?.kinetic_library || {}, k = anim?.kinetic;
  if (!k) return wordsBackdropPicker(anim);
  const curLettering = Object.entries((lib.letterings || {}) as Record<string, any>).find(([, l]) => Object.entries(l.style).every(([key, value]) => k.style?.[key] === value))?.[0];
  const top = wordsPanel.topShelf;
  const pickTop = (key) => () => { wordsPanel.topShelf = wordsPanel.topShelf === key ? null : key; wordsRender(); };
  const sceneName = wordsPanel.show === "stage" || !anim.background_asset ? "Video" : (wordsAvailableBackdrops().find((b) => b.id === anim.background_asset)?.name || anim.background_asset);
  return el("section", { class: "words-layers words-upper-tracks", "aria-label": `${wordsLabel(anim)} scene and look tracks` },
    el("div", { class: "words-shelf-bar words-upper-bar", role: "toolbar", "aria-label": "Scene and look tracks" },
      wordsShelfTab(top, "scene", "Scene", sceneName, pickTop("scene")),
      wordsShelfTab(top, "lettering", "Style", lib.letterings?.[curLettering]?.name || "Custom", pickTop("lettering")),
      wordsShelfTab(top, "layout", "Layout", lib.layouts?.[k.layout]?.name || k.layout, pickTop("layout")),
      wordsShelfTab(top, "overlay", "Atmosphere", lib.overlays?.[k.overlay || "none"]?.name || (k.overlay || "None"), pickTop("overlay")),
      el("span", { class: "words-spacer" }),
      el("button", { class: "ghost", title: "Shuffle the lyric layout and entrance; keep drawings", onclick: () => wordsShuffle(true) }, "Shuffle")),
    top === "scene" ? wordsBackdropPicker(anim) : null,
    top === "lettering" ? wordsChoiceRow(lib, "letterings", curLettering, (key) => wordsKinetic({ style: lib.letterings[key].style })) : null,
    top === "layout" ? wordsChoiceRow(lib, "layouts", k.layout, (layout) => wordsKinetic({ layout })) : null,
    top === "overlay" ? wordsChoiceRow(lib, "overlays", k.overlay || "none", (overlay) => wordsKinetic({ overlay })) : null);
}

function wordsLayers(anim) {
  const lib: any = wordsPanel.read.kinetic_library || {}, k = anim.kinetic;
  const picked = wordsPanel.picked.filter((id) => anim.words.some((w) => w.word_id === id));
  const text = (id) => anim.words.find((w) => w.word_id === id)?.text || id;
  const shelf = wordsPanel.shelf;
  const pickBottom = (key) => () => { wordsPanel.shelf = wordsPanel.shelf === key ? null : key; wordsRender(); };
  const graphicsCount = (k.draws || []).filter((d) => (d.track || lib.strokes?.[d.stroke]?.track || "graphics") === "graphics").length;
  const marksCount = (k.draws || []).filter((d) => (d.track || lib.strokes?.[d.stroke]?.track || "graphics") === "lettering").length;
  return el("section", { class: "words-layers words-lower-tracks", "aria-label": `${wordsLabel(anim)} motion and meaning tracks` },
    wordsSelectionBar(anim, lib, k, picked),
    el("div", { class: "words-shelf-bar words-lower-bar", role: "toolbar", "aria-label": "Motion and meaning tracks" },
      wordsShelfTab(shelf, "arrive", "Arrive", lib.entrances?.[k.entrance]?.name || k.entrance, pickBottom("arrive")),
      wordsShelfTab(shelf, "acts", "Meaning", `${Object.keys(k.acts || {}).length} words`, pickBottom("acts")),
      wordsShelfTab(shelf, "cues", "Cues", `${graphicsCount} drawn`, pickBottom("cues")),
      wordsShelfTab(shelf, "marks", "Marks", `${marksCount}`, pickBottom("marks")),
      el("span", { class: "words-spacer" }),
      el("button", { class: "ghost", "aria-label": "Draw cue on scene", onclick: () => wordsDrawCue(anim) }, "Draw cue")),
    shelf === "arrive" ? wordsChoiceRow(lib, "entrances", k.entrance, (entrance) => wordsKinetic({ entrance })) : null,
    shelf === "acts" ? el("div", { class: "words-shelf-body" }, ...wordsMeaningShelf(anim, lib, k, picked, text)) : null,
    shelf === "cues" ? el("div", { class: "words-shelf-body" }, ...wordsStrokesShelf(anim, lib, k, picked, text, "graphics"), wordsCueEditor(anim)) : null,
    shelf === "marks" ? el("div", { class: "words-shelf-body" }, ...wordsStrokesShelf(anim, lib, k, picked, text, "lettering")) : null);
}

async function wordsApply(use) {
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

async function wordsFlushPendingEdits(anim) {
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

async function wordsSave(openStudio = false) {
  const anim = wordsCurrent();
  if (!anim) return;
  try {
    if (!(await wordsFlushPendingEdits(anim))) return;
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
