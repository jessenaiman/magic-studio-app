class ProjectView {
  error;
  pending;
  queue;
  state;
  pfFilter = "all";
  pfSort = "type";
  pfGroup = true;
  pfSearch = "";
  pfCollapsedGroups = new Set();
  stageTab = "characters";
  constructor(state) {
    this.state = state;
    this.pending = 0;
    this.error = null;
    this.queue = Promise.resolve();
  }
  stage(id) {
    return this.state.data?.status?.stages?.find((stage) => stage.id === id) || { inputs: [], outputs: [] };
  }
  prerequisite(id) {
    return this.state.data?.monitoring?.prerequisites?.find((item) => item.id === id) || { state: "unknown" };
  }
  ready(id) {
    return this.prerequisite(id)?.state === "ready";
  }
  render() {
    renderTarget();
    renderBlocks();
    this.renderContents();
    if (this.state.view === "storyboard" || this.state.view === "design")
      renderStoryboard();
  }
  command(id, params = {}) {
    const project = this.state.project;
    const run = async () => {
      if (!project || this.state.project !== project)
        throw new Error("The open project changed; command was not submitted");
      this.pending++;
      this.error = null;
      updateAppFooter();
      try {
        const result = await invoke("project_command", { project, id, params });
        if (this.state.project !== project)
          throw new Error("The open project changed; its previous command result was not applied");
        if (result.project && result.project !== project)
          return await applyOpenedProject(result), result;
        this.state.data = result;
        this.render();
        if (result?._ui_engine_revision)
          await acFeedback.acknowledge(result._ui_engine_revision, project);
        return result;
      } catch (error) {
        if (this.state.project === project) {
          this.error = String(error.message || error);
          renderBlocks();
        }
        throw error;
      } finally {
        this.pending--;
        updateAppFooter();
      }
    };
    const result = this.queue.then(run);
    this.queue = result.catch(() => {});
    return result;
  }
  block(id, icon, title, meta, done, body, blockState, action, rerender = () => this.renderBlocks()) {
    const open = state.open === id;
    const failed = blockState === "failed";
    const dot = window.companionBoxes.dot(failed ? "failed" : done ? "ready" : "needs-attention", "", id);
    dot.title = failed ? "Failed · rebuild required" : done ? "Ready" : "Needs work";
    const head = el("button", {
      class: "panel-head",
      id: `block-${id}`,
      "aria-label": title,
      "aria-expanded": String(open),
      title: `${title}${meta ? ` · ${meta}` : ""}`,
      onclick: () => {
        const narrow = getComputedStyle(document.documentElement).getPropertyValue("--workspace-narrow").trim() === "1";
        if (narrow && agent.open) {
          agentToggle(false);
          state.open = id;
        } else
          state.open = open ? null : id;
        rerender();
      }
    }, dot, el("span", { class: "ico", html: icon }), el("span", { class: "title" }, title), el("span", { class: "meta" }, meta || ""), el("span", { class: "chev", html: ICONS.chev }));
    if (window.companionBoxes)
      window.companionBoxes.readiness(id, { state: blockState, reason: failed ? "Failed · rebuild required" : done ? "Ready" : "Needs work" });
    return el("section", { box: id, class: `block${open ? " open" : ""}`, "data-state": blockState, "data-block": id, "data-box-id": id }, action ? el("div", { class: "block-headrow" }, head, action) : head, el("div", { class: "block-body" }, el("div", {}, el("div", { class: "block-inner" }, body))));
  }
  renderContents() {
    const host = $("project-contents");
    if (!host)
      return;
    if (!this.state.project || this.state.creating) {
      host.replaceChildren(this.setupCard(Boolean(this.state.creating)));
      return;
    }
    host.replaceChildren(
      el("div", { class: "project-workspace-stack", id: "project-workspace-stack" },
        this.projectFilesPanel(),
        this.stageAssetsSection(),
        this.setupCard(false)
      )
    );
  }
  fileDetail(file) {
    if (file.badge === "DC" && this.state.data?.storyboard?.page_count) {
      const sb = this.state.data.storyboard;
      const unfilled = sb.count ?? 0;
      return `${sb.page_count} pages · ${unfilled} unfilled`;
    }
    return file.detail || "";
  }
  projectFilesPanel() {
    const rawFiles = this.state.data?.trunk?.files || [];
    const sb = this.state.data?.storyboard;
    const allFiles = rawFiles.map((f) => ({ ...f, displayDetail: this.fileDetail(f) }));
    const counts = {
      all: allFiles.length,
      craft: allFiles.filter((f) => f.is_craft_project).length,
      design_pdf: allFiles.filter((f) => f.group === "design_pdf").length,
      effect_film: allFiles.filter((f) => f.group === "effect_film").length,
      photo_vector: allFiles.filter((f) => f.group === "photo_vector").length,
      texture_3d: allFiles.filter((f) => f.group === "texture_3d").length,
      audio_words: allFiles.filter((f) => f.group === "audio_words").length
    };
    const q = this.pfSearch.trim().toLowerCase();
    const filtered = allFiles.filter((f) => {
      if (this.pfFilter === "craft" && !f.is_craft_project)
        return false;
      if (!["all", "craft"].includes(this.pfFilter) && f.group !== this.pfFilter)
        return false;
      if (q && !`${f.name} ${f.rel_dir} ${f.badge} ${f.group_label}`.toLowerCase().includes(q))
        return false;
      return true;
    });
    const sorted = [...filtered].sort((a, b) => {
      if (this.pfSort === "name")
        return String(a.name).localeCompare(String(b.name));
      if (this.pfSort === "modified")
        return (b.modified_ms || 0) - (a.modified_ms || 0);
      if (this.pfSort === "size")
        return (b.size_bytes || 0) - (a.size_bytes || 0);
      return (a.group_order - b.group_order) || (a.craft_priority - b.craft_priority) || String(a.name).localeCompare(String(b.name));
    });

    const folderIcon = '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>';
    const renderRow = (file, index, rowKey) => el("div", { class: "pf-row", "data-badge": file.badge },
      el("span", { class: "pf-idx" }, String(index + 1)),
      el("span", { class: `pf-badge pf-badge-${String(file.badge || "FILE").toLowerCase()}` }, file.badge || "FILE"),
      el("span", { class: "pf-name", title: file.path }, file.name),
      el("span", { class: "pf-dir", title: file.rel_dir }, file.rel_dir),
      el("span", { class: "pf-meta" }, file.displayDetail),
      el("div", { class: "pf-row-actions" },
        el("button", {
          id: `pf-open-${rowKey}`,
          class: "pf-open-btn",
          title: `${file.open_label} (${file.name})`,
          onclick: () => openPath(file.path)
        }, file.open_label || "Open"),
        el("button", {
          id: `pf-reveal-${rowKey}`,
          class: "pf-reveal-btn",
          title: `Reveal ${file.name} in folder`,
          "aria-label": `Reveal ${file.name} in folder`,
          html: folderIcon,
          onclick: () => this.revealFile(file.path)
        })
      )
    );

    const groupsOrder = [
      ["design_pdf", "Design & PDF"],
      ["effect_film", "Effect & Film"],
      ["photo_vector", "Photo & Vector"],
      ["texture_3d", "3D & Texture"],
      ["audio_words", "Audio & Lyrics"]
    ];
    let rowCounter = 0;
    const listBody = !sorted.length
      ? [el("div", { class: "pf-empty" }, "No project files match the active filter.")]
      : this.pfGroup
        ? groupsOrder.map(([gKey, gLabel]) => {
            const items = sorted.filter((f) => f.group === gKey);
            if (!items.length)
              return null;
            const collapsed = this.pfCollapsedGroups.has(gKey);
            return el("div", { class: `pf-group${collapsed ? " collapsed" : ""}` },
              el("button", {
                id: `pf-group-${gKey}`,
                class: "pf-group-head",
                "aria-expanded": String(!collapsed),
                onclick: () => {
                  if (collapsed)
                    this.pfCollapsedGroups.delete(gKey);
                  else
                    this.pfCollapsedGroups.add(gKey);
                  this.renderContents();
                }
              }, el("span", { class: "pf-group-arrow" }, collapsed ? "▸" : "▾"), ` ${gLabel} (${items.length})`),
              collapsed ? null : el("div", { class: "pf-group-rows" }, ...items.map((item, idx) => renderRow(item, idx, `${gKey}-${rowCounter++}`)))
            );
          })
        : [el("div", { class: "pf-group-rows" }, ...sorted.map((item, idx) => renderRow(item, idx, `flat-${idx}`)))];

    const typeSelect = el("select", {
      id: "pf-type-filter",
      class: "pf-select",
      "aria-label": "Filter project files by type",
      onchange: (e) => {
        this.pfFilter = e.currentTarget.value;
        this.renderContents();
      }
    },
      el("option", { value: "all", selected: this.pfFilter === "all" }, `Type: All Files (${counts.all})`),
      el("option", { value: "craft", selected: this.pfFilter === "craft" }, `Type: Craft Projects (${counts.craft})`),
      el("option", { value: "design_pdf", selected: this.pfFilter === "design_pdf" }, `Type: Design & PDF (${counts.design_pdf})`),
      el("option", { value: "effect_film", selected: this.pfFilter === "effect_film" }, `Type: Effect & Film (${counts.effect_film})`),
      el("option", { value: "photo_vector", selected: this.pfFilter === "photo_vector" }, `Type: Photo & Vector (${counts.photo_vector})`),
      el("option", { value: "texture_3d", selected: this.pfFilter === "texture_3d" }, `Type: 3D & Texture (${counts.texture_3d})`),
      el("option", { value: "audio_words", selected: this.pfFilter === "audio_words" }, `Type: Audio & Lyrics (${counts.audio_words})`)
    );

    const sortSelect = el("select", {
      id: "pf-sort",
      class: "pf-select",
      "aria-label": "Sort project files",
      onchange: (e) => {
        this.pfSort = e.currentTarget.value;
        this.renderContents();
      }
    },
      el("option", { value: "type", selected: this.pfSort === "type" }, "Sort: Type"),
      el("option", { value: "name", selected: this.pfSort === "name" }, "Sort: Name"),
      el("option", { value: "modified", selected: this.pfSort === "modified" }, "Sort: Modified"),
      el("option", { value: "size", selected: this.pfSort === "size" }, "Sort: Size")
    );

    const groupCheckbox = el("input", {
      id: "pf-group-toggle",
      type: "checkbox",
      checked: this.pfGroup,
      defaultChecked: true,
      "aria-label": "Group files by Craft type",
      onchange: (e) => {
        this.pfGroup = Boolean(e.currentTarget.checked);
        this.renderContents();
      }
    });

    const searchInput = el("input", {
      id: "pf-search",
      type: "search",
      class: "pf-search",
      placeholder: "Filter by name...",
      "aria-label": "Filter files by name",
      value: this.pfSearch,
      oninput: (e) => {
        this.pfSearch = e.currentTarget.value;
        const pos = e.currentTarget.selectionStart;
        this.renderContents();
        const next = $("pf-search");
        if (next) {
          next.focus();
          try { next.setSelectionRange(pos, pos); } catch (_) {}
        }
      }
    });

    void sb;
    return el("section", { class: "pf-panel", id: "project-files-panel", "aria-label": "Project Files" },
      el("div", { class: "pf-header" },
        el("strong", { class: "pf-title" }, "Project Files"),
        el("div", { class: "pf-toolbar" },
          typeSelect,
          sortSelect,
          el("label", { class: "pf-group-label", for: "pf-group-toggle" },
            groupCheckbox,
            el("span", {}, "Group")
          ),
          el("div", { class: "pf-search-wrap" }, searchInput),
          el("button", {
            id: "pf-paste",
            class: "pf-btn",
            onclick: () => this.pasteArtwork()
          }, "Paste"),
          el("button", {
            id: "pf-import",
            class: "pf-btn pf-btn-primary",
            onclick: (event) => this.importProjectFile(event)
          }, "+ Import File")
        )
      ),
      el("div", { class: "pf-list", id: "pf-list" }, ...listBody)
    );
  }
  stageAssetsSection() {
    const assets = this.state.data?.assets?.assets || [];
    return stageAssetsPanel(assets, {
      project: this.state.project,
      tab: this.stageTab,
      prefix: "stage-asset",
      onTabChange: (nextTab) => {
        this.stageTab = nextTab;
        this.renderContents();
      },
      onAddAsset: (event) => this.importAsset(event),
      onOpen: (assetId) => {
        if (typeof openAnimation === "function")
          openAnimation(assetId);
      },
      onRemove: async (assetId) => {
        try {
          await this.command("project.remove_asset", { id: assetId });
          toast(`Removed ${assetId}`);
        } catch (err) {
          toast(String(err));
        }
      }
    });
  }
  async revealFile(path) {
    try {
      if (window.__TAURI__?.opener?.revealItemInDir) {
        await window.__TAURI__.opener.revealItemInDir(path);
        toast(`Revealed ${String(path).split("/").pop()} in folder`);
      } else {
        await openPath(path);
      }
    } catch (error) {
      toast(String(error));
    }
  }
  async importProjectFile(event) {
    const supplied = event?.currentTarget?.uiFiles;
    const selected = supplied || await dialog.open({
      title: "Import Studio File",
      multiple: true,
      filters: [
        {
          name: "All Studio Files",
          extensions: ["designcraft", "idml", "deckcraft", "pdf", "pdfcraft", "ecproj", "fcproj", "mp4", "mov", "webm", "pcraft", "psd", "vectorcraft", "vcraft", "svg", "tcraft", "glb", "gltf", "png", "jpg", "jpeg", "webp", "mp3", "wav", "flac", "m4a", "srt", "txt", "md"]
        },
        { name: "Craft Projects", extensions: ["designcraft", "ecproj", "fcproj", "pcraft", "vectorcraft", "tcraft", "deckcraft"] },
        { name: "Artwork & Vectors", extensions: ["png", "jpg", "jpeg", "webp", "svg", "psd", "glb"] }
      ]
    });
    if (!selected)
      return;
    const paths = Array.isArray(selected) ? selected : [selected];
    try {
      await this.command("project.import_file", { paths });
      toast(`Imported ${paths.length} file${paths.length === 1 ? "" : "s"} into project`);
    } catch (error) {
      toast(String(error));
    }
  }
  async pickStageAudio() {
    const path = await dialog.open({
      title: "Choose song audio",
      filters: [{ name: "Audio", extensions: ["wav", "mp3", "flac", "m4a", "aac", "ogg"] }]
    });
    if (!path)
      return;
    try {
      await this.command("workshop.set_audio", { path });
      toast("Audio linked to project");
    } catch (error) {
      toast(String(error));
    }
  }
  async openFilmCraft() {
    try {
      await invoke("engine_command", { id: "workspace.open", params: { app: "filmcraft" } });
      toast("Launched FilmCraft");
    } catch (error) {
      toast(String(error));
    }
  }
  setupCard(creating = false) {
    const reelSvg = `<svg viewBox="0 0 120 120" width="84" height="84" fill="none" stroke="#4fd1c5" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="18" y="12" width="84" height="96" rx="8" fill="#101822" stroke="#2d4f5c" stroke-width="2"/><circle cx="60" cy="52" r="28" stroke="#4fd1c5" stroke-width="2"/><circle cx="60" cy="52" r="22" stroke="#2b7a78" stroke-dasharray="4 3"/><circle cx="60" cy="52" r="7" fill="#101822" stroke="#4fd1c5" stroke-width="2"/><path d="M60 30v12M60 62v12M38 52h12M70 52h12M44 36l9 9M67 59l9 9M76 36l-9 9M53 59l-9 9"/><line x1="28" y1="88" x2="92" y2="88" stroke="#4fd1c5"/><rect x="80" y="66" width="8" height="22" rx="2" fill="#101822" stroke="#4fd1c5"/><rect x="28" y="95" width="32" height="5" rx="2" fill="#2b7a78" stroke="none"/><rect x="68" y="94" width="10" height="7" rx="1" fill="#233140" stroke="none"/><rect x="82" y="94" width="10" height="7" rx="1" fill="#233140" stroke="none"/></svg>`;
    const musicIcon = `<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>`;
    const lyricsIcon = `<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>`;
    const importIcon = `<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r=".5" fill="currentColor"/><circle cx="17.5" cy="10.5" r=".5" fill="currentColor"/><circle cx="8.5" cy="7.5" r=".5" fill="currentColor"/><circle cx="6.5" cy="12.5" r=".5" fill="currentColor"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.9 0 1.6-.7 1.6-1.6 0-.4-.2-.8-.4-1.1-.3-.3-.4-.7-.4-1.1 0-.9.7-1.6 1.6-1.6H16c3.3 0 6-2.7 6-6 0-4.9-4.5-9.6-10-9.6Z"/></svg>`;
    const filmIcon = `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4"/></svg>`;

    const hasProject = Boolean(this.state.project);
    const hasAudioOrWords = Boolean(this.ready("song") || this.ready("lyrics") || this.state.lyrics?.line_count);
    const hasScenes = Boolean(this.ready("storyboard") || (this.state.data?.assets?.assets || []).length > 0);
    const stepRow = (done, label) => el("li", { class: `setup-step-item${done ? " done" : ""}` },
      el("span", { class: `setup-step-box${done ? " checked" : ""}`, "aria-hidden": "true" }, done ? "✓" : ""),
      el("span", {}, label)
    );

    return el("div", { id: "sb-empty", class: "setup-stage-wrap" },
      el("div", { class: "setup-stage-hero" },
        el("div", { class: "setup-stage-reel", "aria-hidden": "true", html: reelSvg }),
        el("strong", { class: "setup-stage-title" }, creating ? "Start with your song" : "Setup Your Studio Stage")
      ),
      el("div", { class: "setup-stage-card" },
        el("p", { class: "setup-stage-sub" }, "Drag your assets here or choose a generator to get started"),
        el("div", { class: "setup-tiles-row" },
          el("button", {
            id: "setup-add-audio",
            class: "setup-tile setup-tile-audio",
            onclick: () => this.pickStageAudio()
          }, el("span", { class: "setup-tile-ico", html: musicIcon }), el("span", { class: "setup-tile-label" }, "Add Music / Audio")),
          el("button", {
            id: "setup-add-lyrics",
            class: "setup-tile setup-tile-lyrics",
            onclick: (event) => songView.attachOriginal(event)
          }, el("span", { class: "setup-tile-ico", html: lyricsIcon }), el("span", { class: "setup-tile-label" }, "Add Lyrics")),
          el("button", {
            id: "sb-add-asset",
            class: "setup-tile setup-tile-import",
            onclick: (event) => this.importProjectFile(event)
          }, el("span", { class: "setup-tile-ico", html: importIcon }), el("span", { class: "setup-tile-label" }, "Import"))
        ),
        el("button", {
          id: "setup-open-filmcraft",
          class: "setup-filmcraft-btn",
          onclick: () => this.openFilmCraft()
        }, el("span", { class: "setup-filmcraft-ico", html: filmIcon }), el("span", {}, "FilmCraft ↗")),
        el("div", { class: "setup-next-steps" },
          el("div", { class: "setup-next-heading" }, "Next Steps"),
          el("ul", { class: "setup-steps-list", "aria-label": "Project workflow" },
            stepRow(hasProject, "Initialize project"),
            stepRow(hasAudioOrWords, "Upload backing audio/words"),
            stepRow(hasScenes, "Draft your scenes")
          )
        )
      )
    );
  }
  appendNotices(box) {
    const receipt = state.projectReceipt;
    if (receipt?.project === state.project && (receipt.problems?.length || receipt.warnings?.length)) {
      box.append(el("details", { class: "project-notice", id: "project-intake-problems" }, el("summary", {}, el("span", { class: "project-notice-dot", "aria-hidden": "true" }), el("strong", {}, receipt.intake_ready === false ? "Setup needs attention" : "Project notices"), el("span", { class: "ico", html: ICONS.chev, "aria-hidden": "true" })), el("div", { class: "project-notice-body" }, el("p", { class: "project-path" }, receipt.project), ...(receipt.problems || []).map((problem) => el("p", {}, String(problem))), ...(receipt.warnings || []).map((warning) => el("p", {}, warning.error || warning.effect || warning.code || String(warning))))));
    }
    if (this.error)
      box.append(el("p", { class: "why", id: "project-command-error", role: "alert" }, this.error));
    if (state.data?.design?.error)
      box.append(el("div", { class: "why", id: "storyboard-read-error", role: "alert" }, el("p", {}, `Cannot read the editable storyboard: ${state.data.design.error}`), el("button", {
        id: "storyboard-retry",
        class: "ghost",
        onclick: () => this.command("storyboard.design").catch((error) => toast(String(error)))
      }, "Retry storyboard")));
  }
  buildSongBlock() {
    const song = this.stage("song"), transcript = this.stage("transcript");
    const audio = song.inputs?.[0], liner = (song.inputs || []).slice(1), doc = transcript.document;
    const outputs = (transcript.outputs || []).filter((c) => c.exists !== false && c.path !== audio?.path);
    const lyricsMeta = state.lyrics?.source === "canonical" ? `${state.lyrics.line_count} lines · ${state.lyrics.sections.length} sections` : doc?.words?.length ? "from transcript" : "";
    const transcriptMeta = doc?.words?.length ? transcriptSummary(doc, this.ready("transcript")) : "";
    const openBtn = doc?.words?.length ? el("button", { id: "song-open-transcript", onclick: () => lyricReaderOpen(state.lyrics, doc, audio?.path, true) }, "Open transcript") : null;
    const transcribeBtn = !doc?.words?.length && audio ? el("button", { id: "song-transcribe", "data-commands": "audio.analyze", onclick: () => this.command("audio.analyze", { analyze_beats: false }).catch((error) => toast(String(error))) }, "Transcribe this recording") : null;
    const transcriptItems = [
      openBtn,
      el("button", { id: "song-import-transcript", "data-commands": "workshop.import_transcript", onclick: importProjectTranscript }, "Import timed lyrics…"),
      transcribeBtn,
      el("details", { class: "song-details" }, el("summary", {}, "Details"), el("div", { class: "why" }, transcript.why || ""), ...outputs.map(fileButton))
    ];
    const audioItem = audio ? fileButton(audio) : null;
    const uploadBtn = el("button", {
      id: "song-upload",
      "data-commands": "workshop.set_audio",
      onclick: async () => {
        const path = await dialog.open({ title: "Choose song audio", filters: [{ name: "Audio", extensions: ["wav", "mp3", "flac", "m4a", "aac", "ogg"] }] });
        if (path)
          try {
            await this.command("workshop.set_audio", { path });
            toast("Audio link saved; review timing and storyboard for this recording.");
          } catch (error) {
            toast(String(error));
          }
      }
    }, audio ? "Change audio…" : "Upload audio…");
    const body = [
      songView.metadataSummary(),
      audioItem,
      uploadBtn,
      songSection("lyrics", "Lyrics & notes", lyricsMeta, this.ready("lyrics"), this.prerequisite("lyrics").state, lyricsBody(state.lyrics, doc, liner, audio?.path)),
      songSection("transcript", "Transcription", transcriptMeta, this.ready("transcript"), this.prerequisite("transcript").state, transcriptItems)
    ];
    return this.block("song", ICONS.song, "Song", audio?.name, this.ready("song"), body, this.prerequisite("song").state, audio ? songPlayButton(audio.path) : null);
  }
  renderBlocks() {
    updateAppFooter();
    const s = state.data?.status;
    const box = $("blocks");
    box.replaceChildren();
    if (!s) {
      if (state.creating)
        renderNewProjectBlocks(box);
      return;
    }
    this.appendNotices(box);
    const toggleLabel = state.blocksCollapsed ? "Expand blocks rail" : "Collapse blocks rail";
    box.append(el("button", {
      class: `icon rail-toggle${state.blocksCollapsed ? "" : " open"}`,
      title: toggleLabel,
      "aria-label": toggleLabel,
      onclick: () => {
        state.blocksCollapsed = !state.blocksCollapsed;
        box.classList.toggle("collapsed", state.blocksCollapsed);
        this.renderBlocks();
      }
    }, el("span", { html: '<svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 9l6 6 6-6"/><path d="M18 3H6c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM6 13l6-6-6-6zM12 15l-6 6 6 6z"/></svg>' })));
    box.append(this.buildSongBlock());
    const assets = state.data?.assets?.assets || [], assetFiles = s.asset_files || [];
    const filesDetails = assetFiles.length ? el("details", {}, el("summary", {}, `Project files (${assetFiles.length})`), ...assetFiles.map(fileButton)) : null;
    const assetsHint = "Click any asset to open and animate it directly in the Animation stage.";
    const assetsBody = [el("div", {class:"actions"}, el("button", { id: "asset-import", class: "start", onclick: (event) => this.importAsset(event) }, "Add asset"),
      el("button", {id:"asset-paste",class:"ghost",onclick:() => this.pasteArtwork()}, "Paste artwork")), projectAssetGrid(assets, { prefix: "asset" }), el("p", { class: "why" }, assetsHint), filesDetails];
    box.append(this.block("assets", ICONS.animate, "Assets", `${assets.length} assets`, this.ready("assets"), assetsBody, this.prerequisite("assets").state));
    box.append(this.block("animate", ICONS.animate, "Animate", `${assets.length} objects`, this.ready("animate"), el("p", { class: "why" }, "Choose an asset above to animate it."), this.prerequisite("animate").state));
    box.append(el("button", { id: "project-close-apps", class: "ghost", onclick: () => closeAppsOpenedByMagic() }, "Close apps opened by Magic"));
    if (typeof imagePromptBlock === "function")
      $("magic-image-tools")?.replaceChildren(imagePromptBlock());
    if (state.view === "design" && typeof wordsStatusRefresh === "function")
      wordsStatusRefresh();
  }
  async pasteArtwork() {
    try {
      await this.command("clipboard.import_image", {expected_project:this.state.project});
      state.open = "assets";
      this.render();
      toast("Clipboard artwork saved · choose where to place it");
    } catch (error) { toast(String(error)); }
  }
  async importAsset(event) {
    const supplied = event?.currentTarget?.uiFiles;
    const selected = supplied || await dialog.open({ title: "Add artwork", multiple: true, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }] });
    if (!selected)
      return;
    const paths = Array.isArray(selected) ? selected : [selected];
    try {
      for (const path of paths) {
        const name = path.split("/").pop().replace(/\.[^.]+$/, "");
        await this.command("project.add_asset", { path, kind: "image", name, image: name, load: false });
      }
      state.open = "assets";
      this.render();
      toast("Assets saved · choose where to place them");
    } catch (e) {
      toast(String(e));
    }
  }
}
const projectView = new ProjectView(state);
window.ProjectView = ProjectView;
window.projectView = projectView;
async function closeAppsOpenedByMagic() {
  try {
    const report = await invoke("close_documents");
    const name = (r) => String(r.document?.file || "").split("/").pop();
    const refused = report.filter((r) => !r.closed);
    if (!report.length)
      toast("No apps opened by Magic are open");
    else if (refused.length)
      toast(`Still open: ${refused.map((r) => `${name(r)} (${r.error})`).join("; ")}`);
    else
      toast(`Saved and closed ${report.map(name).join(", ")}`);
  } catch (error) {
    toast(String(error));
  }
}
function designCommand(id, params = {}) {
  return projectView.command(id, params);
}
function block(id, icon, title, meta, done, body, blockState, action) {
  return projectView.block(id, icon, title, meta, done, body, blockState, action);
}
function projectSetupCard(creating = false) {
  return projectView.setupCard(creating);
}
function appendProjectNotices(box) {
  return projectView.appendNotices(box);
}
function buildSongBlock() {
  return projectView.buildSongBlock();
}
function renderBlocks() {
  return projectView.renderBlocks();
}
async function importProjectAsset(event) {
  return projectView.importAsset(event);
}
