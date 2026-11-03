// One owner for Project commands, readiness and page selection.
class ProjectView {
  error: any;
  pending: any;
  queue: any;
  state: any;
  constructor(state: any) {
    this.state = state;
    this.pending = 0;
    this.error = null;
    this.queue = Promise.resolve();
  }
  stage(id: string) { return this.state.data?.status?.stages?.find((stage: any) => stage.id === id) || { inputs: [], outputs: [] }; }
  prerequisite(id: string) { return this.state.data?.monitoring?.prerequisites?.find((item: any) => item.id === id) || { state: 'unknown' }; }
  ready(id: string) { return this.prerequisite(id)?.state === 'ready'; }
  render() {
    renderTarget();
    renderBlocks();
    $('project-contents')?.replaceChildren(this.setupCard());
    if (this.state.view === 'storyboard' || this.state.view === 'design') renderStoryboard();
  }
  command(id: string, params: Record<string, any> = {}) {
    const project = this.state.project;
    const run = async () => {
      if (!project || this.state.project !== project) throw new Error('The open project changed; command was not submitted');
      this.pending++; this.error = null; updateAppFooter();
      try {
        const result = await invoke('project_command', { project, id, params });
        if (this.state.project !== project) throw new Error('The open project changed; its previous command result was not applied');
        if (result.project && result.project !== project) return await applyOpenedProject(result), result;
        this.state.data = result; this.render();
        if (result?._ui_engine_revision) await acFeedback.acknowledge(result._ui_engine_revision, project);
        return result;
      } catch (error: any) {
        if (this.state.project === project) { this.error = String(error.message || error); renderBlocks(); }
        throw error;
      } finally { this.pending--; updateAppFooter(); }
    };
    const result = this.queue.then(run);
    this.queue = result.catch(() => {});
    return result;
  }

  /** A collapsible rail block. `rerender` redraws the rail that owns it (the Project rail by default). */
  block(id: string, icon: string, title: string, meta: string, done: boolean, body: any, blockState?: any, action?: any, rerender: () => void = () => this.renderBlocks()) {
    const open = state.open === id;
    const failed = blockState === "failed";
    const dot = window.companionBoxes.dot(failed ? "failed" : done ? "ready" : "needs-attention", "", id);
    dot.title = failed ? "Failed · rebuild required" : done ? "Ready" : "Needs work";
    const head = el("button", {
      class: "panel-head", id: `block-${id}`, "aria-label": title, "aria-expanded": String(open), title: `${title}${meta ? ` · ${meta}` : ""}`,
      onclick: () => {
        const narrow = getComputedStyle(document.documentElement).getPropertyValue("--workspace-narrow").trim() === "1";
        if (narrow && agent.open) { agentToggle(false); state.open = id; } else state.open = open ? null : id;
        rerender();
      }
    },
      dot,
      el("span", { class: "ico", html: icon }),
      el("span", { class: "title" }, title),
      el("span", { class: "meta" }, meta || ""),
      el("span", { class: "chev", html: ICONS.chev }));
    if (window.companionBoxes) window.companionBoxes.readiness(id, { state: blockState, reason: failed ? "Failed · rebuild required" : done ? "Ready" : "Needs work" });
    return el("section", { box: id, class: `block${open ? " open" : ""}`, "data-state": blockState, "data-block": id, "data-box-id": id },
      action ? el("div", { class: "block-headrow" }, head, action) : head,
      el("div", { class: "block-body" }, el("div", {}, el("div", { class: "block-inner" }, body))));
  }

  setupCard(creating = false) {
    const preview = el('div', { class: 'project-setup-preview', 'aria-hidden': 'true' },
      el('div', { class: 'project-setup-sheet' }, el('span', { class: 'project-setup-art', html: ICONS.animate }),
        el('span', { class: 'project-setup-line' }), el('div', { class: 'project-setup-fields' }, ...Array.from({ length: 4 }, () => el('span')))));
    const steps = el('ol', { class: 'project-setup-steps', 'aria-label': 'Project workflow' },
      ...[['song', 'Recording'], ['lyrics', 'Lyrics'], ['animate', 'Animate']].map(([icon, label]) =>
        el('li', {}, el('span', { class: 'ico', html: ICONS[icon], 'aria-hidden': 'true' }), el('span', {}, label))));
    return el('div', { id: 'sb-empty', class: 'panel-empty project-setup' }, preview,
      el('strong', {}, creating ? 'Start with your song' : 'Project workspace'),
      el('p', {}, creating ? 'Add a recording in the Song panel.' : 'Add audio, lyrics, or 2D/3D/CAD assets to animate directly.'), steps,
      creating ? null : el('button', {
        id: 'sb-add-asset', class: 'start',
        onclick: (event: any) => this.importAsset(event)
      }, 'Add asset'));
  }

  appendNotices(box: any) {
    const receipt = state.projectReceipt;
    if (receipt?.project === state.project && (receipt.problems?.length || receipt.warnings?.length)) {
      box.append(el('details', { class: 'project-notice', id: 'project-intake-problems' },
        el('summary', {}, el('span', { class: 'project-notice-dot', 'aria-hidden': 'true' }),
          el('strong', {}, receipt.intake_ready === false ? 'Setup needs attention' : 'Project notices'),
          el('span', { class: 'ico', html: ICONS.chev, 'aria-hidden': 'true' })),
        el('div', { class: 'project-notice-body' }, el('p', { class: 'project-path' }, receipt.project),
          ...(receipt.problems || []).map((problem: any) => el('p', {}, String(problem))),
          ...(receipt.warnings || []).map((warning: any) => el('p', {}, warning.error || warning.effect || warning.code || String(warning))))));
    }
    if (this.error) box.append(el('p', { class: 'why', id: 'project-command-error', role: 'alert' }, this.error));
    if (state.data?.design?.error) box.append(el('div', { class: 'why', id: 'storyboard-read-error', role: 'alert' },
      el('p', {}, `Cannot read the editable storyboard: ${state.data.design.error}`),
      el('button', {
        id: 'storyboard-retry', class: 'ghost',
        onclick: () => this.command('storyboard.design').catch((error: any) => toast(String(error)))
      }, 'Retry storyboard')));
  }

  buildSongBlock() {
    const song = this.stage('song'), transcript = this.stage('transcript');
    const audio = song.inputs?.[0], liner = (song.inputs || []).slice(1), doc = transcript.document;
    const outputs = (transcript.outputs || []).filter((c: any) => c.exists !== false && c.path !== audio?.path);
    const lyricsMeta = state.lyrics?.source === 'canonical' ? `${state.lyrics.line_count} lines · ${state.lyrics.sections.length} sections` : (doc?.words?.length ? 'from transcript' : '');
    const transcriptMeta = doc?.words?.length ? transcriptSummary(doc, this.ready('transcript')) : '';
    const openBtn = doc?.words?.length ? el('button', { id: 'song-open-transcript', onclick: () => lyricReaderOpen(state.lyrics, doc, audio?.path, true) }, 'Open transcript') : null;
    const transcribeBtn = !doc?.words?.length && audio ? el('button', { id: 'song-transcribe', 'data-commands': 'audio.analyze', onclick: () => this.command('audio.analyze', { analyze_beats: false }).catch((error: any) => toast(String(error))) }, 'Transcribe this recording') : null;
    const transcriptItems = [openBtn,
      el('button', { id: 'song-import-transcript', 'data-commands': 'workshop.import_transcript', onclick: importProjectTranscript }, 'Import timed lyrics…'),
      transcribeBtn,
      el('details', { class: 'song-details' }, el('summary', {}, 'Details'), el('div', { class: 'why' }, transcript.why || ''), ...outputs.map(fileButton))];
    const audioItem = audio ? fileButton(audio) : null;
    const uploadBtn = el('button', {
      id: 'song-upload', 'data-commands': 'workshop.set_audio', onclick: async () => {
        const path = await dialog.open({ title: 'Choose song audio', filters: [{ name: 'Audio', extensions: ['wav', 'mp3', 'flac', 'm4a', 'aac', 'ogg'] }] });
        if (path) try { await this.command('workshop.set_audio', { path }); toast('Audio link saved; review timing and storyboard for this recording.'); } catch (error) { toast(String(error)); }
      }
    }, audio ? 'Change audio…' : 'Upload audio…');
    const body = [songView.metadataSummary(), audioItem, uploadBtn,
      songSection('lyrics', 'Lyrics & notes', lyricsMeta, this.ready('lyrics'), this.prerequisite('lyrics').state, lyricsBody(state.lyrics, doc, liner, audio?.path)),
      songSection('transcript', 'Transcription', transcriptMeta, this.ready('transcript'), this.prerequisite('transcript').state, transcriptItems)];
    return this.block('song', ICONS.song, 'Song', audio?.name, this.ready('song'), body, this.prerequisite('song').state, audio ? songPlayButton(audio.path) : null);
  }

  renderBlocks() {
    updateAppFooter();
    const s = state.data?.status;
    const box = $("blocks");
    box.replaceChildren();
    if (!s) { if (state.creating) renderNewProjectBlocks(box); return; }
    this.appendNotices(box);
    const toggleLabel = state.blocksCollapsed ? 'Expand blocks rail' : 'Collapse blocks rail';
    box.append(el('button', {
      class: `icon rail-toggle${state.blocksCollapsed ? '' : ' open'}`, title: toggleLabel, 'aria-label': toggleLabel,
      onclick: () => { state.blocksCollapsed = !state.blocksCollapsed; box.classList.toggle('collapsed', state.blocksCollapsed); this.renderBlocks(); }
    },
      el('span', { html: '<svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 9l6 6 6-6"/><path d="M18 3H6c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM6 13l6-6-6-6zM12 15l-6 6 6 6z"/></svg>' })));
    box.append(this.buildSongBlock());
    const assets = state.data?.assets?.assets || [], assetFiles = s.asset_files || [];
    const filesDetails = assetFiles.length ? el('details', {}, el('summary', {}, `Project files (${assetFiles.length})`), ...assetFiles.map(fileButton)) : null;
    const assetsHint = 'Click any asset to open and animate it directly in the Animation stage.';
    const assetsBody = [el('button', { id: 'asset-import', class: 'start', onclick: (event: any) => this.importAsset(event) }, 'Add asset'), projectAssetGrid(assets, { prefix: 'asset' }), el('p', { class: 'why' }, assetsHint), filesDetails];
    box.append(this.block('assets', ICONS.animate, 'Assets', `${assets.length} assets`, this.ready('assets'), assetsBody, this.prerequisite('assets').state));
    box.append(this.block('animate', ICONS.animate, 'Animate', `${assets.length} objects`, this.ready('animate'),
      el('p', { class: 'why' }, 'Choose an asset above to animate it.'), this.prerequisite('animate').state));
    if (typeof imagePromptBlock === 'function') $('magic-image-tools')?.replaceChildren(imagePromptBlock());
    if (state.view === 'design' && typeof wordsStatusRefresh === 'function') wordsStatusRefresh();
  }

  async importAsset(event: any) {
    const supplied = event?.currentTarget?.uiFiles;
    const selected = supplied || await dialog.open({ title: 'Add artwork', multiple: true, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] }); if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    try {
      for (const path of paths) { const name = path.split('/').pop().replace(/\.[^.]+$/, ''); await this.command('project.add_asset', { path, kind: 'image', name, image: name, load: false }); }
      state.open = 'assets'; this.renderBlocks(); toast('Assets saved · choose where to place them');
    } catch (e) { toast(String(e)); }
  }
}

const projectView = new ProjectView(state);
(window as any).ProjectView = ProjectView;
(window as any).projectView = projectView;
function designCommand(id: string, params: Record<string, any> = {}) { return projectView.command(id, params); }
function block(id: string, icon: string, title: string, meta: string, done: boolean, body: any, blockState?: any, action?: any) {
  return projectView.block(id, icon, title, meta, done, body, blockState, action);
}
function projectSetupCard(creating = false) { return projectView.setupCard(creating); }
function appendProjectNotices(box: any) { return projectView.appendNotices(box); }
function buildSongBlock() { return projectView.buildSongBlock(); }
function renderBlocks() { return projectView.renderBlocks(); }
async function importProjectAsset(event: any) { return projectView.importAsset(event); }
