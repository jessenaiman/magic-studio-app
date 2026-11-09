// Presentation adapter: reuse the existing controls, IDs and event handlers.
// Thumbnail words always come from the artist source, never a design item's cached text.
function lyricPreviewItems(words, design) {
  const byId = new Map(words.map(word => [word.word_id, word.text]));
  const finite = (value, fallback) => Number.isFinite(value) ? value : fallback;
  return (design?.items || []).map(item => ({
    text: (item.word_ids || []).map(id => byId.get(id)).filter(Boolean).join(" "),
    x: finite(item.x, 0), y: finite(item.y, 0), size: Math.max(8, finite(item.size, 58)),
    rotate: finite(item.rotate, 0), weight: finite(item.weight, 400),
    font: item.font || "song", italic: !!item.italic, color: item.color || "#173f49",
  })).filter(item => item.text);
}

function authoringVisibility(mode, together = false) {
  return { words: together || mode !== "motion", drawings: together || mode === "motion" };
}

function authoringSongDuration(recording, project, fallback) {
  return recording?.project === project && Number.isFinite(recording.duration) && recording.duration > 0 ? recording.duration : fallback;
}

if (typeof window !== "undefined") {
  const svgNode = (tag, attrs = {}) => {
    const node = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, String(value));
    return node;
  };
  const preview = (anim, line) => {
    const visibility = authoringVisibility(wordsPanel.presentationMode, wordsPanel.previewTogether);
    const design = wordsDesign(anim, line.id);
    const svg = svgNode("svg", { viewBox: "0 0 1920 1080", class: "lyrics-composition-thumb", role: "img", "aria-label": `Layout thumbnail: ${line.words.map(word => word.text).join(" ")}` });
    const group = svgNode("g");
    for (const item of (visibility.words ? lyricPreviewItems(line.words, design) : [])) {
      const text = svgNode("text", {
        x: item.x, y: item.y, "dominant-baseline": "text-before-edge",
        "font-size": item.size, "font-family": item.font === "song" ? wordsSongFont(anim) : item.font,
        "font-weight": item.weight, "font-style": item.italic ? "italic" : "normal", fill: item.color,
        transform: `rotate(${item.rotate} ${item.x} ${item.y})`,
      });
      text.textContent = item.text;
      group.append(text);
    }
    for (const drawing of visibility.drawings ? design.draws || [] : []) {
      const box = String(drawing.view_box || "").trim().split(/\s+/).map(Number);
      if (box.length !== 4 || !box.every(Number.isFinite) || box[2] <= 0 || box[3] <= 0) continue;
      const strokeGroup = svgNode("g", { transform: `translate(${drawing.x || 0} ${drawing.y || 0}) scale(${(drawing.w || box[2]) / box[2]} ${(drawing.h || box[3]) / box[3]}) translate(${-box[0]} ${-box[1]})` });
      for (const path of drawing.paths || []) strokeGroup.append(svgNode("path", { d: path.d || "", fill: "none", stroke: path.stroke || "#173f49", "stroke-width": path.width || 2, "stroke-linecap": "round" }));
      group.append(strokeGroup);
    }
    if (!visibility.words && !design.draws?.length) {
      const empty = svgNode("text", { x:960, y:540, "text-anchor":"middle", "font-size":90, fill:"#667487" });
      empty.textContent = "No drawing"; group.append(empty);
    }
    svg.append(group);
    requestAnimationFrame(() => {
      if (!svg.isConnected) return;
      const box = group.getBBox();
      if (box.width > 0 && box.height > 0) {
        const pad = Math.max(24, box.width * .06);
        svg.setAttribute("viewBox", `${box.x - pad} ${box.y - pad} ${box.width + pad * 2} ${box.height + pad * 2}`);
      }
    });
    return svg;
  };
  const buildCard = wordsTimelineController.buildLineCard.bind(wordsTimelineController);
  wordsTimelineController.buildLineCard = (anim, index, line, at, length) => {
    const card = buildCard(anim, index, line, at, length);
    const labels = card.querySelector(".words-line-card-row");
    if (labels) labels.hidden = true;
    if (!card.id) card.id = `lyrics-moment-${anim.animation_id}-${line.id}`;
    card.setAttribute("role", "button");
    card.setAttribute("aria-label", `${line.words.map(word => word.text).join(" ")} · ${wordsClock(at)}`);
    card.append(preview(anim, line), el("div", { class: "lyrics-card-caption" },
      el("span", {}, `${wordsClock(at)} – ${wordsClock(at + length)}`),
      el("span", {}, `${(wordsDesign(anim, line.id).draws || []).length} ${(wordsDesign(anim, line.id).draws || []).length === 1 ? "drawing" : "drawings"}`)));
    return card;
  };
  const timeline = wordsTimelineController.timeline.bind(wordsTimelineController);
  wordsTimelineController.timeline = (...args) => {
    const rail = timeline(...args);
    rail.append(...[...rail.children].sort((a, b) =>
      Number(a.style.getPropertyValue("--at-s") || 0) - Number(b.style.getPropertyValue("--at-s") || 0)));
    return rail;
  };
  // The left rail is a browsing list, not a second timeline that scrubs the song on scroll.
  const geometry = wordsFlowController.rollGeometry.bind(wordsFlowController);
  wordsFlowController.rollGeometry = rail => rail?.id === "words-lyrics" ? null : geometry(rail);
  const scrollTo = wordsFlowController.scrollRailTo.bind(wordsFlowController);
  wordsFlowController.scrollRailTo = (rail, time, smooth) => {
    if (rail?.id !== "words-lyrics") return scrollTo(rail, time, smooth);
    const card = [...rail.querySelectorAll(".words-line")].find(node =>
      time >= Number(node.style.getPropertyValue("--at-s")) &&
      time < Number(node.style.getPropertyValue("--at-s")) + Number(node.style.getPropertyValue("--len-s")));
    if (!card) return;
    const r = card.getBoundingClientRect(), parent = rail.getBoundingClientRect();
    const header = rail.querySelector(".panel-head")?.getBoundingClientRect().height || 0;
    if (r.top < parent.top + header) rail.scrollTop += r.top - parent.top - header - 8;
    else if (r.bottom > parent.bottom) rail.scrollTop += r.bottom - parent.bottom + 8;
  };
  const feedback = acFeedback.snapshot.bind(acFeedback);
  acFeedback.snapshot = () => ({ ...feedback(), authoring: {
    mode: wordsPanel.presentationMode || "design", section: wordsPanel.selected, line: wordsPanel.line,
    draftDesigns: wordsPanel.designs.size, draftStyleFields: Object.keys(wordsPanel.styleEdits).length,
    sourceReadOnly: true, previewTogether: !!wordsPanel.previewTogether,
  } });
  window.setMagicAuthoringMode = mode => {
    if (!["design", "motion"].includes(mode) || wordsPanel.presentationMode === mode) return;
    wordsPanel.audio?.pause(); wordsPanel.motionVideo?.pause(); wordsPanel.playing = false;
    wordsPanel.presentationMode = mode;
    wordsPanel.previewTogether = false;
    wordsPanel.inspectorTab = mode === "motion" ? "draw" : "word";
    wordsPanel.mode = "lyrics";
    wordsPanel.drawMode = false;
    document.title = `Magic Studio — ${mode === "design" ? "Lyrics" : "Motion"}`;
    if (wordsPanel.read) wordsRender();
  };
  window.setMagicAuthoringMode(window.__magicAuthoringMode || "design");
  function addSongWaveform(lanes, anim) {
    const span = wordsLineSpan(anim, wordsPanel.line), project = state.project;
    const track = el("div", { class: "lane-track audio-waveform-track", role: "img", "aria-label": "Loading actual song waveform" }, el("span", { class: "waveform-status" }, "Loading audio…"), el("span", { class: "lane-head" }));
    lanes.prepend(el("div", { class: "lane", "data-track": "audio" }, el("span", { class: "lane-name" }, "Song audio"), track));
    void wordsCommand("song.waveform", { start: Math.max(0, span.start - .4), end: span.end, bins: 800 }).then(result => {
      if (!track.isConnected || state.project !== project) return;
      const peaks = result.peaks || [];
      if (!peaks.length) throw new Error("No waveform data");
      wordsPanel.recording = {project,source:result.source,duration:result.duration};
      const duration = authoringSongDuration(wordsPanel.recording, project, wordsSongSpan().end);
      const scrub = document.getElementById("words-scrub");
      if (scrub) scrub.max = String(duration);
      if (wordsPanel.timeLabel) wordsPanel.timeLabel.textContent = `${wordsClock(wordsPanel.time)} / ${wordsClock(duration)}`;
      const svg = svgNode("svg", { viewBox: `0 0 ${peaks.length} 32`, preserveAspectRatio: "none", class: "song-waveform" });
      const amplitude = value => Math.max(-1, Math.min(1, value));
      const top = peaks.map((pair, i) => `${i},${16 - amplitude(pair[1]) * 15}`).join(" ");
      const bottom = [...peaks].reverse().map((pair, i) => `${peaks.length - 1 - i},${16 - amplitude(pair[0]) * 15}`).join(" ");
      svg.append(svgNode("polygon", { points: `${top} ${bottom}`, fill: "currentColor" }));
      track.replaceChildren(svg, el("span", { class: "lane-head" }));
      track.setAttribute("aria-label", `Song audio waveform · ${wordsClock(result.start)} to ${wordsClock(result.end)} · decoded by FilmCraft`);
    }).catch(error => {
      if (!track.isConnected) return;
      track.replaceChildren(el("span", { class: "waveform-status" }, "Audio waveform unavailable"));
      track.title = String(error);
      track.setAttribute("aria-label", "Audio waveform unavailable");
    });
  }
  const mount = wordsViewController.mountMain.bind(wordsViewController);
  wordsViewController.mountMain = (...args) => {
    mount(...args);
    const [view, anim] = args;
    view.dataset.authoringMode = wordsPanel.presentationMode || "design";
    view.classList.add("lyrics-authoring-layout");
    const main = document.getElementById("words-main");
    const viewStatus = el("span", { id:"authoring-preview-label", class:"muted" }, wordsPanel.previewTogether ? "Combined preview · words + drawings" : wordsPanel.presentationMode === "motion" ? "Drawings only" : "Lyrics only");
    const together = el("button", { id:"authoring-preview-together", class:wordsPanel.previewTogether ? "start" : "ghost", "aria-pressed":String(!!wordsPanel.previewTogether), onclick:() => { wordsPanel.previewTogether = !wordsPanel.previewTogether; wordsRender(); } }, "Preview together");
    main.querySelector(".lyric-canvas")?.before(el("div", { class:"authoring-preview-toolbar" }, viewStatus, together));
    const effects = wordsPanel.effectsRail ||= el("aside", { id: "words-effects-rail", class: "panel", "data-scroll": "auto", "data-look": "motion", "aria-label": "Effects" });
    const content = el("div", { class: "panel-body lyrics-effects-body" });
    for (const column of main.querySelectorAll('.words-engine-col[aria-label="Motion styles and shaders"], .words-engine-col[aria-label="EffectCraft brush presets"], .words-engine-col[aria-label="EffectCraft effects"]')) content.append(column);
    // Drawing mode exposes its native brushes/stamps in the same effects panel.
    if (wordsPanel.inspectorTab === "draw") {
      for (const column of main.querySelectorAll(".words-engine-grid > .words-engine-col")) content.append(column);
    }
    // Preserve handlers and IDs, but do not present known no-op easing or guessed
    // native-preset mappings as usable controls during the visual migration.
    for (const button of main.querySelectorAll('[aria-label="Choreography layouts"] .words-typeface-pills button')) {
      button.disabled = true; button.title = "Easing is not connected to saved animation yet";
    }
    for (const button of content.querySelectorAll('[aria-label="Motion styles and shaders"] .words-typeface-pills button')) {
      button.disabled = true; button.title = "This native text preset is not connected yet";
    }
    if (wordsPanel.presentationMode === "motion") {
      const shapes = content.querySelector('[aria-label="Animated doodle stamps"]');
      if (shapes) { shapes.querySelector("strong").textContent = "Drawing shapes"; content.prepend(shapes); }
      for (const column of [...content.children]) {
        if (column === shapes) continue;
        const label = column.getAttribute("aria-label") === "EffectCraft brush presets" ? "Brush library" : "Atmosphere";
        content.append(el("details", { class: "authoring-effect-library" }, el("summary", {}, label), column));
      }
    }
    if (!content.children.length) content.append(el("p", { class: "muted" }, "Select a word or drawing to see its effects."));
    const heading = new PanelHeader({ id: "words-effects-head", title: "Effects" }).element;
    heading.classList.add("panel-head"); heading.dataset.look = "heading";
    heading.querySelector(".panel-title").textContent = wordsPanel.presentationMode === "motion" ? "Drawing effects" : "Effects";
    effects.replaceChildren(heading, content);
    view.append(effects);
    const source = document.getElementById("words-lyrics-head");
    if (source) {
      source.querySelector(".panel-title").textContent = wordsPanel.presentationMode === "motion" ? "Learning moments" : "Lyric moments";
      source.append(el("small", { class: "lyrics-source-note" }, "Artist words and actions · read-only"));
    }
    // Retain the existing cue controls in a disclosure; no synthetic cues masquerade as artwork.
    const cues = wordsPanel.rightEl;
    const disclosure = el("details", { class: "lyrics-cue-disclosure" }, el("summary", {}, "Authored drawings"));
    if (wordsPanel.read?.graphics?.length) disclosure.append(cues);
    else {
      cues.hidden = true;
      disclosure.append(el("p", { class: "muted" }, "No authored drawing cues in this section."), cues);
    }
    if (wordsPanel.read?.graphics?.length) cues.hidden = false;
    main.append(disclosure);
    if (wordsPanel.presentationMode === "design") {
      const controls = main.querySelector("#words-workspace-controls");
      if (controls && typeof nativeWordControls === "function") controls.prepend(nativeWordControls(anim));
    }
    if (wordsPanel.presentationMode === "motion") {
      main.querySelector("#words-workspace-controls")?.prepend(existingDrawingControls(anim));
      const drawing = wordsDesign(anim, wordsPanel.line).draws?.[wordsPanel.selectedDrawingIndex || 0];
      const selectedWords = wordsLineWords(anim, wordsPanel.line).filter(word => drawing?.word_ids?.includes(word.word_id));
      const linked = selectedWords.map(word => word.text).join(" ") || "No linked word";
      main.querySelector("#words-workspace-controls")?.prepend(el("p", { class: "authoring-linked-word" }, "Drawing context · ", el("strong", {}, linked)));
      const brushSettings = content.querySelector('[aria-label="EffectCraft brush presets"] .words-style-toggles');
      if (brushSettings) {
        const controls = main.querySelector("#words-workspace-controls");
        const settings = el("section", { class: "panel-body authoring-stroke-settings", "aria-label": "New stroke settings" }, el("strong", {}, "New stroke settings"), brushSettings);
        controls?.insertBefore(settings, controls.children[1] || null);
      }
    }
    // Reuse the existing timed word/accent/drawing controls and playback updates.
    const lanes = wordsLanes(anim);
    if (lanes) {
      addSongWaveform(lanes, anim);
      const span = wordsLineSpan(anim, wordsPanel.line), start = Math.max(0, span.start - .4);
      const ruler = el("div", { class: "authoring-time-ruler", "aria-hidden": "true" }, ...Array.from({ length: 7 }, (_, i) => el("span", {}, wordsClock(start + (span.end - start) * i / 6))));
      const section = el("section", { id: "authoring-timeline", class: "panel authoring-timeline", "aria-label": "Word and drawing timeline" },
        el("div", { class: "authoring-timeline-heading" }, el("strong", {}, "Selected moment"), el("span", { class: "muted" }, `${wordsClock(span.start)} – ${wordsClock(span.end)}`)), ruler, lanes);
      view.querySelector("#authoring-timeline")?.remove();
      view.append(section);
    }
  };
}
