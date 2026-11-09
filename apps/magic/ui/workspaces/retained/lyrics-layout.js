// Presentation & timeline adapter for Lyrics and Motion workspaces.
// Uses clean moment cards on the left (no filmstrip sprockets), the 16:9 stage with curved
// ♪ ♫ ♬ musical-note tether arrows and dashed motion trajectories in the center, the
// Effects / Drawing Effects rail on the right, and the full-width Song Audio Waveform +
// Word/Accent/Drawing timeline dock at the bottom.

function lyricPreviewItems(words, design) {
  const byId = new Map(words.map(word => [word.word_id, word.text]));
  const finite = (value, fallback) => Number.isFinite(value) ? value : fallback;
  return (design?.items || []).map(item => ({
    text: (item.word_ids || []).map(id => byId.get(id)).filter(Boolean).join(" "),
    x: finite(item.x, 0), y: finite(item.y, 0), size: Math.max(8, finite(item.size, 58)),
    rotate: finite(item.rotate, 0), weight: finite(item.weight, 400),
    font: item.font || "song", italic: !!item.italic,
    color: item.color || "#173f49", outline: item.outline || null,
  })).filter(item => item.text);
}

function isLightFillColor(hex) {
  const m = String(hex || "").trim().match(/^#?([0-9a-f]{6})$/i);
  if (!m) return false;
  const n = parseInt(m[1], 16);
  const r = (n >> 16) & 255, g = (n >> 8) & 255, b = n & 255;
  return (0.299 * r + 0.587 * g + 0.114 * b) > 185;
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

  function projectRootDir() {
    const p = String(state.project || "");
    return p.replace(/[/\\][^/\\]+$/, "");
  }

  function syncActiveEffectClipForSection() {
    const clips = wordsPanel.read?.scene_clips || [];
    if (!clips.length) {
      wordsPanel.activeEffectClipPath = null;
      return;
    }
    if (wordsPanel.presentationMode !== "motion") {
      if (!wordsPanel.keepClipInDesign) {
        wordsPanel.activeEffectClipPath = null;
      }
      return;
    }
    const animations = wordsPanel.read?.document?.animations || [];
    const idx = Math.max(0, animations.findIndex(a => a.animation_id === wordsPanel.selected));
    const entry = clips.find(c => c.sectionIndex === idx) || clips[0];
    if (entry && !wordsPanel.manualEffectClipId) {
      wordsPanel.activeEffectClipId = entry.id;
      wordsPanel.activeEffectClipStart = entry.start;
      wordsPanel.activeEffectClipPath = entry.path;
    }
  }

  const preview = (anim, line) => {
    const visibility = authoringVisibility(wordsPanel.presentationMode, wordsPanel.previewTogether);
    const design = wordsDesign(anim, line.id);
    const svg = svgNode("svg", {
      viewBox: "0 0 1920 1080",
      class: "lyrics-composition-thumb",
      role: "img",
      "aria-label": `Layout thumbnail: ${line.words.map(word => word.text).join(" ")}`
    });
    const group = svgNode("g");
    for (const item of (visibility.words ? lyricPreviewItems(line.words, design) : [])) {
      const needsDarkStroke = item.outline || (isLightFillColor(item.color) ? "#173f49" : null);
      const attrs = {
        x: item.x, y: item.y, "dominant-baseline": "text-before-edge",
        "font-size": item.size, "font-family": item.font === "song" ? wordsSongFont(anim) : item.font,
        "font-weight": item.weight, "font-style": item.italic ? "italic" : "normal",
        fill: item.color,
        transform: `rotate(${item.rotate} ${item.x} ${item.y})`,
      };
      if (needsDarkStroke) {
        attrs.stroke = needsDarkStroke;
        attrs["stroke-width"] = String(Math.max(5, Math.round(item.size * 0.11)));
        attrs["stroke-linejoin"] = "round";
        attrs["paint-order"] = "stroke fill";
      }
      const text = svgNode("text", attrs);
      text.textContent = item.text;
      group.append(text);
    }
    for (const drawing of visibility.drawings ? design.draws || [] : []) {
      const box = String(drawing.view_box || "").trim().split(/\s+/).map(Number);
      if (box.length !== 4 || !box.every(Number.isFinite) || box[2] <= 0 || box[3] <= 0) continue;
      const strokeGroup = svgNode("g", {
        transform: `translate(${drawing.x || 0} ${drawing.y || 0}) scale(${(drawing.w || box[2]) / box[2]} ${(drawing.h || box[3]) / box[3]}) translate(${-box[0]} ${-box[1]})`
      });
      for (const path of drawing.paths || []) {
        const strokeCol = path.stroke || "#173f49";
        const pathAttrs = {
          d: path.d || "",
          fill: "none",
          stroke: isLightFillColor(strokeCol) ? "#8B3D26" : strokeCol,
          "stroke-width": Math.max(4, path.width || 5),
          "stroke-linecap": "round",
          "stroke-linejoin": "round",
        };
        strokeGroup.append(svgNode("path", pathAttrs));
      }
      group.append(strokeGroup);
    }
    if (!visibility.words && !(design.draws && design.draws.length)) {
      const empty = svgNode("text", { x: 960, y: 540, "text-anchor": "middle", "font-size": 78, fill: "#667487" });
      empty.textContent = "No drawing";
      group.append(empty);
    }
    svg.append(group);
    requestAnimationFrame(() => {
      if (!svg.isConnected) return;
      const box = group.getBBox();
      if (box.width > 0 && box.height > 0) {
        const pad = Math.max(24, box.width * 0.06);
        svg.setAttribute("viewBox", `${box.x - pad} ${box.y - pad} ${box.width + pad * 2} ${box.height + pad * 2}`);
      }
    });
    return svg;
  };

  const buildCard = wordsTimelineController.buildLineCard.bind(wordsTimelineController);
  wordsTimelineController.buildLineCard = (anim, index, line, at, length) => {
    const card = buildCard(anim, index, line, at, length);
    if (!card.id) card.id = `lyrics-moment-${anim.animation_id}-${line.id}`;
    card.setAttribute("role", "button");
    card.setAttribute("aria-label", `${line.words.map(word => word.text).join(" ")} · ${wordsClock(at)}`);
    const drawCount = (wordsDesign(anim, line.id).draws || []).length;
    const linkedWords = (wordsDesign(anim, line.id).draws || [])
      .flatMap(d => d.word_ids || [])
      .map(id => line.words.find(w => w.word_id === id)?.text)
      .filter(Boolean);
    const drawLabel = drawCount
      ? (linkedWords.length ? `✎ "${linkedWords[0]}" (${drawCount})` : `${drawCount} ${drawCount === 1 ? "drawing" : "drawings"}`)
      : "0 drawings";
    card.append(
      preview(anim, line),
      el("div", { class: "lyrics-card-caption" },
        el("span", {}, `${wordsClock(at)} – ${wordsClock(at + length)}`),
        el("span", { class: drawCount ? "lyrics-card-draw-badge" : "" }, drawLabel)
      )
    );
    return card;
  };

  const timeline = wordsTimelineController.timeline.bind(wordsTimelineController);
  wordsTimelineController.timeline = (...args) => {
    const rail = timeline(...args);
    rail.append(...[...rail.children].sort((a, b) =>
      Number(a.style.getPropertyValue("--at-s") || 0) - Number(b.style.getPropertyValue("--at-s") || 0)));
    return rail;
  };

  // The left rail is a clean vertical card list without filmstrip sprocket geometry.
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
    if (!["design", "motion"].includes(mode)) return;
    const changed = wordsPanel.presentationMode !== mode;
    if (changed) {
      wordsPanel.audio?.pause();
      wordsPanel.motionVideo?.pause();
      wordsPanel.playing = false;
    }
    wordsPanel.presentationMode = mode;
    wordsPanel.previewTogether = false;
    wordsPanel.inspectorTab = mode === "motion" ? "draw" : "word";
    wordsPanel.mode = "lyrics";
    if (mode === "design") {
      wordsPanel.drawMode = false;
    }
    document.title = `Magic Studio — ${mode === "design" ? "Lyrics" : "Motion"}`;
    syncActiveEffectClipForSection();
    if (wordsPanel.read && changed) wordsRender();
  };

  window.setMagicAuthoringMode(window.__magicAuthoringMode || "design");

  function buildMotionEffectCraftBar(anim) {
    const root = projectRootDir();
    const clips = wordsPanel.read?.scene_clips || [];
    const animations = wordsPanel.read?.document?.animations || [];
    const curSecIdx = Math.max(0, animations.findIndex(a => a.animation_id === anim.animation_id));
    const activeClipId = wordsPanel.activeEffectClipId || (clips.find(c => c.sectionIndex === curSecIdx) || clips[0])?.id;

    const clipButtons = clips.map(clip => {
      const isOn = activeClipId === clip.id && Boolean(wordsPanel.activeEffectClipPath);
      return el("button", {
        type: "button",
        id: `motion-clip-${clip.id.toLowerCase()}`,
        class: `chip motion-ec-clip-pill${isOn ? " active" : ""}`,
        "aria-pressed": String(isOn),
        title: `EffectCraft Scene Composition ${clip.id} (${clip.path}) · Click to preview clip & choreograph overlays`,
        onclick: () => {
          wordsPanel.manualEffectClipId = clip.id;
          wordsPanel.activeEffectClipId = clip.id;
          wordsPanel.activeEffectClipStart = clip.start;
          wordsPanel.activeEffectClipPath = clip.path;
          wordsPanel.scene = true;
          const targetAnim = animations[clip.sectionIndex] || animations[0];
          if (targetAnim && targetAnim.animation_id !== wordsPanel.selected) {
            wordsPanel.selected = targetAnim.animation_id;
            const lines = wordsLines(targetAnim);
            const matchingLine = lines.find(l => wordsLineSpan(targetAnim, l.id).start >= clip.start - 0.5) || lines[0];
            if (matchingLine) wordsPanel.line = matchingLine.id;
          }
          wordsPanel.time = clip.start + 0.35;
          wordsRender();
          wordsSeekSong(wordsPanel.time);
        }
      }, el("span", { class: "motion-ec-badge" }, "EC"), clip.label);
    });

    const openLyricsBtn = el("button", {
      type: "button",
      id: "motion-open-lyrics-ecproj",
      class: "ghost",
      title: "Open lettering/lyrics.ecproj in EffectCraft",
      onclick: async () => {
        try {
          await wordsCommand("workspace.open", { path: `${root}/lettering/lyrics.ecproj` });
          toast("Opened lyrics.ecproj in EffectCraft");
        } catch (err) {
          toast(String(err));
        }
      }
    }, "✦ Lyrics (.ecproj)");

    const drawCueBtn = el("button", {
      type: "button",
      id: "motion-draw-cue-btn",
      class: `chip words-sketch-pill${wordsPanel.drawMode ? " active" : ""}`,
      title: "Toggle freehand EffectCraft brush / motion overlay drawing on the 16:9 video stage",
      onclick: () => {
        wordsPanel.drawMode = !wordsPanel.drawMode;
        wordsPanel.inspectorTab = "draw";
        wordsRender();
      }
    }, wordsPanel.drawMode ? "✎ Drawing on Stage…" : "✎ Draw on Stage");

    return el("section", {
      id: "motion-ec-clips-bar",
      class: "words-shelf-bar motion-ec-clips-bar",
      "aria-label": "EffectCraft scene compositions and clips"
    },
      el("span", { class: "motion-ec-bar-title" }, "🎬 EffectCraft Clips:"),
      el("div", { class: "motion-ec-clips-scroll" }, ...clipButtons),
      el("div", { class: "motion-ec-bar-actions" }, drawCueBtn, openLyricsBtn)
    );
  }

  const STORYBOARD_FIELD_LABELS = {
    title: "Scene Title",
    cue: "Lyric / Time Cue",
    time: "Timestamp",
    purpose: "Scene Purpose",
    staging_notes: "Staging & Motion Notes",
    overall_design: "Visual Direction",
    exact_prompt: "Scene Prompt",
    hand_note: "Director Note",
    strip_note: "Contact Strip Note",
    motion_subheading: "Motion Subheading",
    motion_choreography_notes: "Choreography & Timing",
    motion_drawn_spec: "Drawn SVG / Overlay Spec",
    motion_hand_note: "Motion Director Note",
    cover_title: "Production Title",
    cover_tagline: "Tagline",
    cover_subtitle: "Logline",
    world_subheading: "World Subheading",
    world_location_prompt: "Location Prompt",
    char_subheading: "Character Subheading",
    char_exact_prompt: "Character Prompt",
  };

  function friendlySlotLabel(label) {
    if (STORYBOARD_FIELD_LABELS[label]) return STORYBOARD_FIELD_LABELS[label];
    return String(label || "")
      .replace(/^(cover|world|char|motion|scene)_/, "")
      .replace(/_/g, " ")
      .replace(/\b\w/g, c => c.toUpperCase());
  }

  function buildStoryboardQuickEditor(anim) {
    const project = state.project;
    const container = el("details", {
      id: "authoring-storyboard-card",
      class: "authoring-effect-library authoring-storyboard-card",
      open: true,
    }, el("summary", {}, wordsPanel.presentationMode === "motion" ? "Storyboard · Motion & Scene Notes" : "Storyboard · Scene Context & Text"));

    const body = el("div", { class: "authoring-sb-body" });
    container.append(body);

    const renderBody = () => {
      const pages = wordsPanel.storyboardPages || [];
      if (!pages.length) {
        body.replaceChildren(el("p", { class: "muted" }, "Loading storyboard spreads…"));
        return;
      }
      const animations = wordsPanel.read?.document?.animations || [];
      const curSecIdx = Math.max(0, animations.findIndex(a => a.animation_id === anim?.animation_id));
      const scenePages = pages.filter(p => p.archetype === "scene");
      const motionPage = pages.find(p => p.archetype === "motion");

      let defaultPage = scenePages[curSecIdx] || scenePages[0] || pages[0];
      if (wordsPanel.presentationMode === "motion" && motionPage && wordsPanel.sbPreferMotion !== false) {
        defaultPage = motionPage;
      }
      const selectedPageNum = wordsPanel.sbSelectedPage || defaultPage?.page || 1;
      const activePage = pages.find(p => p.page === selectedPageNum) || defaultPage || pages[0];
      if (!activePage) return;

      const select = el("select", {
        id: "authoring-sb-page-select",
        class: "authoring-sb-select",
        "aria-label": "Select storyboard spread",
        onchange: e => {
          const nextPage = Number(e.target.value);
          wordsPanel.sbSelectedPage = nextPage;
          wordsPanel.sbPreferMotion = pages.find(p => p.page === nextPage)?.archetype === "motion";
          renderBody();
        }
      }, ...pages.map(p => {
        const opt = el("option", { value: String(p.page) }, `P${p.page} · ${(p.archetype || "scene").toUpperCase()}${p.section ? ` (${p.section})` : ""} — ${p.title || "Untitled"}`);
        if (p.page === activePage.page) opt.selected = true;
        return opt;
      }));

      const openDcBtn = el("button", {
        type: "button",
        id: "authoring-sb-open-btn",
        class: "ghost",
        title: "Open this spread in DesignCraft",
        onclick: async () => {
          try {
            await wordsCommand("storyboard.open", { page: activePage.page });
            toast(`Opened page ${activePage.page} in DesignCraft`);
          } catch (err) {
            toast(String(err));
          }
        }
      }, "DesignCraft ↗");

      const topRow = el("div", { class: "authoring-sb-toolbar" }, select, openDcBtn);

      const editableSlots = (activePage.slots || []).filter(s =>
        s.kind === "text" &&
        !s.locked &&
        !s.hidden &&
        !/(page_no_|running_|_heading$|_badge$|_tag$|_hint$)/.test(s.label)
      );

      const drafts = {};
      const fieldsWrap = el("div", { class: "authoring-sb-fields" });
      for (const slot of editableSlots.slice(0, 8)) {
        const initial = String(slot.text || "");
        drafts[slot.label] = initial;
        const isMultiline = initial.length > 52 || /notes|purpose|design|prompt|spec|action/i.test(slot.label);
        const inputEl = isMultiline
          ? el("textarea", {
              id: `authoring-sb-slot-${slot.label}`,
              rows: "2",
              value: initial,
              oninput: e => { drafts[slot.label] = e.target.value; }
            }, initial)
          : el("input", {
              id: `authoring-sb-slot-${slot.label}`,
              type: "text",
              value: initial,
              oninput: e => { drafts[slot.label] = e.target.value; }
            });
        fieldsWrap.append(
          el("label", { class: "authoring-sb-field" },
            el("span", { class: "authoring-sb-field-label" }, friendlySlotLabel(slot.label)),
            inputEl
          )
        );
      }

      const saveBtn = el("button", {
        type: "button",
        id: "authoring-sb-save-btn",
        class: "start authoring-sb-save-btn",
        onclick: async () => {
          try {
            saveBtn.disabled = true;
            saveBtn.textContent = "Saving…";
            await wordsCommand("storyboard.set", { page: activePage.page, text: drafts });
            const refreshed = await wordsCommand("storyboard.pages", { full: true });
            wordsPanel.storyboardPages = refreshed.pages || [];
            toast(`Saved Storyboard page ${activePage.page}`);
            renderBody();
          } catch (err) {
            toast(String(err));
            saveBtn.disabled = false;
            saveBtn.textContent = "Save Storyboard Text";
          }
        }
      }, "Save Storyboard Text");

      body.replaceChildren(
        topRow,
        el("div", { class: "authoring-sb-meta muted" },
          `Layout: ${activePage.section || "Spread"} · ${editableSlots.length} editable text slots`
        ),
        fieldsWrap,
        saveBtn
      );
    };

    if (wordsPanel.storyboardProject !== project || !wordsPanel.storyboardPages) {
      wordsPanel.storyboardProject = project;
      void wordsCommand("storyboard.pages", { full: true }).then(res => {
        if (state.project !== project) return;
        wordsPanel.storyboardPages = res.pages || [];
        renderBody();
      }).catch(err => {
        body.replaceChildren(el("p", { class: "muted" }, `Storyboard unavailable: ${err}`));
      });
    } else {
      renderBody();
    }

    return container;
  }

  function addSongWaveform(lanes, anim) {
    const span = wordsLineSpan(anim, wordsPanel.line), project = state.project;
    const track = el("div", { class: "lane-track audio-waveform-track", role: "img", "aria-label": "Loading actual song waveform" }, el("span", { class: "waveform-status" }, "Loading audio…"), el("span", { class: "lane-head" }));
    lanes.prepend(el("div", { class: "lane", "data-track": "audio" }, el("span", { class: "lane-name" }, "Song audio"), track));
    void wordsCommand("song.waveform", { start: Math.max(0, span.start - .4), end: span.end, bins: 800 }).then(result => {
      if (!track.isConnected || state.project !== project) return;
      const peaks = result.peaks || [];
      if (!peaks.length) throw new Error("No waveform data");
      wordsPanel.recording = { project, source: result.source, duration: result.duration };
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
    syncActiveEffectClipForSection();
    mount(...args);
    const [view, anim] = args;
    view.dataset.authoringMode = wordsPanel.presentationMode || "design";
    view.classList.add("lyrics-authoring-layout");

    const main = document.getElementById("words-main");
    const viewStatus = el("span", { id: "authoring-preview-label", class: "muted" },
      wordsPanel.previewTogether ? "Combined preview · words + drawings" : wordsPanel.presentationMode === "motion" ? "Drawings only" : "Lyrics only"
    );
    const together = el("button", {
      id: "authoring-preview-together",
      class: wordsPanel.previewTogether ? "start" : "ghost",
      "aria-pressed": String(!!wordsPanel.previewTogether),
      onclick: () => { wordsPanel.previewTogether = !wordsPanel.previewTogether; wordsRender(); }
    }, "Preview together");

    main?.querySelector(".authoring-preview-toolbar")?.remove();
    main?.querySelector("#motion-ec-clips-bar")?.remove();
    const stageCanvas = main?.querySelector(".lyric-canvas");
    if (stageCanvas) {
      if (wordsPanel.presentationMode === "motion") {
        stageCanvas.before(buildMotionEffectCraftBar(anim));
      }
      stageCanvas.before(el("div", { class: "authoring-preview-toolbar" }, viewStatus, together));
    }

    // Move Effects / Drawing Effects into the right-hand rail (#words-effects-rail)
    const effects = wordsPanel.effectsRail ||= el("aside", {
      id: "words-effects-rail",
      class: "panel",
      "data-scroll": "auto",
      "data-look": "motion",
      "aria-label": "Effects"
    });
    const content = el("div", { class: "panel-body lyrics-effects-body" });
    if (main) {
      for (const column of main.querySelectorAll('.words-engine-col[aria-label="Motion styles and shaders"], .words-engine-col[aria-label="EffectCraft brush presets"], .words-engine-col[aria-label="EffectCraft effects"]')) {
        content.append(column);
      }
      if (wordsPanel.inspectorTab === "draw") {
        for (const column of main.querySelectorAll(".words-engine-grid > .words-engine-col")) {
          content.append(column);
        }
      }
    }
    if (wordsPanel.presentationMode === "motion") {
      const shapes = content.querySelector('[aria-label="Animated doodle stamps"]');
      if (shapes) {
        const strong = shapes.querySelector("strong");
        if (strong) strong.textContent = "Drawing shapes";
        content.prepend(shapes);
      }
      for (const column of [...content.children]) {
        if (column === shapes) continue;
        const label = column.getAttribute("aria-label") === "EffectCraft brush presets" ? "Brush library" : "Atmosphere";
        content.append(el("details", { class: "authoring-effect-library", open: true }, el("summary", {}, label), column));
      }
    }
    if (!content.children.length) {
      content.append(el("p", { class: "muted" }, "Select a word or drawing to see its effects."));
    }
    content.append(buildStoryboardQuickEditor(anim));
    const heading = new PanelHeader({ id: "words-effects-head", title: "Effects" }).element;
    heading.classList.add("panel-head");
    heading.dataset.look = "heading";
    heading.querySelector(".panel-title").textContent = wordsPanel.presentationMode === "motion" ? "Drawing effects" : "Effects";
    effects.replaceChildren(heading, content);
    view.append(effects);

    const source = document.getElementById("words-lyrics-head");
    if (source) {
      source.querySelector(".panel-title").textContent = wordsPanel.presentationMode === "motion" ? "Learning moments" : "Lyric moments";
      source.querySelector(".lyrics-source-note")?.remove();
      source.append(el("small", { class: "lyrics-source-note" }, "Artist words and actions · read-only"));
    }

    // Hide the sprocketed #words-film rail from Lyrics/Motion (since sprocketed filmstrip rails belong in Studio)
    const cues = wordsPanel.rightEl;
    if (cues) {
      cues.hidden = true;
    }

    if (wordsPanel.presentationMode === "design" && main) {
      const controls = main.querySelector("#words-workspace-controls");
      if (controls && typeof nativeWordControls === "function") {
        controls.querySelector(".word-motion-island")?.remove();
        controls.prepend(nativeWordControls(anim));
      }
    }
    if (wordsPanel.presentationMode === "motion" && main) {
      const controls = main.querySelector("#words-workspace-controls");
      if (controls && typeof existingDrawingControls === "function") {
        controls.querySelector(".existing-drawing-controls")?.remove();
        controls.prepend(existingDrawingControls(anim));
      }
      const draws = wordsDesign(anim, wordsPanel.line).draws || [];
      const drawing = draws[wordsPanel.selectedDrawingIndex || 0] || draws[0];
      const selectedWords = wordsLineWords(anim, wordsPanel.line).filter(word => drawing?.word_ids?.includes(word.word_id));
      const linked = selectedWords.map(word => word.text).join(" ") || (drawing?.name ? `${drawing.name} · Line cue` : "No linked word");
      controls?.querySelector(".authoring-linked-word")?.remove();
      controls?.prepend(el("p", { class: "authoring-linked-word" }, "Drawing context · ", el("strong", {}, linked)));
      const brushSettings = content.querySelector('[aria-label="EffectCraft brush presets"] .words-style-toggles');
      if (brushSettings && controls) {
        const settings = el("section", { class: "panel-body authoring-stroke-settings", "aria-label": "New stroke settings" }, el("strong", {}, "New stroke settings"), brushSettings);
        controls.insertBefore(settings, controls.children[1] || null);
      }
    }

    // Reuse the timed word/accent/drawing controls and FilmCraft song waveform in the bottom dock.
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

    requestAnimationFrame(() => {
      if (typeof wordsArrows === "function") wordsArrows(view, anim);
    });
  };
}
