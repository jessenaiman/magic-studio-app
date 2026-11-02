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

  private stageTetherCurve(x1: number, y1: number, x2: number, y2: number, dashed: boolean) {
    const mx = (x1 + x2) / 2;
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", `M ${x1.toFixed(1)} ${y1.toFixed(1)} C ${mx.toFixed(1)} ${y1.toFixed(1)}, ${mx.toFixed(1)} ${y2.toFixed(1)}, ${x2.toFixed(1)} ${y2.toFixed(1)}`);
    path.setAttribute("class", `words-arrow-curve active${dashed ? " stage-dashed" : " stage-solid"}`);
    if (dashed) path.setAttribute("stroke-dasharray", "6 5");
    return path;
  }

  private stageTetherDot(cx: number, cy: number, color = "var(--accent)") {
    const c = document.createElementNS("http://www.w3.org/2000/svg", "circle");
    for (const [k, v] of [["cx", cx.toFixed(1)], ["cy", cy.toFixed(1)], ["r", "6"], ["fill", color], ["stroke", "var(--bg)"], ["stroke-width", "2"]]) c.setAttribute(k, v);
    return c;
  }

  private buildStageTetherGroup(view: any, box: DOMRect) {
    const activeLine = view.querySelector(".words-tl > .words-line.active");
    const canvas = view.querySelector(".lyric-canvas");
    if (!activeLine || !canvas) return null;
    const tetherPort = activeLine.querySelector(".words-line-tether") || activeLine;
    const portRect = tetherPort.getBoundingClientRect();
    const x1 = portRect.right - box.left + view.scrollLeft, y1 = portRect.top + portRect.height / 2 - box.top + view.scrollTop;
    const items = [...canvas.querySelectorAll(".lyric-item")];
    const heroNode = canvas.querySelector(".lyric-item.active") || items.at(-1) || canvas;
    const firstNode = items[0] && items[0] !== heroNode ? items[0] : null;
    const heroRect = heroNode.getBoundingClientRect();
    const hxL = heroRect.left - box.left + view.scrollLeft, hy = heroRect.top + heroRect.height / 2 - box.top + view.scrollTop;
    const hxR = heroRect.right - box.left + view.scrollLeft;
    const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    group.setAttribute("class", "words-arrow-group active primary flowing");
    group.setAttribute("data-line-id", activeLine.dataset.lineId || "");
    group.setAttribute("data-start", "0"); group.setAttribute("data-end", "3");
    if (firstNode) {
      const fRect = firstNode.getBoundingClientRect();
      const fx = fRect.left - box.left + view.scrollLeft, fy = fRect.top + fRect.height / 2 - box.top + view.scrollTop;
      group.append(this.stageTetherCurve(x1, y1, fx, fy, true), this.stageTetherDot(fx, fy));
    }
    group.append(this.stageTetherCurve(x1, y1, hxL, hy, false), this.stageTetherDot(x1, y1), this.stageTetherDot(hxL, hy, "var(--amber)"), this.buildArrowNotesNode());
    const rightFrame = view.querySelector(".words-roll > .words-graphic.active") || view.querySelector("#words-film .words-roll");
    if (rightFrame) {
      const rRect = rightFrame.getBoundingClientRect();
      const rx = rRect.left - box.left + view.scrollLeft, ry = Math.min(rRect.bottom - 20, Math.max(rRect.top + 20, heroRect.top + heroRect.height * 0.35)) - box.top + view.scrollTop;
      group.append(this.stageTetherCurve(hxR, hy, rx, ry, false), this.stageTetherDot(rx, ry));
    }
    return group;
  }

  appendCueArrows(view: any, svg: SVGElement, box: DOMRect, frames: any[], lines: any[]) {
    const scale = parseFloat(getComputedStyle(view).getPropertyValue("--song-s")) || 18;
    const graphics = wordsPanel.read?.graphics || [];
    for (const frame of frames) {
      const cue = graphics.find((c: any) => c.id === frame.dataset.cueId);
      if (!cue) continue;
      for (const line of lines) {
        const node = this.arrowNode(view, box, scale, cue, frame, line);
        if (node) svg.appendChild(node);
      }
    }
  }

  arrows(view: any) {
    view.querySelector(":scope > svg.words-arrows")?.remove();
    if (wordsPanel.links.hidden) return;
    const lines = [...view.querySelectorAll(".words-tl > .words-line")], frames = [...view.querySelectorAll(".words-roll > .words-graphic")];
    if (!lines.length) return;
    const box = view.getBoundingClientRect(), svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("class", `words-arrows${wordsPanel.links.pulse ? " blending" : ""}${wordsPanel.links.front ? " front" : ""}`);
    svg.setAttribute("role", "group"); svg.setAttribute("aria-label", "Lyric and drawing connections");
    svg.style.setProperty("--arrows-width", `${view.scrollWidth}px`); svg.style.setProperty("--arrows-height", `${view.scrollHeight}px`);
    this.appendCueArrows(view, svg, box, frames, lines);
    if (wordsPanel.mode === "timing") {
      const stageTether = this.buildStageTetherGroup(view, box);
      if (stageTether) svg.appendChild(stageTether);
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
    for (const chip of view.querySelectorAll(".words-line .word.lettered")) {
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
    const box = view.getBoundingClientRect(), eyeY = 24;
    view.style.setProperty("--words-playhead-y", `${eyeY}px`);
    return { tl, seconds, eyeY, tlTop: tl.getBoundingClientRect().top - box.top + view.scrollTop };
  }

  follow(t: number, smooth = false) {
    const view = $("view-design"), g = this.rollGeometry(view);
    if (!g) return;
    const target = Math.max(0, g.tlTop + g.tl.clientHeight * t / g.seconds - 120);
    wordsPanel.programmaticScrollUntil = performance.now() + (smooth ? 450 : 180);
    view.scrollTo({ top: target, behavior: smooth ? "smooth" : "instant" });
  }

  ensureBelowVideo(t: number) {
    this.follow(t, true);
  }

  syncViewScroll(view: any) {
    this.arrows(view);
    const now = performance.now();
    if (now >= (wordsPanel.userWheelUntil || 0)) return;
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

  private buildTokenPill(anim: any, line: any, words: any[], it: any, idx: number, activeIdx: number) {
    const label = it.word_ids.map((id: string) => words.find((w: any) => w.word_id === id)?.text).join(" ");
    const move = it.motion && it.motion !== "quiet" ? (WORDS_MOVES.find((m) => m[0] === it.motion)?.[1] || it.motion) : null;
    const on = idx === activeIdx;
    const icon = it.motion === "pop" ? "🔥" : it.motion === "rise" ? "↗" : it.motion === "fall" ? "↘" : it.motion === "loop" ? "↻" : "⚡";
    return el("button", {
      class: `chip words-line-word${on ? " active" : ""}${move ? " has-act" : ""}`,
      "data-token-index": String(idx),
      title: `${label} · Click to edit token style, motion & marks`,
      onclick: () => {
        wordsPanel.cue = null;
        wordsPanel.item = idx;
        wordsPanel.picked = [...it.word_ids];
        const firstWord = words.find((w: any) => w.word_id === (it.accent || it.word_ids[0]));
        if (firstWord) wordsPanel.time = wordsAt(firstWord) + 0.12;
        wordsRender();
        wordsSeekSong(wordsPanel.time);
      }
    }, label, move ? el("span", { class: "words-token-move" }, `${icon} ${move}`) : null);
  }

  private resolveActiveItemIndex(design: any, picked: string[]) {
    if (wordsPanel.item != null && design.items?.[wordsPanel.item]) return wordsPanel.item;
    const byPick = (design.items || []).findIndex((it: any) => it.word_ids?.some((id: string) => picked.includes(id)));
    if (byPick >= 0) return byPick;
    const heroIdx = (design.items || []).findIndex((it: any) => it.motion && it.motion !== "quiet");
    return heroIdx >= 0 ? heroIdx : 0;
  }

  selectionBar(anim: any, _lib: any, _k: any, picked: string[]) {
    const lines = wordsLines(anim), lineIdx = Math.max(0, lines.findIndex((l) => l.id === wordsPanel.line));
    const line = lines[lineIdx] || lines[0];
    if (!line) return null;
    const design = wordsDesign(anim, line.id), words = wordsLineWords(anim, line.id);
    const activeIdx = this.resolveActiveItemIndex(design, picked);
    const hasGrouped = (design.items || []).some((it: any) => (it.word_ids?.length || 0) > 1);
    return el("div", { class: "words-selection-bar", role: "region", "aria-label": "Selected line tokens" },
      el("span", { class: "words-token-bracket", "aria-hidden": "true" }, "["),
      el("div", { class: "words-selection-words" },
        ...(design.items || []).map((it: any, idx: number) => this.buildTokenPill(anim, line, words, it, idx, activeIdx))),
      el("span", { class: "words-token-bracket", "aria-hidden": "true" }, "]"),
      el("div", { class: "words-selection-actions" },
        hasGrouped ? el("button", {
          class: "ghost", title: "Split grouped phrase into individual draggable word tokens",
          onclick: () => wordsDesignController.splitLineTokens(anim, line.id)
        }, "Split words") : null,
        el("button", {
          id: "words-replay-line", class: "ghost", title: "Play just this line's animation",
          onclick: () => wordsPreviewLine(anim, line.id, true)
        }, "▶ Replay"),
        el("button", {
          id: "words-loop-line", class: "ghost", "aria-pressed": String(wordsPanel.loopLine),
          title: "Loop this line continuously",
          onclick: () => {
            wordsPanel.loopLine = !wordsPanel.loopLine;
            if (wordsPanel.loopLine) wordsPreviewLine(anim, line.id, true);
            else wordsVideo("pause");
            wordsRender();
          }
        }, wordsPanel.loopLine ? "⟳ Looping" : "⟳ Loop")));
  }

  private colChoreography(anim: any, lineId: string) {
    const layouts: Array<[string, string, string, string]> = [
      ["cascade", "cascade", "Cascade", "Diagonal staircase"],
      ["scatter", "scatter", "Spiral / Scatter", "Tilted dynamic spread"],
      ["poster", "giant", "Zoom Blast", "Hero word giant & tilted"],
      ["stack", "stack", "Stack", "Vertical stepped column"],
      ["plain", "karaoke", "Plain", "Clean horizontal row"]
    ];
    return el("div", { class: "words-engine-col", "aria-label": "Choreography layouts" },
      el("div", { class: "words-engine-col-head" }, el("strong", {}, "Choreography")),
      el("div", { class: "words-choreo-grid" },
        ...layouts.map(([kind, previewKey, name, does], idx) => {
          const card = wordsCard(idx === (wordsPanel.choreoIdx ?? 0), name, does, () => {
            wordsPanel.choreoIdx = idx;
            wordsDesignEdit(anim, lineId, (d) => { d.items = wordsLayout(anim, lineId, kind).items; });
          });
          card.classList.add("words-diagram-card");
          card.prepend(this.optionPreview("layouts", previewKey));
          return card;
        })));
  }

  private colMotionAndMarks(anim: any, lineId: string, design: any, activeIdx: number) {
    const it = design.items?.[activeIdx] || design.items?.[0];
    const moves: Array<[string, string, string, string]> = [
      ["pop", "⚡", "Bounce / Pop", "Burst on beat"],
      ["loop", "〰", "Wave / Loop", "Continuous sway"],
      ["fall", "📐", "Kinetic Tilt", "Drop in angled"],
      ["rise", "🔥", "Fire / Impact", "Ignite upward"],
      ["east", "✨", "Neon Drift", "Glide right"],
      ["settle", "★", "Sparkle Settle", "Soft landing"]
    ];
    return el("div", { class: "words-engine-col", "aria-label": "Motion styles and shaders" },
      el("div", { class: "words-engine-col-head" }, el("strong", {}, "Motion Styles & Shaders")),
      el("div", { class: "words-motion-grid" },
        ...moves.map(([key, icon, name, does]) => el("button", {
          class: `effect words-shader-card${it?.motion === key ? " active" : ""}`,
          role: "radio", "aria-checked": String(it?.motion === key), title: `${name} · ${does}`,
          onclick: () => wordsDesignEdit(anim, lineId, (d) => {
            const target = d.items[activeIdx] || d.items[0];
            if (!target) return;
            target.motion = key;
            if (key !== "quiet" && !target.accent) target.accent = target.word_ids.at(-1);
          })
        }, el("span", { class: "words-shader-icon" }, icon), el("strong", {}, name)))));
  }

  private buildTokenSliders(lineId: string, design: any, it: any, activeIdx: number) {
    const slider = (label: string, key: "size" | "rotate", min: number, max: number, unit: string) => {
      const rawVal = Math.round(Number(it?.[key] || (key === "size" ? 88 : 0)));
      const display = key === "size" ? `${(rawVal / 80).toFixed(2)}x` : `${rawVal}${unit}`;
      const valBadge = el("span", { "data-slider-val": key, class: "words-slider-val" }, display);
      const input = el("input", {
        "data-slider": key, type: "range", min: String(min), max: String(max), step: "1", "aria-label": `Token ${label}`,
        oninput: (e: any) => {
          const v = Number(e.target.value);
          valBadge.textContent = key === "size" ? `${(v / 80).toFixed(2)}x` : `${v}${unit}`;
          const held = wordsPanel.designs.get(lineId) ?? structuredClone(design);
          if (held.items[activeIdx]) held.items[activeIdx][key] = v;
          wordsPanel.designs.set(lineId, held);
          wordsCanvasDraw();
        },
        onchange: () => wordsRender()
      });
      input.value = input.defaultValue = String(rawVal);
      return el("label", { class: "words-slider-row" }, el("span", { class: "words-slider-label" }, label), input, valBadge);
    };
    return el("div", { class: "words-sliders-box" }, slider("Scale", "size", 24, 260, "px"), slider("Tilt", "rotate", -45, 45, "°"));
  }

  private colTokenStyle(anim: any, lineId: string, design: any, activeIdx: number) {
    const it = design.items?.[activeIdx] || design.items?.[0];
    if (!it) return el("div", { class: "words-engine-col" }, el("p", { class: "muted" }, "Select a token to style."));
    const words = wordsLineWords(anim, lineId);
    const curLook = WORDS_LETTERING.find((l) => l[3] === (it.font || "song") && l[4] === it.weight && l[5] === it.italic)?.[0] || "song";
    const strokeQuick = WORDS_STROKES.filter((s) => ["Underline", "Rising swoop", "Loop", "Arrow down"].includes(s.name));
    const fill = el("input", {
      type: "color", "aria-label": "Token fill colour",
      onchange: (e: any) => wordsDesignEdit(anim, lineId, (d) => { if (d.items[activeIdx]) d.items[activeIdx].color = e.target.value; })
    });
    fill.value = fill.defaultValue = (/^#[0-9a-f]{6}$/i.test(it.color || "") ? it.color : "#f59e0b").toLowerCase();
    const outlineColor = el("input", {
      type: "color", "aria-label": "Token outline colour",
      onchange: (e: any) => wordsDesignEdit(anim, lineId, (d) => { if (d.items[activeIdx]) d.items[activeIdx].outline = e.target.value; })
    });
    outlineColor.value = outlineColor.defaultValue = (/^#[0-9a-f]{6}$/i.test(it.outline || "") ? it.outline : "#2a1205").toLowerCase();
    const outlineOn = el("input", {
      type: "checkbox", "aria-label": "Token outline enabled",
      onchange: (e: any) => wordsDesignEdit(anim, lineId, (d) => { if (d.items[activeIdx]) d.items[activeIdx].outline = e.target.checked ? outlineColor.value : null; })
    });
    outlineOn.checked = outlineOn.defaultChecked = !!it.outline;
    return el("div", { class: "words-engine-col", "aria-label": "Lettering and drawn marks" },
      el("div", { class: "words-engine-col-head" }, el("strong", {}, "Lettering & Drawn Marks")),
      el("div", { class: "words-typeface-pills" },
        ...WORDS_LETTERING.map(([key, name, does, font, weight, italic]) => el("button", {
          class: `chip words-type-pill${curLook === key ? " active" : ""}`, "aria-pressed": String(curLook === key), title: `${name} · ${does}`,
          onclick: () => wordsDesignEdit(anim, lineId, (d) => { if (d.items[activeIdx]) Object.assign(d.items[activeIdx], { font, weight, italic }); })
        }, String(name)))),
      el("div", { class: "words-marks-bar" },
        ...strokeQuick.map((s) => {
          const active = (design.draws || []).some((d: any) => d.name === s.name);
          return el("button", {
            class: `chip words-mark-pill${active ? " active" : ""}`, "aria-pressed": String(active), title: `${s.name} · ${s.does}`,
            onclick: () => wordsDesignEdit(anim, lineId, (d) => {
              const existingIdx = (d.draws || []).findIndex((x: any) => x.name === s.name);
              if (existingIdx >= 0) { d.draws.splice(existingIdx, 1); return; }
              const near = d.items[activeIdx] || d.items[0] || { x: 400, y: 480, size: 160, color: "#f59e0b", accent: null };
              const accentWord = words.find((w: any) => w.word_id === near.accent);
              const start = Math.max(0, (accentWord ? wordsAt(accentWord) : wordsLineSpan(anim, lineId).start) - wordsLineSpan(anim, lineId).start);
              d.draws.push({
                name: s.name, view_box: s.view_box, x: Math.round(near.x), y: Math.round((near.y || 480) + (near.size || 140) * 1.02), w: s.w, h: s.h,
                paths: [{ d: s.d, stroke: near.color || "#f59e0b", width: 8, from: +start.toFixed(2), to: +(start + s.draw).toFixed(2) }]
              });
            })
          }, `✎ ${s.name}`);
        })),
      el("div", { class: "words-style-toggles" },
        el("label", { class: "field-row" }, "Fill", fill),
        el("label", { class: "field-row" }, outlineOn, "Outline", outlineColor)));
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
        this.shelfTab(top, "lettering", "Style", lib.letterings?.[curLettering]?.name || "Ignite Kinetic", pickTop("lettering")),
        this.shelfTab(top, "layout", "Layout", "Dynamic Shift", pickTop("layout")),
        this.shelfTab(top, "overlay", "Atmosphere", lib.overlays?.[k.overlay || "none"]?.name || (k.overlay || "None"), pickTop("overlay")),
        el("span", { class: "words-spacer" }),
        el("button", { class: "ghost", title: "Presets & Styles", onclick: pickTop("lettering") }, "✨ Presets & Styles ▾")),
      top === "scene" ? wordsBackdropPicker(anim) : null,
      top === "lettering" ? this.choiceRow(lib, "letterings", curLettering, (key) => wordsKinetic({ style: lib.letterings[key].style })) : null,
      top === "layout" ? this.choiceRow(lib, "layouts", k.layout, (layout) => wordsKinetic({ layout })) : null,
      top === "overlay" ? this.choiceRow(lib, "overlays", k.overlay || "none", (overlay) => wordsKinetic({ overlay })) : null);
  }

  layers(anim: any) {
    const lib: any = wordsPanel.read?.kinetic_library || {}, k = anim.kinetic || {};
    const picked = wordsPanel.picked.filter((id: string) => anim.words.some((w: any) => w.word_id === id));
    const lines = wordsLines(anim), lineId = wordsPanel.line || lines[0]?.id;
    const design = lineId ? wordsDesign(anim, lineId) : { items: [], draws: [] };
    const activeIdx = this.resolveActiveItemIndex(design, picked);
    const it = design.items?.[activeIdx] || design.items?.[0];
    const words = lineId ? wordsLineWords(anim, lineId) : [];
    const tokenLabel = it ? it.word_ids.map((id: string) => words.find((w: any) => w.word_id === id)?.text).join(" ").toUpperCase() : "TOKEN";
    return el("section", { class: "words-layers words-lower-tracks", "aria-label": `${wordsLabel(anim)} animation engine inspector` },
      this.selectionBar(anim, lib, k, picked),
      el("div", { class: "words-engine-panel", role: "group", "aria-label": "Advanced Animation Engine" },
        el("div", { class: "words-engine-header" },
          el("div", { class: "words-engine-title" },
            el("strong", {}, "ADVANCED ANIMATION ENGINE"),
            el("span", { class: "words-editing-token" }, `— Editing token: "${tokenLabel}"`)),
          it ? this.buildTokenSliders(lineId, design, it, activeIdx) : null),
        el("div", { class: "words-engine-grid" },
          this.colChoreography(anim, lineId),
          this.colMotionAndMarks(anim, lineId, design, activeIdx),
          this.colTokenStyle(anim, lineId, design, activeIdx))));
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

  async renderLettering() {
    const anim = wordsCurrent();
    if (!anim) return;
    try {
      if (!(await this.flushPendingEdits(anim))) return;
      toast("Building EffectCraft project & rendering transparent lettering…");
      const res = await wordsCommand("words.letter", { render: true });
      await wordsOpen();
      toast(`Lettering rendered: ${res.render || res.project}`);
    } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
  }

  async exportFilm() {
    const anim = wordsCurrent();
    if (!anim) return;
    try {
      if (!(await this.flushPendingEdits(anim))) return;
      toast("Rendering EffectCraft lettering & assembling FilmCraft MP4…");
      await wordsCommand("words.letter", { render: true });
      const res = await wordsCommand("video.assemble", { export: true });
      await wordsOpen();
      toast(`Film exported: ${res.export || res.project}`);
    } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
  }

  async save(_openStudio = false) {
    const anim = wordsCurrent();
    if (!anim) return;
    try {
      if (!(await this.flushPendingEdits(anim))) return;
      await wordsCommand("words.letter", { render: false }).catch(() => {});
      await wordsOpen();
      toast(`Saved ${wordsLabel(anim)} choreography to EffectCraft project`);
    } catch (error) { toast(String(error)); }
  }
}

const wordsStageController = new WordsStageController();
(window as any).WordsStageController = WordsStageController;
(window as any).wordsStageController = wordsStageController;
