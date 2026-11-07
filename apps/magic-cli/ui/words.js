const wordsCard = (on, title, does, onclick) => el("button", { class: "effect", role: "radio", "aria-checked": String(!!on), title: `${title} · ${does}`, "aria-label": title, onclick }, el("strong", {}, title), el("span", { class: "muted" }, does));
function wordsOptions(anim) {
  return wordsViewController.options(anim);
}
const WORDS_VIEW_ICONS = {
  timing: '<svg viewBox="0 0 24 24"><path d="M4 5v14M20 5v14M4 9h16M4 15h16"/><circle cx="9" cy="9" r="2"/><circle cx="16" cy="15" r="2"/></svg>',
  lyrics: '<svg viewBox="0 0 24 24"><path d="M4 5h16M12 5v14M8 19h8"/></svg>',
  motion: '<svg viewBox="0 0 24 24"><path d="M3 17c4-13 8 6 13-8l4-4M14 5h6v6"/></svg>'
};
function wordsModeTabs() {
  return wordsViewController.modeTabs();
}
function wordsSetMode(mode) {
  return wordsViewController.setMode(mode);
}
function wordsPersistLinks() {
  return wordsViewController.persistLinks();
}
function wordsLinksToggleButton() {
  return wordsViewController.linksToggleButton();
}
function wordsLinkControls() {
  return wordsViewController.linkControls();
}
function wordsMotionActions(anim) {
  return wordsViewController.motionActions(anim);
}
function wordsDrawingEdit(anim, cue) {
  return wordsViewController.drawingEdit(anim, cue);
}
function wordsDrawCue(anim, replaceIndex = null) {
  return wordsViewController.drawCue(anim, replaceIndex);
}
function wordsCueEditor(anim) {
  return wordsViewController.cueEditor(anim);
}
function wordsTimingPanel(anim) {
  return wordsViewController.timingPanel(anim);
}
function wordsInnerControls(anim) {
  return wordsViewController.innerControls(anim);
}
function wordsDesignerToolbar(anim) {
  return wordsViewController.designerToolbar(anim);
}
function wordsHeaderMeta(anim) {
  return wordsViewController.headerMeta(anim);
}
function wordsMountMain(view, anim, animations, end, parts) {
  return wordsViewController.mountMain(view, anim, animations, end, parts);
}
function wordsRender() {
  return wordsViewController.render();
}
function wordsFitClock(view) {
  return wordsViewController.fitClock(view);
}
function wordsSizeInput(anim) {
  return wordsViewController.sizeInput(anim);
}
function wordsSyncLiveSliders(it) {
  if (it.size != null)
    wordsPanel.tokenSliders?.size.setValue(it.size / 80);
  if (it.rotate != null)
    wordsPanel.tokenSliders?.rotate.setValue(it.rotate);
}
addEventListener("pointermove", (event) => {
  const drag = wordsPanel.drag, anim = wordsCurrent();
  if (!drag || !anim || !wordsPanel.line)
    return;
  const design = wordsPanel.designs.get(wordsPanel.line) ?? structuredClone(wordsDesign(anim, wordsPanel.line));
  const it = design.items[drag.i];
  if (!it)
    return;
  if (drag.mode === "scale") {
    const sx = drag.corner?.includes("w") ? -1 : 1, sy = drag.corner?.includes("n") ? -1 : 1;
    const delta = (sx * (event.clientX - drag.x) + sy * (event.clientY - drag.y)) / Math.max(0.15, drag.scale);
    it.size = Math.max(24, Math.min(260, Math.round(drag.startSize + delta * 0.35)));
  } else if (drag.mode === "rotate") {
    it.rotate = Math.max(-45, Math.min(45, Math.round(drag.startRot + (event.clientX - drag.x) * 0.35)));
  } else {
    it.x = Math.round(drag.from[0] + (event.clientX - drag.x) / Math.max(0.15, drag.scale));
    it.y = Math.round(drag.from[1] + (event.clientY - drag.y) / Math.max(0.15, drag.scale));
  }
  wordsPanel.designs.set(wordsPanel.line, design);
  drag.moved = true;
  wordsSyncLiveSliders(it);
  wordsCanvasDraw();
});
addEventListener("pointerup", () => {
  const drag = wordsPanel.drag;
  wordsPanel.drag = null;
  if (drag?.moved)
    wordsRender();
});
addEventListener("keydown", (event) => {
  if (state.view !== "design" || event.key !== "Escape" || wordsPanel.item == null || /INPUT|SELECT|TEXTAREA/.test(event.target.tagName))
    return;
  wordsPanel.item = null;
  wordsRender();
});
function wordsStyleFields(anim) {
  return wordsViewController.styleFields(anim);
}
function wordsUseButton(anim) {
  return wordsViewController.useButton(anim);
}
function wordsAvailableBackdrops() {
  return wordsViewController.availableBackdrops();
}
async function wordsPickBackdrop(anim, assetId) {
  return wordsViewController.pickBackdrop(anim, assetId);
}
function wordsBackdropPicker(anim) {
  return wordsViewController.backdropPicker(anim);
}
function wordsSongSpan() {
  return wordsViewController.songSpan();
}
function wordsToggleSongPlay() {
  return wordsViewController.toggleSongPlay();
}
function wordsStageEmptyMessage(url) {
  return wordsViewController.stageEmptyMessage(url);
}
function wordsStage(anim) {
  return wordsViewController.stage(anim);
}
async function wordsResolveShowUrl(show, anim) {
  return wordsViewController.resolveShowUrl(show, anim);
}
async function wordsShow(show, frame) {
  return wordsViewController.show(show, frame);
}
function wordsSeekSong(t) {
  return wordsViewController.seekSong(t);
}
function wordsStatusRefresh() {
  return wordsViewController.statusRefresh();
}
function wordsVideo(action, params = {}) {
  return wordsViewController.video(action, params);
}
function wordsVideoRange() {
  return wordsViewController.videoRange();
}
function wordsFit(size = wordsPanel.size) {
  return wordsViewController.fit(size);
}
function wordsFollowLivePlayback(song) {
  return wordsViewController.followLivePlayback(song);
}
function wordsHandlePreviewBoundary(song, isPlaying) {
  return wordsViewController.handlePreviewBoundary(song, isPlaying);
}
function wordsSyncPendingSeek(song, isPlaying) {
  return wordsViewController.syncPendingSeek(song, isPlaying);
}
function wordsHandleStateMessage(data) {
  return wordsViewController.handleStateMessage(data);
}
addEventListener("message", (event) => {
  const frame = $("words-video-frame");
  if (!frame || event.source !== frame.contentWindow || event.data?.source !== "hf-preview")
    return;
  const data = event.data;
  if (data.type === "stage-size") {
    wordsFit({ width: data.width, height: data.height });
    wordsVideoRange();
  }
  if (data.type === "state")
    wordsHandleStateMessage(data);
});
function wordsWidth(view = $("view-design")) {
  return wordsViewController.width(view);
}
let wordsObservedWidth = -1;
const wordsViewFitter = new ResizeObserver(() => {
  const view = $("view-design");
  if (!view)
    return;
  const width = `${view.clientWidth}:${innerHeight}:${view.getBoundingClientRect().top}`;
  if (width === wordsObservedWidth)
    return;
  wordsObservedWidth = width;
  requestAnimationFrame(() => {
    const view = $("view-design");
    if (view?.classList.contains("active")) {
      wordsWidth(view);
      wordsFit();
      wordsFitClock(view);
      wordsArrows(view);
    }
  });
});
addEventListener("resize", () => {
  wordsWidth();
  wordsFit();
  const view = $("view-design");
  if (view)
    wordsArrows(view);
});
new MutationObserver(() => {
  if ($("view-design")?.classList.contains("active"))
    return;
  if (wordsPanel.playing)
    wordsVideo("pause");
}).observe(document.getElementById("view-design") || document.body, { attributes: true, attributeFilter: ["class"] });
async function wordsKinetic(change) {
  return wordsViewController.kinetic(change);
}
async function wordsShuffle(kinetic = true) {
  return wordsViewController.shuffle(kinetic);
}

class WordsViewController {
  buildLookOptions(lineId, design, it, picks, itemText, edit, each) {
    const letterings = wordsLetterings();
    const look = letterings.find((l) => l[3] === (it.font || "song") && l[4] === it.weight && l[5] === it.italic)?.[0];
    const range = (label, key, min, max, unit = "") => {
      const input = el("input", {
        type: "range",
        min: String(min),
        max: String(max),
        step: "1",
        "aria-label": label,
        oninput: (e) => {
          const held = wordsPanel.designs.get(lineId) ?? structuredClone(design);
          each(held, (x) => {
            x[key] = Number(e.target.value);
          });
          wordsPanel.designs.set(lineId, held);
          wordsCanvasDraw();
        },
        onchange: () => wordsRender()
      });
      input.value = input.defaultValue = String(it[key]);
      return el("label", { class: "field-row" }, `${label} ${unit}`, input);
    };
    const colour = el("input", { type: "color", "aria-label": "Colour", onchange: (e) => edit((d) => each(d, (x) => {
      x.color = e.target.value;
    })) });
    colour.value = colour.defaultValue = /^#[0-9a-f]{6}$/i.test(it.color) ? it.color : "#000000";
    const outline = el("input", { type: "color", "aria-label": "Outline colour", onchange: (e) => edit((d) => each(d, (x) => {
      x.outline = e.target.value;
    })) });
    outline.value = outline.defaultValue = /^#[0-9a-f]{6}$/i.test(it.outline || "") ? it.outline : "#173b43";
    const outlined = el("input", { type: "checkbox", "aria-label": "Outline on", onchange: (e) => edit((d) => each(d, (x) => {
      x.outline = e.target.checked ? outline.value : null;
    })) });
    outlined.checked = outlined.defaultChecked = !!it.outline;
    return [
      el("div", { class: "group-label" }, `Lettering for “${picks.map((i) => itemText(design.items[i])).join(" ")}”`),
      el("div", { class: "words-effects" }, ...letterings.map(([key, name, does, font, weight, italic]) => wordsCard(look === key, String(name), String(does), () => edit((d) => each(d, (x) => Object.assign(x, { font, weight, italic })))))),
      el("div", { class: "words-head" }, el("label", { class: "field-row" }, "Colour", colour), el("label", { class: "field-row" }, outlined, "Outline", outline), range("Size", "size", 8, 600, "px"), range("Tilt", "rotate", -45, 45, "°"))
    ];
  }
  options(anim) {
    const lineId = wordsPanel.line, design = wordsDesign(anim, lineId), it = design.items[wordsPanel.item], words = wordsLineWords(anim, lineId);
    const itemText = (x) => x.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ");
    const edit = (change) => wordsDesignEdit(anim, lineId, change);
    const tab = wordsPanel.tab;
    const moves = wordsMoves();
    const strokes = wordsStrokes();
    if (tab === "look" && !it)
      return [el("div", { class: "group-label" }, "Line designs"), el("div", { class: "words-effects" }, wordsCard(false, "Plain line", "Every word along the bottom, quiet", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "plain")))), wordsCard(false, "Poster", "The longest word big and italic, the rest small around it", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "poster")))), wordsCard(false, "Stack", "One word per row, the last one bold and settling", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "stack")))), anim.design?.[lineId] || wordsPanel.designs.has(lineId) ? wordsCard(false, "Clear the design", "Back to a plain line when saved", () => {
        wordsPanel.designs.set(lineId, null);
        wordsRender();
      }) : null)];
    const picks = wordsPickedItems(design);
    const each = (d, change) => picks.forEach((i) => change(d.items[i]));
    if (tab === "look")
      return this.buildLookOptions(lineId, design, it, picks, itemText, edit, each);
    if (tab === "move" && !it)
      return [el("div", { class: "group-label" }, "Moves in this line"), el("div", { class: "words-effects" }, ...design.items.map((x, i) => wordsCard(false, `“${itemText(x)}”`, x.motion === "quiet" ? "Quiet" : `${moves.find((m) => m[0] === x.motion)?.[1]} on “${words.find((w) => w.word_id === x.accent)?.text ?? itemText(x)}”`, () => {
        wordsPanel.item = i;
        wordsRender();
      })))];
    if (tab === "move") {
      const accent = el("select", { "aria-label": "Accent word", onchange: (e) => edit((d) => {
        d.items[wordsPanel.item].accent = e.target.value || null;
      }) }, el("option", { value: "" }, "the line's start"), ...it.word_ids.map((id) => el("option", { value: id }, `“${words.find((w) => w.word_id === id)?.text}”`)));
      accent.value = it.accent || "";
      return [
        el("div", { class: "group-label" }, `How “${itemText(it)}” moves`),
        el("div", { class: "words-effects" }, ...moves.map(([key, name, does]) => wordsCard(it.motion === key, String(name), String(does), () => edit((d) => {
          const x = d.items[wordsPanel.item];
          x.motion = key;
          delete x.motion_keys;
          if (key !== "quiet" && !x.accent)
            x.accent = x.word_ids.at(-1);
        })))),
        el("div", { class: "words-head" }, el("label", { class: "field-row" }, "Lands on", accent))
      ];
    }
    const near = it || design.items[0];
    return [
      el("div", { class: "group-label" }, "Drawings in this line · click one to remove it"),
      el("div", { class: "words-effects" }, ...design.draws.map((d, i) => wordsCard(true, d.name, `draws ${wordsClock(wordsLineSpan(anim, lineId).start + d.paths[0].from)}`, () => edit((x) => {
        x.draws.splice(i, 1);
      }))), design.draws.length ? null : el("p", { class: "muted" }, "No drawings yet.")),
      el("div", { class: "group-label" }, `Add a drawing ${it ? `by “${itemText(it)}”` : "to this line"}`),
      el("div", { class: "words-effects" }, ...strokes.map((s) => wordsCard(false, s.name, s.does, () => edit((d) => {
        const accentWord = words.find((w) => w.word_id === near.accent), start = (accentWord ? wordsAt(accentWord) : wordsLineSpan(anim, lineId).start) - wordsLineSpan(anim, lineId).start;
        d.draws.push({
          name: s.name,
          view_box: s.view_box,
          x: Math.round(near.x),
          y: Math.round(near.y + near.size * 1.15),
          w: s.w,
          h: s.h,
          paths: [{ d: s.d, stroke: near.color, width: 6, from: Math.max(0, +start.toFixed(2)), to: +(Math.max(0, start) + s.draw).toFixed(2) }]
        });
      }))))
    ];
  }
  modeTabs() {
    return el("div", { id: "words-view-modes", class: "segmented", role: "tablist", "aria-label": "Lyrics view" }, ...Object.entries(WORDS_VIEW_ICONS).map(([mode, html]) => el("button", {
      role: "tab",
      "data-words-mode": mode,
      "aria-selected": String(wordsPanel.mode === mode),
      "aria-label": `${mode[0].toUpperCase() + mode.slice(1)} view`,
      title: `${mode[0].toUpperCase() + mode.slice(1)} controls`,
      onclick: () => this.setMode(mode)
    }, el("span", { class: "words-view-icon", html }), mode[0].toUpperCase() + mode.slice(1))));
  }
  setMode(mode) {
    if (!Object.hasOwn(WORDS_VIEW_ICONS, mode))
      return;
    if (mode !== "motion") {
      document.querySelector(".words-sketch")?.remove();
      document.querySelector(".words-sketch-cancel")?.remove();
    }
    wordsPanel.mode = mode;
    if (mode === "motion") {
      wordsPanel.topShelf = "overlay";
      wordsPanel.shelf = "cues";
    } else if (mode === "lyrics" && !wordsPanel.topShelf) {
      wordsPanel.topShelf = "scene";
      wordsPanel.shelf = "acts";
    }
    this.render();
  }
  persistLinks() {
    try {
      localStorage.setItem("magicstudio.lyrics.links", JSON.stringify(wordsPanel.links));
    } catch {}
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
      onclick: (e) => {
        wordsPanel.links.hidden = !wordsPanel.links.hidden;
        e.currentTarget.setAttribute("aria-pressed", String(!wordsPanel.links.hidden));
        this.persistLinks();
      }
    });
  }
  linkControls() {
    const links = wordsPanel.links;
    const colour = el("input", { type: "color", "aria-label": "Connection colour", title: "Connection colour", oninput: (e) => {
      links.color = e.target.value;
      e.target.defaultValue = e.target.value;
      this.persistLinks();
    } });
    colour.value = colour.defaultValue = links.color || "#70c4ff";
    const toggle = (key, label, icon) => el("button", { class: "icon", "aria-label": label, title: label, "aria-pressed": String(links[key]), html: icon, onclick: (e) => {
      links[key] = !links[key];
      e.currentTarget.setAttribute("aria-pressed", String(links[key]));
      this.persistLinks();
    } });
    return el("div", { id: "words-link-controls", class: "transport" }, el("span", { class: "group-label" }, "Connections"), colour, this.linksToggleButton(), toggle("front", "Bring connections in front", '<svg viewBox="0 0 24 24"><rect x="4" y="8" width="12" height="12"/><path d="M8 4h12v12M11 13l8-8M13 5h6v6"/></svg>'), toggle("pulse", "Slow connection colour blend", '<svg viewBox="0 0 24 24"><path d="M3 12c3-8 6 8 9 0s6 8 9 0"/></svg>'));
  }
  motionActions(anim) {
    const library = wordsPanel.read?.kinetic_library || {};
    const effects = wordsPanel.motionAdd === "effect" ? wordsChoiceRow(library, "overlays", anim.kinetic?.overlay || "none", (overlay) => {
      wordsPanel.motionAdd = null;
      this.kinetic({ overlay });
    }) : null;
    return el("div", {}, el("div", { class: "transport", "aria-label": "Add visual cue" },
      el("button", { class: "start", "aria-label": "Draw cue on scene", onclick: () => this.drawCue(anim) }, "Draw cue"),
      el("button", { class: "ghost", "aria-expanded": String(Boolean(effects)), onclick: () => {
        wordsPanel.motionAdd = effects ? null : "effect";
        this.render();
      } }, "Scene effects")), effects);
  }
  drawingEdit(anim, cue) {
    if (cue.index == null)
      return null;
    const draw = anim.kinetic.draws[cue.index];
    const change = (patch) => this.kinetic({ draws: anim.kinetic.draws.map((d, i) => i === cue.index ? { ...d, ...patch } : d) });
    const field = (key, label) => {
      const input = el("input", {
        type: "number",
        placeholder: "Auto",
        "aria-label": `${label} of drawing`,
        onchange: (e) => {
          if (e.target.value !== "" && Number.isFinite(Number(e.target.value))) {
            e.target.defaultValue = e.target.value;
            change({ [key]: Number(e.target.value) });
          }
        }
      });
      input.value = input.defaultValue = draw[key] ?? "";
      return el("label", { class: "field-row" }, label, input);
    };
    const color = el("input", { type: "color", "aria-label": "Drawing colour", onchange: (e) => {
      e.target.defaultValue = e.target.value;
      change({ color: e.target.value });
    } });
    color.value = color.defaultValue = draw.color || "#e4572e";
    return el("div", { class: "words-drawing-edit" }, field("x", "X"), field("y", "Y"), field("width", "Size"), color, el("button", { class: "ghost", "aria-label": "Redraw selected cue", onclick: () => this.drawCue(anim, cue.index) }, "Redraw"));
  }
  drawCue(anim, replaceIndex = null) {
    const stage = $("words-preview");
    if (!stage || stage.querySelector(".words-sketch"))
      return;
    const previous = replaceIndex == null ? null : anim.kinetic?.draws?.[replaceIndex];
    const word = anim.words.find((w) => w.word_id === (previous?.word_id || wordsPanel.picked[0])) || wordsLineWords(anim, wordsPanel.line)[0];
    if (!word)
      return toast("Select a lyric line to attach the drawing");
    const project = state.project, size = wordsPanel.size || { width: 1920, height: 1080 };
    this.video("pause");
    wordsPanel.audio?.pause();
    wordsPanel.playing = false;
    wordsCanvasDraw();
    const start = wordsPanel.time;
    const overlay = document.createElementNS("http://www.w3.org/2000/svg", "svg"), path = document.createElementNS(overlay.namespaceURI, "path");
    overlay.setAttribute("viewBox", `0 0 ${size.width} ${size.height}`);
    overlay.setAttribute("class", "words-sketch");
    overlay.setAttribute("tabindex", "0");
    overlay.setAttribute("aria-label", "Draw a cue on the scene; Escape cancels");
    path.setAttribute("fill", "none");
    path.setAttribute("stroke", previous?.color || "#e4572e");
    path.setAttribute("stroke-width", "7");
    path.setAttribute("stroke-linecap", "round");
    overlay.appendChild(path);
    const cancel = el("button", { class: "words-sketch-cancel", "aria-label": "Cancel drawing", onclick: () => {
      overlay.remove();
      cancel.remove();
    } }, "Cancel drawing");
    stage.appendChild(overlay);
    stage.appendChild(cancel);
    overlay.focus();
    let points = [], active = false, pending = false, stored = false;
    const savedIndex = replaceIndex ?? anim.kinetic?.draws?.length ?? 0;
    const point = (e) => {
      const r = overlay.getBoundingClientRect();
      return [Math.round(Math.max(0, Math.min(size.width, (e.clientX - r.left) * size.width / r.width))), Math.round(Math.max(0, Math.min(size.height, (e.clientY - r.top) * size.height / r.height)))];
    };
    const paint = () => path.setAttribute("d", points.map((p, i) => `${i ? "L" : "M"}${p[0]} ${p[1]}`).join(" "));
    overlay.onpointerdown = (e) => {
      if (pending)
        return;
      e.preventDefault();
      overlay.setPointerCapture(e.pointerId);
      points = [point(e)];
      active = true;
      paint();
    };
    overlay.onpointermove = (e) => {
      if (!active || points.length >= 500)
        return;
      const p = point(e), last = points.at(-1);
      if (Math.hypot(p[0] - last[0], p[1] - last[1]) > 2) {
        points.push(p);
        paint();
      }
    };
    const save = async () => {
      if (points.length < 2)
        return;
      if (state.project !== project || wordsPanel.selected !== anim.animation_id)
        return toast("Selection changed; drawing was not saved");
      pending = true;
      const drawing = {
        ...previous,
        word_id: word.word_id,
        track: "graphics",
        drawing: { name: "Drawing", view_box: `0 0 ${size.width} ${size.height}`, d: path.getAttribute("d") },
        x: 0,
        y: 0,
        width: size.width,
        color: previous?.color || "#e4572e",
        duration: previous?.duration || 1.2,
        hold: previous?.hold ?? 1,
        ...(wordsPanel.motionWorkspace && !previous ? { offset: start - wordsAt(word) } : {}),
      };
      const draws = [...anim.kinetic?.draws || []];
      if (replaceIndex == null)
        draws.push(drawing);
      else
        draws[replaceIndex] = drawing;
      try {
        const kinetic = { layout: "karaoke", entrance: "fade-up", overlay: "none", ...anim.kinetic, draws };
        if (!stored) {
          const result = await wordsCommand("words.save", { animation_id: anim.animation_id, expected_source_revision: anim.source_revision, kinetic, _expected_project: project });
          if (result.conflict)
            throw new Error("This section changed; cancel and reload before drawing again");
          anim.source_revision = result.animation.source_revision;
          anim.kinetic = kinetic;
          stored = true;
        }
        if (wordsPanel.read.in_video?.includes(anim.animation_id))
          await wordsCommand("words.apply", { animation_id: anim.animation_id, _expected_project: project });
        wordsPanel.cue = `${anim.animation_id}-draw-${savedIndex}`;
        wordsPanel.stage = null;
        wordsPanel.motionAdd = null;
        overlay.remove();
        cancel.remove();
        await wordsOpen();
      } catch (error) {
        pending = false;
        cancel.textContent = "Retry save";
        cancel.setAttribute("aria-label", "Retry drawing save");
        cancel.onclick = save;
        toast(String(error));
      }
    };
    overlay.onpointerup = () => {
      if (!active)
        return;
      active = false;
      save();
    };
    overlay.onpointercancel = () => {
      active = false;
      points = [];
      paint();
    };
    overlay.onkeydown = (e) => {
      if (e.key === "Escape") {
        overlay.remove();
        cancel.remove();
      }
    };
  }
  cueEditor(anim) {
    const cue = wordsPanel.read.graphics?.find((c) => c.id === wordsPanel.cue && c.animation_id === anim.animation_id);
    if (!cue)
      return el("p", { class: "muted" }, "Select a drawing or effect clip on the right. Drag its top to move it, its lower edge to trim it, or type exact seconds here.");
    const field = (label, value, apply) => {
      const input = el("input", {
        type: "number",
        step: "0.01",
        min: "0",
        "aria-label": `${label} of ${cue.name} (seconds)`,
        onchange: (e) => {
          const v = Number(e.target.value);
          if (e.target.value !== "" && Number.isFinite(v) && v >= 0) {
            e.target.defaultValue = e.target.value;
            apply(v);
          }
        }
      });
      input.value = input.defaultValue = value.toFixed(2);
      return el("label", { class: "field-row" }, label, input);
    };
    return el("div", { class: "words-timing-summary", "data-cue-kind": cue.kind }, el("strong", {}, cue.name), field("Start", cue.start, (v) => wordsRetimeCue(cue, v, v + cue.end - cue.start)), field("End", cue.end, (v) => wordsRetimeCue(cue, cue.start, v)), el("button", { class: "icon", "aria-label": "Replay linked learning moment", title: "Replay the lyric and this cue together", html: ICONS.play, onclick: () => wordsPreviewCue(cue) }), this.drawingEdit(anim, cue), el("button", { class: "icon", "aria-label": "Delete selected visual cue", title: "Delete this cue", onclick: () => {
      wordsPanel.cue = null;
      this.kinetic(cue.kind === "overlay" ? { overlay: "none" } : { draws: anim.kinetic.draws.filter((_, i) => i !== cue.index) });
    }, html: '<svg viewBox="0 0 24 24"><path d="M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7"/></svg>' }), el("p", { class: "muted" }, cue.word ? `Linked to “${cue.word}” · drag the left point to choose another word.` : "This atmosphere effect spans the section; adjust Start/End or pick another Atmosphere above."));
  }
  timingPanel(anim) {
    return el("section", { class: "words-timing", "aria-label": "Timing controls" }, this.linkControls(), this.cueEditor(anim));
  }
  innerControls(anim) {
    if (wordsPanel.motionWorkspace)
      return [this.motionActions(anim), this.cueEditor(anim)];
    if (wordsPanel.mode === "timing")
      return [this.timingPanel(anim)];
    if (anim.kinetic)
      return [wordsLayers(anim)];
    wordsPanel.tab = wordsPanel.mode === "motion" ? "draw" : "look";
    return [wordsLanes(anim), ...this.options(anim)];
  }
  designerToolbar(anim) {
    const songSpan = this.songSpan(), lineSpan = wordsPanel.line ? wordsLineSpan(anim, wordsPanel.line) : { start: 0, end: songSpan.end };
    const segmented = (label, current, options, pick) => el("div", { class: "segmented", role: "tablist", "aria-label": label }, ...options.map(([key, name]) => el("button", { role: "tab", "aria-selected": String(current === key), onclick: () => pick(key) }, name)));
    const scrub = el("input", {
      type: "range",
      id: "words-scrub",
      min: "0",
      max: String(songSpan.end),
      step: "0.01",
      "aria-label": "Song time",
      oninput: (e) => {
        e.target.defaultValue = e.target.value;
        const t = Number(e.target.value);
        if (wordsPanel.playing) {
          this.video("pause");
          wordsPanel.audio?.pause();
          wordsPanel.playing = false;
        }
        wordsPanel.previewRange = null;
        wordsPanel.autoPlay = false;
        wordsPanel.userScrollingUntil = performance.now() + 220;
        this.seekSong(t);
        this.followLivePlayback(t);
        wordsCanvasDraw(t);
        wordsNow(t);
        wordsFollow(t, false);
      }
    });
    scrub.value = scrub.defaultValue = String(wordsPanel.time || lineSpan.start);
    const transport = [wordsPanel.playButton = el("button", { class: "icon", "aria-label": "Play or pause", title: "Play or pause", onclick: wordsPlay, html: ICONS.play }), scrub, wordsPanel.timeLabel = el("span", { class: "meta" }, `${wordsClock(wordsPanel.time || lineSpan.start)} / ${wordsClock(songSpan.end)}`)];
    if (wordsPanel.motionWorkspace)
      return el("div", { class: "words-toolbar transport" }, transport);
    return el("div", { class: "words-toolbar transport" }, transport, segmented("Behind the lyrics", wordsPanel.scene ? "scene" : "paper", [["scene", "Scene"], ["paper", "Paper"]], (key) => {
      wordsPanel.scene = key === "scene";
      this.render();
    }), el("button", {
      class: "ghost",
      title: "Build EffectCraft project (lettering/lyrics.ecproj) and render transparent lyrics.mov",
      onclick: () => wordsStageController.renderLettering()
    }, state.lyricsOnly ? "Render Lettering" : "Render Lyrics to Studio"), state.lyricsOnly ? null : el("button", {
      class: "start",
      title: "Export the saved FilmCraft timeline or assemble a new one",
      onclick: () => wordsStageController.exportFilm()
    }, "Export Film"));
  }
  headerMeta(anim) {
    const lines = wordsLines(anim);
    if (!lines.some((l) => l.id === wordsPanel.line)) {
      wordsPanel.line = lines[0]?.id || null;
      wordsPanel.item = null;
      if (!wordsPanel.playing)
        wordsPanel.time = wordsPanel.line ? wordsLineSpan(anim, wordsPanel.line).start + 0.35 : 0;
    }
    const activeLine = lines.find((l) => l.id === wordsPanel.line) || lines[0];
    if (activeLine && !wordsPanel.picked.some((id) => anim.words.some((w) => w.word_id === id))) {
      const hero = activeLine.words.find((w) => anim.kinetic?.acts?.[w.word_id]) || activeLine.words[0];
      if (hero)
        wordsPanel.picked = [hero.word_id];
    }
    const design = wordsPanel.line ? wordsDesign(anim, wordsPanel.line) : null, it = design?.items[wordsPanel.item];
    const lineNo = lines.findIndex((l) => l.id === wordsPanel.line) + 1;
    const itemText = it ? it.word_ids.map((id) => anim.words.find((w) => w.word_id === id)?.text).join(" ") : "";
    return { lineNo, description: `${wordsLabel(anim)} · line ${lineNo}${it ? ` · ${itemText}` : ""}` };
  }
  mountMain(view, anim, animations, end, parts) {
    const main = wordsPanel.mainEl ||= el("section", { id: "words-main" });
    main.setAttribute("aria-label", wordsPanel.motionWorkspace ? "Motion workspace" : `${wordsLabel(anim)} lyrics`);
    const [head, notice, upper, stageNode, toolbar, controls] = parts;
    const before = [head, ...notice ? [notice] : [], upper];
    const after = [...toolbar ? [toolbar] : [], controls];
    if (stageNode.parentElement === main) {
      for (const child of [...main.children]) {
        if (child !== stageNode)
          child.remove();
      }
      stageNode.before(...before);
      stageNode.after(...after);
    } else {
      main.replaceChildren(...before, stageNode, ...after);
    }
    const left = wordsPanel.leftEl ||= el("aside", { id: "words-lyrics" });
    const right = wordsPanel.rightEl ||= el("aside", { id: "words-film" });
    for (const rail of [left, right]) {
      rail.tabIndex = 0;
      rail.onwheel = rail.onpointerdown = rail.onkeydown = () => wordsFlowController.beginRailScroll(rail);
      rail.onscroll = () => wordsFlowController.syncRailScroll(rail);
    }
    const searchInput = el("input", {
      type: "search",
      class: "field words-search-input",
      placeholder: "Filter lyrics…",
      "aria-label": "Filter lyrics",
      oninput: (e) => {
        wordsPanel.searchQuery = e.target.value;
        const q = (wordsPanel.searchQuery || "").trim().toLowerCase();
        for (const card of left.querySelectorAll(".words-line")) {
          const text = card.textContent?.toLowerCase() || "";
          card.classList.toggle("search-dimmed", !!q && !text.includes(q));
        }
      }
    });
    searchInput.value = wordsPanel.searchQuery || "";
    left.replaceChildren(new PanelHeader({ id: "words-lyrics-head", title: "Lyrics", actions: [searchInput] }).element, wordsTimeline(animations, end));
    right.replaceChildren(new PanelHeader({ id: "words-film-head", title: "Visual cues" }).element, wordsRoll(animations, end));
    for (const [node, id] of [[main, "words-main"], [left, "words-lyrics"], [right, "words-film"]])
      registerElement(node, id);
    if (main.parentElement !== view || left.parentElement !== view || right.parentElement !== view) {
      view.replaceChildren(left, main, right);
    }
  }
  render() {
    const view = $("view-design"), anim = wordsCurrent(), animations = wordsPanel.read?.document?.animations || [];
    if (!anim) {
      view.replaceChildren(el("p", { class: "panel-empty inline" }, "No lyric sections yet."));
      return;
    }
    const { lineNo, description } = this.headerMeta(anim);
    const end = Math.max(...animations.map((a) => wordsSpan(a).end)) + 4;
    const changes = wordsPanel.designs.size + Object.keys(wordsPanel.styleEdits).length;
    const saveLabel = changes ? `Save ${changes} change${changes > 1 ? "s" : ""} & sync EffectCraft` : "Save & sync EffectCraft project";
    const save = el("button", {
      id: "words-save",
      class: "icon",
      title: saveLabel,
      "aria-label": saveLabel,
      onclick: () => wordsSave(!changes),
      html: '<svg viewBox="0 0 24 24"><path d="M5 3h12l4 4v14H3V3h2zM7 3v6h9V3M7 21v-8h10v8"/></svg>'
    });
    const designer = wordsPanel.view === "design" && !!wordsPanel.line;
    this.mountMain(view, anim, animations, end, [
      new PanelHeader({
        id: "words-main-head",
        title: wordsPanel.motionWorkspace ? `Motion · ${wordsLabel(anim)}` : `${wordsLabel(anim)} · ${lineNo}`,
        description,
        actions: wordsPanel.motionWorkspace ? [
          el("button", { type: "button", class: "ghost", onclick: () => location.assign("motion.html") }, "Compositions"),
          this.linksToggleButton(),
        ] : [this.modeTabs(), this.linksToggleButton(), save]
      }).element,
      wordsPanel.read.reference_changed ? el("p", { class: "not-linked" }, "The timing reference changed since this was saved; check the words before saving.") : null,
      wordsPanel.motionWorkspace ? this.motionSource() : wordsUpperTracks(anim),
      designer ? wordsCanvas(anim) : this.stage(anim),
      designer ? this.designerToolbar(anim) : null,
      el("div", { id: "words-workspace-controls" }, ...this.innerControls(anim))
    ]);
    view.dataset.wordsMode = wordsPanel.mode;
    wordsViewFitter.observe(view);
    if ($("magic-workstation"))
      wordsViewFitter.observe($("magic-workstation"));
    this.width(view);
    requestAnimationFrame(() => {
      wordsCanvasDraw();
      this.fitClock(view);
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
  fitClock(view) {
    view.style.removeProperty("--song-s");
    const base = parseFloat(getComputedStyle(view).getPropertyValue("--song-s-px")) || 14;
    let scale = base;
    for (const row of view.querySelectorAll(".words-tl > .words-line")) {
      const seconds = Number(row.style.getPropertyValue("--len-s"));
      if (seconds > 0)
        scale = Math.max(scale, (row.scrollHeight + parseFloat(getComputedStyle(row).marginTop || "0") + 4) / seconds);
    }
    const capped = Math.min(36, scale);
    if (capped > base)
      view.style.setProperty("--song-s", `${Math.ceil(capped)}px`);
  }
  sizeInput(anim) {
    const input = el("input", {
      type: "range",
      min: "60",
      max: "220",
      step: "2",
      "aria-label": "Word size",
      oninput: (e) => {
        wordsPanel.styleEdits.size = Number(e.target.value);
        if (wordsPanel.styleEdits.size === (anim.style?.size ?? 120))
          delete wordsPanel.styleEdits.size;
        this.render();
      }
    });
    input.value = String(wordsPanel.styleEdits.size ?? anim.style?.size ?? 120);
    input.defaultValue = String(anim.style?.size ?? 120);
    return input;
  }
  styleFields(anim) {
    const field = (key, label, type, fallback) => {
      const saved = anim.style?.[key] ?? fallback;
      const input = el("input", {
        type,
        "aria-label": label,
        oninput: (e) => {
          if (e.target.value === saved)
            delete wordsPanel.styleEdits[key];
          else
            wordsPanel.styleEdits[key] = e.target.value;
        },
        onchange: () => this.render()
      });
      input.value = wordsPanel.styleEdits[key] ?? saved;
      input.defaultValue = saved;
      return el("label", { class: "field-row" }, label, input);
    };
    return [
      field("font", "Font", "text", ""),
      field("color", "Colour", "color", "#ffffff"),
      field("emphasis_color", "Emphasis", "color", "#ffd078"),
      field("outline_color", "Outline", "color", "#173b43"),
      field("background", "Background", "text", "")
    ];
  }
  useButton(anim) {
    const used = wordsPanel.read.in_video?.includes(anim.animation_id);
    const label = used ? "Restore lyrics in the web preview" : "Apply this section to the web preview";
    return el("button", {
      class: "icon",
      title: label,
      "aria-label": label,
      "aria-pressed": String(!!used),
      onclick: () => wordsApply(!used),
      html: '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 12 3 3 7-7"/></svg>'
    });
  }
  availableBackdrops() {
    const seen = new Set, list = [];
    for (const img of [...wordsPanel.backgrounds?.images || [], ...wordsPanel.read?.images || []]) {
      const id = img.asset_id || img.id;
      if (!id || !img.path || seen.has(id))
        continue;
      seen.add(id);
      list.push({ id, name: img.name || id, path: img.path });
    }
    return list;
  }
  async pickBackdrop(anim, assetId) {
    const heldTime = wordsPanel.time, wasPlaying = wordsPanel.playing;
    try {
      const result = await wordsCommand("words.save", {
        animation_id: anim.animation_id,
        expected_source_revision: anim.source_revision,
        background_asset: assetId,
        _expected_project: state.project
      });
      if (result.conflict) {
        toast("This verse changed elsewhere; reloaded");
        return wordsOpen();
      }
      anim.source_revision = result.animation.source_revision;
      anim.background_asset = result.animation.background_asset;
      anim.background_custom = result.animation.background_custom;
      if (wordsPanel.read?.in_video?.includes(anim.animation_id)) {
        await wordsCommand("words.apply", { animation_id: anim.animation_id, use: true });
      }
      wordsPanel.show = assetId ? "still" : "stage";
      wordsPanel.stageDirty = true;
      this.render();
      await this.show(wordsPanel.show);
      wordsPanel.time = heldTime;
      this.seekSong(heldTime);
      if (wasPlaying)
        this.video("play");
    } catch (error) {
      toast(String(error));
    }
  }
  backdropPicker(anim) {
    const images = this.availableBackdrops();
    const isFullVideo = wordsPanel.show === "stage" || !anim.background_custom && !anim.background_asset;
    const videoBtn = el("button", {
      class: "words-background words-background-mode",
      title: "Play full animated video scene",
      "aria-label": "Use full video scene",
      "aria-pressed": String(isFullVideo),
      onclick: () => this.pickBackdrop(anim, null)
    }, el("span", { class: "words-background-label" }, "Video"));
    return el("div", { class: "words-backdrop", role: "group", "aria-label": "Scene backgrounds" }, el("span", { class: "meta" }, "Scene"), el("div", { class: "words-backgrounds" }, videoBtn, ...images.map((image) => el("button", {
      class: "words-background",
      title: image.name,
      "aria-label": `Use background ${image.name}`,
      "aria-pressed": String(!isFullVideo && anim.background_asset === image.id),
      onclick: () => this.pickBackdrop(anim, image.id)
    }, el("img", { src: convertFileSrc(image.path), alt: image.name, loading: "lazy" })))));
  }
  songSpan() {
    const animations = wordsPanel.read?.document?.animations || [];
    const studioEnds = (wordsPanel.studio?.sections || []).map((s) => Number(s.end) || 0);
    const animEnds = animations.map((a) => wordsSpan(a).end || 0);
    return { start: 0, end: Math.max(10, ...studioEnds, ...animEnds) };
  }
  toggleSongPlay() {
    if (wordsPanel.playing) {
      this.video("pause");
      return;
    }
    wordsPanel.previewRange = null;
    const full = this.songSpan(), o = wordsPanel.offset;
    this.video("set-play-range", { startSeconds: 0, endSeconds: Math.max(1, full.end - o) });
    if (wordsPanel.time >= full.end - 0.5)
      this.seekSong(0);
    this.video("play");
  }
  stageEmptyMessage(url) {
    if (!url)
      return "Start the Studio server to play the video here.";
    return wordsPanel.studio?.error ? `The video's lyric clips could not be read: ${wordsPanel.studio.error}` : "The video has no lyric clips for this section.";
  }
  stage(anim) {
    const section = wordsStudioSection(anim) || wordsSpan(anim), songSpan = this.songSpan();
    const url = serverUrl();
    wordsPanel.server = url || null;
    if (!section || !url) {
      wordsPanel.stage = null;
      return el("p", { class: "panel-empty inline" }, this.stageEmptyMessage(url));
    }
    if (wordsPanel.stage && wordsPanel.show !== "edited") {
      wordsPanel.stage.dataset.animation = anim.animation_id;
      if (wordsPanel.stageDirty)
        this.show(wordsPanel.show);
      return wordsPanel.stage;
    }
    const scrub = el("input", {
      type: "range",
      id: "words-scrub",
      min: "0",
      max: String(songSpan.end),
      step: "0.01",
      "aria-label": "Video time",
      oninput: (e) => {
        e.target.defaultValue = e.target.value;
        const t = Number(e.target.value);
        if (wordsPanel.playing) {
          this.video("pause");
          wordsPanel.playing = false;
        }
        wordsPanel.previewRange = null;
        wordsPanel.autoPlay = false;
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
    const view = (show, label) => el("button", { role: "tab", "data-show": show, "aria-selected": String(wordsPanel.show === show), onclick: () => this.show(show) }, label);
    wordsPanel.stage = el("div", { class: "words-preview", "data-animation": anim.animation_id }, anim.effects?.text ? el("div", { class: "segmented", role: "tablist", "aria-label": "What the stage shows" }, view("still", "Scene image"), view("edited", `Edited · ${anim.effects.text.name}`)) : null, el("div", { class: "stage", id: "words-preview" }, frame), el("div", { class: "transport" }, wordsPanel.playButton = el("button", { class: "icon", "aria-label": "Play song", title: "Play or pause the entire song", onclick: () => this.toggleSongPlay(), html: ICONS.play }), scrub, wordsPanel.timeLabel = el("span", { class: "meta" }, `${wordsClock(wordsPanel.time || 0)} / ${wordsClock(songSpan.end)}`), this.useButton(anim)));
    if (wordsPanel.show === "edited" && !anim.effects?.text)
      wordsPanel.show = "still";
    this.show(wordsPanel.show, frame);
    return wordsPanel.stage;
  }
  async resolveShowUrl(show, anim) {
    if (show === "edited") {
      return `${wordsPanel.player}/comp/${(await wordsCommand("words.compose", { animation_id: anim.animation_id })).src}`;
    }
    if (!["lyrics", "stage", "still"].includes(show))
      return wordsPanel.player;
    const effectiveShow = show === "still" && !anim.background_asset && !this.availableBackdrops().length ? "stage" : show;
    return `${wordsPanel.player}/comp/${(await wordsCommand("words.lyrics_view", { show: effectiveShow, animation_id: anim.animation_id })).src}`;
  }
  async show(show, frame = wordsPanel.stage?.querySelector("#words-video-frame") || $("words-video-frame")) {
    const anim = wordsCurrent();
    wordsPanel.show = show;
    wordsPanel.stage?.querySelectorAll('[role="tab"]').forEach((tab) => tab.setAttribute("aria-selected", String(tab.dataset.show === show)));
    if (!frame || !wordsPanel.player || !anim)
      return;
    wordsPanel.offset = show === "edited" ? anim.song_anchor || 0 : 0;
    try {
      const src = await this.resolveShowUrl(show, anim);
      const cleanCurrent = (frame.getAttribute("src") || "").split("?")[0];
      if (cleanCurrent !== src || wordsPanel.stageDirty) {
        wordsPanel.stageDirty = false;
        frame.setAttribute("src", `${src}?v=${Date.now()}`);
      } else
        this.videoRange();
    } catch (error) {
      toast(String(error).replace(/^.*?: /, ""));
    }
  }
  seekSong(t) {
    wordsPanel.time = t;
    wordsPanel.pendingSeek = t;
    const scrub = $("words-scrub");
    if (scrub) {
      scrub.value = String(t);
      scrub.defaultValue = scrub.value;
    }
    if (wordsPanel.timeLabel)
      wordsPanel.timeLabel.textContent = `${wordsClock(t)} / ${wordsClock(this.songSpan().end)}`;
    wordsCanvasDraw(t);
    wordsNow(t);
    this.video("seek", { timeSeconds: Math.max(0, t - wordsPanel.offset), seekMode: "commit" });
  }
  statusRefresh() {
    const url = serverUrl() || null;
    if (url === wordsPanel.server || !wordsCurrent())
      return;
    wordsPanel.stage = null;
    this.render();
    this.videoRange();
  }
  video(action, params = {}) {
    if (action === "pause" && wordsPanel.audio && !wordsPanel.audio.paused) {
      wordsPanel.audio.pause();
      wordsPanel.playing = false;
    }
    const frame = $("words-video-frame");
    if (frame) {
      frame.contentWindow?.postMessage({ source: "hf-parent", type: "control", action, ...params }, "*");
    } else if (action === "play") {
      wordsTimelineController.play(true);
    }
  }
  videoRange() {
    const anim = wordsCurrent();
    if (!anim)
      return;
    const full = this.songSpan(), range = wordsPanel.previewRange || full;
    const o = wordsPanel.offset;
    this.video("set-play-range", { startSeconds: Math.max(0, range.start - o), endSeconds: Math.max(1, range.end - o) });
    if (!wordsPanel.playing)
      this.seekSong(wordsPanel.autoPlay && wordsPanel.previewRange ? range.start : wordsPanel.time || wordsSpan(anim).start || 0);
    if (wordsPanel.autoPlay)
      this.video("play");
  }
  fit(size = wordsPanel.size) {
    const frame = $("words-video-frame"), box = $("words-preview");
    if (!frame || !box || !size)
      return;
    wordsPanel.size = size;
    frame.style.setProperty("--w", `${size.width}px`);
    frame.style.setProperty("--h", `${size.height}px`);
    const scale = Math.min(box.clientWidth / size.width, box.clientHeight / size.height);
    frame.style.setProperty("--scale", String(scale));
    frame.style.setProperty("--frame-x", `${(box.clientWidth - size.width * scale) / 2}px`);
    frame.style.setProperty("--frame-y", `${(box.clientHeight - size.height * scale) / 2}px`);
  }
  followLivePlayback(song) {
    const animations = wordsPanel.read?.document?.animations || [];
    const unsaved = wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length;
    if (unsaved || wordsPanel.previewRange)
      return;
    const here = animations.find((a) => {
      const s = wordsSpan(a);
      return song >= s.start - 0.3 && song < s.end + 0.1;
    });
    const view = $("view-design");
    const sectionChanged = here && here.animation_id !== wordsPanel.selected;
    if (sectionChanged) {
      wordsPanel.selected = here.animation_id;
      view?.querySelectorAll(".words-band[data-section]").forEach((b) => b.classList.toggle("active", b.dataset.section === here.animation_id));
      document.querySelector("#words-main .words-upper-tracks")?.replaceWith(wordsUpperTracks(here));
    }
    const current = wordsCurrent();
    if (!current)
      return;
    const line = wordsLines(current).findLast((l) => wordsAt(l.words[0]) <= song + 0.25) || wordsLines(current)[0];
    if (line && (sectionChanged || line.id !== wordsPanel.line)) {
      wordsPanel.line = line.id;
      view?.querySelectorAll(".words-tl > .words-line").forEach((r) => r.classList.toggle("active", r.dataset.animationId === current.animation_id && r.dataset.lineId === line.id));
      const lineNo = wordsLines(current).findIndex((l) => l.id === line.id) + 1;
      const headTitle = $("words-main-head")?.querySelector(".panel-title");
      if (headTitle)
        headTitle.textContent = wordsPanel.motionWorkspace ? `Motion · ${wordsLabel(current)}` : `${wordsLabel(current)} · ${lineNo}`;
      $("words-workspace-controls")?.replaceChildren(...this.innerControls(current));
    }
  }
  motionSource() {
    const label = wordsPanel.motionMediaStale ? "Earlier Studio render" :
      wordsPanel.motionMedia ? "Studio video" : "Scene still · render a Studio preview for video";
    return el("p", { class: "meta" }, label);
  }
  handlePreviewBoundary(song, isPlaying) {
    const range = wordsPanel.previewRange;
    if (!range || song < range.end - 0.04)
      return false;
    if (wordsPanel.loopLine) {
      this.seekSong(range.start);
      if (!isPlaying)
        this.video("play");
      return true;
    }
    if (isPlaying) {
      this.video("pause");
      this.seekSong(Math.max(range.start, range.end - 0.18));
      return true;
    }
    return false;
  }
  syncPendingSeek(song, isPlaying) {
    if (wordsPanel.pendingSeek == null)
      return false;
    if (Math.abs(song - wordsPanel.pendingSeek) <= 0.35 || isPlaying) {
      wordsPanel.pendingSeek = null;
      if (wordsPanel.autoPlay && !isPlaying)
        this.video("play");
      return false;
    }
    this.seekSong(wordsPanel.pendingSeek);
    if (wordsPanel.autoPlay)
      this.video("play");
    return true;
  }
  handleStateMessage(data) {
    if (!$("words-video-frame") || wordsPanel.view === "design")
      return;
    const song = data.currentTime + wordsPanel.offset;
    if (!data.isPlaying && performance.now() < (wordsPanel.userScrollingUntil || 0))
      return;
    if (this.syncPendingSeek(song, data.isPlaying))
      return;
    wordsPanel.time = song;
    wordsPanel.playing = !!data.isPlaying;
    if (this.handlePreviewBoundary(song, data.isPlaying))
      return;
    if (data.isPlaying)
      wordsPanel.autoPlay = false;
    if (data.isPlaying || !wordsPanel.previewRange)
      this.followLivePlayback(song);
    const scrub = $("words-scrub");
    if (scrub) {
      scrub.value = String(song);
      scrub.defaultValue = scrub.value;
    }
    if (wordsPanel.timeLabel)
      wordsPanel.timeLabel.textContent = `${wordsClock(song)} / ${wordsClock(this.songSpan().end)}`;
    wordsNow(song);
    wordsPanel.playButton?.setAttribute("aria-label", data.isPlaying ? "Pause song" : "Play song");
  }
  width(view = $("view-design")) {
    if (!view)
      return;
    view.style.setProperty("--words-available", `${Math.max(220, innerHeight - Math.max(0, view.getBoundingClientRect().top) - 48)}px`);
    view.classList.toggle("narrow", view.clientWidth < 1100);
    view.classList.toggle("tight", view.clientWidth < 600);
  }
  async kinetic(change) {
    const anim = wordsCurrent();
    if (!anim?.kinetic)
      return;
    const heldLine = wordsPanel.line || wordsLines(anim)[0]?.id;
    const kinetic = { ...anim.kinetic, ...change };
    try {
      const result = await wordsCommand("words.save", { animation_id: anim.animation_id, expected_source_revision: anim.source_revision, origin: "magicstudio-design", kinetic });
      if (result.conflict) {
        toast("Someone changed these lyrics first; reloaded the latest");
        return wordsOpen();
      }
      if (wordsPanel.read?.in_video?.includes(anim.animation_id)) {
        await wordsCommand("words.apply", { animation_id: anim.animation_id, use: true }).catch(() => {});
      }
      await wordsCommand("words.letter", { render: false }).catch(() => {});
      wordsPanel.line = heldLine;
      wordsPanel.followed = `${anim.animation_id}/${heldLine}`;
      wordsPanel.previewRange = wordsLineWindow(anim, heldLine);
      wordsPanel.autoPlay = true;
      wordsPanel.time = wordsPanel.previewRange.start;
      wordsPanel.stageDirty = true;
      await wordsOpen();
    } catch (error) {
      toast(String(error).replace(/^.*?: /, ""));
    }
  }
  async shuffle(kinetic = true) {
    const anim = wordsCurrent();
    if (!anim)
      return;
    try {
      if (kinetic && anim.kinetic) {
        const lib = wordsPanel.read.kinetic_library || {};
        const pick = (items, current) => {
          const keys = Object.keys(items || {});
          return keys[Math.floor(Math.random() * keys.length)] || current;
        };
        return this.kinetic({ layout: pick(lib.layouts, anim.kinetic.layout), entrance: pick(lib.entrances, anim.kinetic.entrance), seed: Math.floor(Math.random() * 1000) });
      }
      await wordsCommand("words.design", { section: anim.animation_id, seed: Math.floor(Math.random() * 1000), kinetic });
      wordsPanel.stage = null;
      await wordsOpen();
    } catch (error) {
      toast(String(error).replace(/^.*?: /, ""));
    }
  }
}
const wordsViewController = new WordsViewController;
window.WordsViewController = WordsViewController;
window.wordsViewController = wordsViewController;
