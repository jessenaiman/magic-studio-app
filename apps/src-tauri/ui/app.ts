// Magic Studio shell: the project view. Data comes from magicstudio-engine through the
// shell's commands; files open with one click; the studio tab hosts the video target's studio.
const { invoke: nativeInvoke, convertFileSrc } = window.__TAURI__.core;
// Engine commands say who asked: a control click or a callback, and which request it belongs to.
function stampEngineParams(params: any, action: any, companion: any): any {
  return {...params, params: {...params.params, _origin: action ? 'UI click' : 'UI callback', _client: 'Studio user (unverified label)', _control: action?.control, _request_id: action?.request_id || params.params?._request_id || companion?.request}};
}

// Opening or creating a project carries the control and request that triggered it.
function stampOpenParams(params: any, action: any): any {
  return {...params, request: params?.request ?? action?.request_id ?? null, control: params?.control ?? action?.control ?? null};
}

function reportTransportError(command: string, params: any, action: any, error: unknown): void {
  const message = String(error).slice(0, 300);
  acFeedback.last = {transport: command, id: params?.id || null, status: 'error', error: message};
  acFeedback.report(null, null, {stage: 'handler_error', command: params?.id || command, control: action?.control || null, request_id: action?.request_id || null, error: message});
}

function noteTransportSuccess(command: string, params: any, companion: any, result: any): void {
  acFeedback.last = {transport: command, id: params?.id || null, status: 'success'};
  if (companion) window.uiRegistry?.event({...companion, phase: 'success', result});
  if (result?._ui_engine_revision) acFeedback.pendingRevision = result._ui_engine_revision;
}

// `options` carries transport options, e.g. request headers for raw byte payloads.
async function invoke(command: string, params?: any, options?: any) {
  const action = acFeedback.action;
  const companion = command === 'engine_command' ? window.uiRegistry?.capture(params.id, {...params.params, _request_id: action?.request_id || params.params?._request_id}) : null;
  if (companion) window.uiRegistry?.event({...companion, phase: "start"});
  acFeedback.pending++; await acFeedback.report();
  if (command === 'engine_command' && params) params = stampEngineParams(params, action, companion);
  if (command === 'project_create' || command === 'project_open') params = stampOpenParams(params, action);
  acFeedback.report(null, null, {stage: 'handler_dispatch', control: action?.control || null, request_id: action?.request_id || null, command: params?.id || command});
  try {
    const result = await nativeInvoke(command, params, options);
    noteTransportSuccess(command, params, companion, result);
    return result;
  } catch (error) {
    if (companion) window.uiRegistry?.event({...companion, phase: 'error', error: String(error)});
    reportTransportError(command, params, action, error);
    throw error;
  } finally { acFeedback.pending--; await acFeedback.report(); }
}
const $ = (id: string): any => document.getElementById(id);

const ICONS = {
  song: '<svg viewBox="0 0 24 24"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>',
  lyrics: '<svg viewBox="0 0 24 24"><path d="M4 6h16M4 11h16M4 16h10"/></svg>',
  transcript: '<svg viewBox="0 0 24 24"><path d="M3 12h2M7 8v8M11 5v14M15 9v6M19 11v2"/></svg>',
  animate: '<svg viewBox="0 0 24 24"><path d="M12 3l9 5-9 5-9-5 9-5z"/><path d="M3 13l9 5 9-5"/></svg>',
  chev: '<svg viewBox="0 0 24 24"><path d="M9 6l6 6-6 6"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M8 5l11 7-11 7z"/></svg>',
  pause: '<svg viewBox="0 0 24 24"><path d="M8 5v14M16 5v14"/></svg>',
};

// The app's one mutable state bag. Fields below are the ones set at startup; views add the rest
// (storyboardScene, lyrics, creating, ...) as they run, hence the open index signature.
interface AppState {
  project: string | null;
  data: any;
  page: number;
  open: string | null;
  view: string;
  blocksCollapsed: boolean;
  [key: string]: any;
}
let state: AppState = { project: null, data: null, page: 0, open: "song", view: "project", blocksCollapsed: false };

function updateMagicSelection() {
  const layer = motion?.meta?.layers?.find((part) => part.id === motion.selected);
  const text = layer ? `${layer.label || layer.id} · ${layer.id}` : 'No part selected';
  updateAppFooter();
  for (const id of ['magic-target-image']) {
    const node = $(id);
    if (node) { node.textContent = text; node.title = layer ? `Selected part ${layer.label || layer.id} (${layer.id}) on ${motion.character}` : 'Select a part to target image generation'; }
  }
}

function projectAssetGrid(assets,{active=null,prefix='magic-asset'}={}) {
  return el('div',{class:'magic-assets-grid'},assets.map(asset=>magicAssetTile(asset,{active:asset.id===active,prefix,onclick:()=>openAnimation(asset.id),onremove:async()=>{
    try{
      await designCommand('project.remove_asset',{id:asset.id});
      if(state.view==='animation')await animationRefresh();
      toast(`Removed ${asset.name||asset.id}`);
    }catch(e){toast(String(e));}
  }})));
}

function magicAssetTile(asset, { active = false, candidate = false, prefix='magic-asset', onclick, onremove = null }) {
  const name = asset.name || asset.id;
  const image = /\.(png|jpe?g|webp|gif)$/i.test(asset.path || '')
    ? el('img', { src: convertFileSrc(asset.path), alt: '', draggable: 'false' })
    : el('span', { class: 'magic-asset-placeholder' }, 'PART');
  const remove = onremove ? el('span', {
    class: 'forget asset-forget',
    id: `${prefix}-remove-${asset.id}`,
    role: 'button',
    tabindex: '0',
    'data-commands': 'project.remove_asset',
    'aria-label': `Remove ${name} (${asset.id})`,
    title: `Remove ${name} from project`,
    html: '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>',
    onclick: (e) => { e.stopPropagation(); onremove(); },
    onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); onremove(); } }
  }) : null;
  const tile=el('button', { class: `magic-asset-tile${active ? ' active' : ''}${candidate ? ' candidate' : ''}`, id: `${prefix}-${asset.id}`,draggable:'true',
    title: candidate ? `${name} · generated candidate awaiting review` : `${name} (${asset.id}) · drag onto a storyboard page or click to animate`, onclick },
    image, el('span', { class: 'magic-asset-name' }, name),
    candidate ? el('span', { class: 'magic-asset-badge' }, 'Candidate') : el('span', { class: 'magic-asset-id' }, asset.id),
    remove);
  tile.addEventListener('dragstart',event=>{event.dataTransfer.setData('application/x-magicstudio-asset',asset.id);event.dataTransfer.setData('text/plain',asset.id);event.dataTransfer.effectAllowed='copy';});return tile;
}

// ---- Unified footer: workflow stages · what the active tab shows · save and studio state ----
const FOOTER_STAGES = [['song', 'Song'], ['lyrics', 'Lyrics'], ['transcript', 'Transcript'], ['storyboard', 'Storyboard'], ['animate', 'Animate'], ['studio', 'Studio']];
const STAGE_DOT: Record<string, string> = { ok: 'ready', warn: 'needs-attention', idle: 'idle' };
const STAGE_NOTE: Record<string, string> = { ok: 'ready', warn: 'needs work', idle: 'not started' };

class AppFooterController {
  queued = false;

  stageState(id: string) {
    if (id === 'studio') return serverUrl() ? 'ok' : videoProject() ? 'warn' : 'idle';
    const seen = projectView.prerequisite(id).state;
    return seen === 'ready' ? 'ok' : seen ? 'warn' : 'idle';
  }

  async go(id: string) {
    if (id === 'studio') return setView('studio');
    if (state.view !== 'project' && await setView('project') === false) return;
    state.open = SONG_SECTIONS.includes(id) ? 'song' : id;
    state.blocksCollapsed = false;
    $('blocks').classList.remove('collapsed');
    renderBlocks();
  }

  animationText(): string {
    const meta = motion.meta;
    if (motion.mode === 'composite') return 'Composite · images arranged in time';
    if (!meta) return motion.character ? `${motion.character} · loading…` : 'No object chosen';
    const clips = (meta.tracks || []).filter((t: any) => !t.standard).length;
    const track = motion.rest ? 'Rest pose' : (meta.tracks.find((t: any) => t.id === motion.track)?.label || motion.track.replaceAll('-', ' '));
    const part = meta.layers.find((l: any) => l.id === motion.selected);
    return `${motion.character} · ${meta.layers.length} layers / ${clips} clips · ${track} · Selected: ${part ? part.label || part.id : 'none'}`;
  }

  storyboardText(): string {
    const pages = state.data?.storyboard?.previews?.length || 0;
    const objects = (state.data?.assets?.assets || []).length;
    return [
      pages ? `Storyboard · Page ${state.page + 1} / ${pages}` : 'Storyboard · not built yet',
      pages ? state.pageTitles?.[state.page] : null,
      `${objects} object${objects === 1 ? '' : 's'}`,
    ].filter(Boolean).join(' · ');
  }

  studioText(): string {
    const project = videoProject();
    return project ? `Video project: ${project}` : 'No video project yet';
  }

  contextText(): string {
    if (state.view === 'home') return 'Choose a project or start a new song';
    if (state.view === 'animation') return this.animationText();
    if (state.view === 'studio') return this.studioText();
    if (state.view === 'analytics') return 'Mayhem · code, usage and tool reports';
    return this.storyboardText();
  }

  scheduleUpdate(): void {
    if (this.queued) return;
    this.queued = true;
    queueMicrotask(() => { this.queued = false; this.render(); });
  }

  renderStages(): void {
    $('footer-stages').replaceChildren(...FOOTER_STAGES.map(([id, label]) => {
      const tone = this.stageState(id);
      return el('button', {
        class: 'footer-stage', id: `footer-stage-${id}`, title: `${label} · ${STAGE_NOTE[tone]}`,
        onclick: () => this.go(id).catch((e) => toast(String(e))),
      }, window.companionBoxes.dot(STAGE_DOT[tone], '', `footer-stage-${id}`), label);
    }));
  }

  saveStatus(unsaved: boolean): { tone: string; label: string } {
    if (projectView.pending > 0) return { tone: 'working', label: 'Saving…' };
    if (unsaved) return { tone: 'needs-attention', label: 'Unsaved edits' };
    return state.project ? { tone: 'ready', label: 'Project saved' } : { tone: 'idle', label: 'Not saved' };
  }

  isMotionUnsaved(): boolean {
    return state.view === 'animation'
      && typeof magicTheatre !== 'undefined'
      && (magicTheatre.interactionRevision > magicTheatre.savedRevision || Boolean(magicTheatre.gestureActive));
  }

  saveStateTitle(motionUnsaved: boolean, unsaved: boolean): string {
    if (motionUnsaved) return 'Theatre motion has unsaved keyframes · use Save motion';
    if (unsaved) return 'Storyboard changes need saving';
    return 'Magic Studio project state; HyperFrames saves composition edits in its own editor';
  }

  renderSaveState(): void {
    const motionUnsaved = this.isMotionUnsaved();
    const unsaved = motionUnsaved || state.data?.design?.dirty === true || projectView.textDrafts.size > 0;
    const { tone, label } = this.saveStatus(unsaved);
    $('footer-save-state').replaceChildren(window.companionBoxes.dot(tone, '', 'footer-save-state'), label);
    $('footer-save-state').title = this.saveStateTitle(motionUnsaved, unsaved);
  }

  renderServer(): void {
    const url = serverUrl();
    $('footer-server').replaceChildren(
      window.companionBoxes.dot(url ? 'ready' : 'idle', '', 'target'),
      url ? `Studio · ${url.replace(/^https?:\/\//, '').replace(/\/$/, '')}` : 'Studio off',
    );
  }

  render(): void {
    if (!$('app-footer')) return;
    this.renderStages();
    updateStoryboardSaveControls();
    const context = this.contextText();
    $('footer-context').textContent = context;
    $('footer-context').title = context;
    this.renderSaveState();
    this.renderServer();
  }
}

const appFooter = new AppFooterController();

function footerStageState(id: string) { return appFooter.stageState(id); }
async function footerGo(id: string) { return appFooter.go(id); }
function animationFooter(): string { return appFooter.animationText(); }
function storyboardFooter(): string { return appFooter.storyboardText(); }
function studioFooter(): string { return appFooter.studioText(); }
function footerContext(): string { return appFooter.contextText(); }
function updateAppFooter(): void { appFooter.scheduleUpdate(); }
function renderFooterStages(): void { appFooter.renderStages(); }
function saveStatus(unsaved: boolean): { tone: string; label: string } { return appFooter.saveStatus(unsaved); }
function renderFooterSaveState(): void { appFooter.renderSaveState(); }
function renderFooterServer(): void { appFooter.renderServer(); }
function renderAppFooter(): void { appFooter.render(); }
// Theatre's own listener (theatre.js) updates its revisions first; render after it.
window.addEventListener('message', (event) => { if (event.data?.magicTheatre && event.data.type !== 'values') setTimeout(updateAppFooter); });

let toastTimer: ReturnType<typeof setTimeout> | undefined;
function toast(text: string) {
  const t = $("toast");
  t.textContent = text;
  t.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("show"), 2400);
}

// Every control in this app is built here.
//
// `el()` is therefore the ONE place a control joins the shared contracts, and
// it inherits BOTH of them:
//
//   * THE BOX -- a stable id, the project and context it belongs to, and the
//     state the app knows about it (idle, working, ready, needs-attention,
//     failed) with its reason, priority, request, job and resolution. Every box
//     exposes the same "scent", so the app light, the companion and the TUI all
//     react to one observation instead of three guesses.
//
//   * THE SLOT -- membership of a rail. A slot declares a stable id, the
//     destinations or roles it accepts, and an explicit priority. Every rail is
//     the same reusable component, so a control can join a rail without
//     knowing anything about how rails work.
//
// A control is in both contracts BY CONSTRUCTION: there is no per-block
// companion logic anywhere and no block has to opt in.
//
// Box ids must be STABLE across renders, so they come from explicit intent
// first (the `box` attribute, then the element's own id) and only fall back
// to a label. Labels change; ids are contracts.
// THE CONFIG. One entry per element id says which component it is, which look
// it takes and how dense it is. el() reads it by id, so a look is chosen in
// the config, never at the call site.
const uiConfig: UiConfig = { version: 0, looks: {}, densities: [], elements: {} };
window.uiConfig = uiConfig;

async function loadComponents() {
  const response = await fetch("components.json");
  if (!response.ok) throw new Error(`components.json ${response.status}`);
  Object.assign(uiConfig, await response.json());
  for (const [id, entry] of Object.entries(uiConfig.elements)) {
    const node = document.getElementById(id);
    if (node) { applyEntry(node, entry, id); node.setAttribute("data-box-id", id); window.companionBoxes?.register(id, { project: state.project, ...entry }); }
  }
  for (const node of document.body.querySelectorAll("*")) window.uiRegistry?.registerElement(node);
  return uiConfig;
}

function applyEntry(node, entry, id = node.id) {
  if (!entry) return node;
  node.dataset.uiConfig = id;
  if (entry.component) node.className = [...new Set(`${node.className || ''} ${entry.component}`.trim().split(/\s+/))].join(' ');
  if (entry.look) node.dataset.look = entry.look;
  if (entry.density) node.dataset.density = entry.density;
  if (entry.scroll) node.dataset.scroll = entry.scroll;
  if (entry.component === 'panel' && entry.rail && !node.querySelector('[data-companion-slot]')) {
    const head = node.querySelector('.panel-head');
    if (!head) { node.dataset.contractError = 'Panel has no shared header'; return node; }
    delete node.dataset.contractError;
    node.setAttribute('data-companion-rail', entry.rail);
    const slot = document.createElement('span');
    slot.className = 'panel-rail'; slot.setAttribute('data-companion-slot', entry.rail);
    slot.setAttribute('aria-hidden', 'true'); head.append(slot);
    window.uiRegistry?.registerElement(slot);
  }
  return node;
}

// A listener runs with the clicked control's action recorded, so feedback can say what the user did.
function runListener(listener: (event: Event) => unknown, event: Event): unknown {
  const previous = acFeedback.action;
  acFeedback.action = event.acAction;
  try { return listener(event); } finally { acFeedback.action = previous; }
}

// One attribute: class, html, an on* listener, a boolean property, or a plain attribute.
function applyAttribute(node: any, key: string, value: any): void {
  if (key === "class") node.className = value;
  else if (key === "html") node.innerHTML = value;
  else if (key.startsWith("on")) node.addEventListener(key.slice(2), (event: Event) => runListener(value, event));
  else if (typeof value === "boolean" && key in node) node[key] = value;
  else node.setAttribute(key, value);
}

// Apply the element's components.json entry and register it with the companion boxes and registry.
function registerElement(node: any, boxId: string | null): void {
  const entry = boxId ? uiConfig.elements[boxId] : null;
  if (entry) applyEntry(node, entry, boxId);
  if (boxId) {
    node.setAttribute("data-box-id", boxId);
    const project = typeof state === "undefined" ? null : state.project;
    if (window.companionBoxes) window.companionBoxes.register(boxId, { project, label: entry?.label || null, look: entry?.look || null, density: entry?.density || null });
  }
  window.uiRegistry?.registerElement(node);
}

// Create an element from a tag, attributes and children.
function el(tag: string, attrs: Record<string, any> = {}, ...children: any[]): any {
  const node = document.createElement(tag);
  let box = null;
  for (const [key, value] of Object.entries(attrs)) {
    if (value === null || value === undefined) continue;
    if (key === "box") box = value;
    else applyAttribute(node, key, value);
  }
  for (const child of children.flat()) if (child != null) node.append(child);
  registerElement(node, box || attrs.id || null);
  return node;
}

// Copyable examples use public project targeting; execution stays in the shared engine.
function magicCommandText(id,params={},project=state.project){
  const quote=value=>`'${String(value).replaceAll("'","'\\''")}'`;
  return `magic-cli${project ? ` --project ${quote(project)}` : ''} exec ${id}${Object.entries(params).map(([key,value])=>` ${quote(`${key}=${typeof value==='object'?JSON.stringify(value):value}`)}`).join('')}`;
}

const kindOf = (name) => (name.split(".").pop() || "file").toUpperCase().slice(0, 4);

function fileButton(chip) {
  return el("button", { class: "file", title: chip.path, "aria-label": `Open ${chip.name}`, onclick: () => openPath(chip.path) },
    el("span", { class: "kind" }, kindOf(chip.name)), el("span", { class: "name" }, chip.name));
}

async function openPath(path) {
  try { await invoke("open_path", { path }); toast(`Opened ${path.split("/").pop()}`); }
  catch (e) { toast(String(e)); }
}

// ---- Blocks -------------------------------------------------------------------------------

const SONG_SECTIONS = ['lyrics', 'transcript'];

// What each workspace does when it opens. Wrapped in arrows: the views load after this file.
const WORKSPACE_OPENERS: Record<string, () => unknown> = {
  animation: () => animationOpen(),
  design: () => wordsOpen(),
  skills: () => { if (typeof renderResources === "function") renderResources(); },
  analytics: () => { if (typeof renderAnalytics === "function") renderAnalytics(); },
};

class AppShellController {
  placeInk() {
    window.workspaceNavigation?.select(state.view);
  }

  async showStudio() {
    const url = this.serverUrl();
    $("studio-empty").classList.toggle("show", !url);
    if (!url) return;
    try { await invoke("studio_show", { url, mode: state.view }); }
    catch (e) { toast(String(e)); throw e; }
  }

  async openWorkspace(view: string): Promise<void> {
    const open = WORKSPACE_OPENERS[view];
    if (open) { await invoke("studio_hide"); await open(); return; }
    if (view !== "project" && this.serverUrl()) {
      await new Promise(requestAnimationFrame);
      await this.showStudio();
      return;
    }
    await invoke("studio_hide");
    if (view === "project") return;
    document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === "view-studio"));
    $("studio-empty").classList.add("show");
  }

  canLeaveCurrentView(nextView: string): boolean {
    if (state.view === "animation" && nextView !== "animation" && typeof theatreCanLeave === "function") {
      return Boolean(theatreCanLeave());
    }
    return true;
  }

  cleanupPreviousView(nextView: string) {
    if (typeof animationPause === "function") animationPause();
    if (nextView !== 'animation' && typeof animationJobsDispose === 'function') animationJobsDispose();
  }

  async setView(view: string, action?: any) {
    const errors = acFeedback.errors.length;
    if (view === "split") view = "studio";
    if (!this.canLeaveCurrentView(view)) return false;
    this.cleanupPreviousView(view);
    state.view = view;
    window.workspaceNavigation?.select(view);
    const page = view === "studio" ? "project" : view;
    document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${page}`));
    this.placeInk();
    await this.openWorkspace(view);
    updateAppFooter();
    window.magicCompanions?.sync();
    if (action && feedbackSnapshot().rendered && acFeedback.errors.length === errors) {
      await acFeedback.report(null, null, { stage: "ui_navigation_applied", ...action, view: state.view });
    }
  }

  updateStoryboardAccess() {
    window.workspaceNavigation?.readiness(
      state.data?.design?.progression?.allowed === true || state.animateOnly === true,
      state.data?.design?.progression?.reason,
    );
  }

  serverUrl() {
    const server = state.data?.status?.video_server || {};
    return server.running === true ? server.url : null;
  }

  videoProject() {
    const s = state.data?.status;
    return s?.video_project_path || s?.video_project || null;
  }

  renderTarget() {
    const s = state.data?.status;
    if (!s) return;
    const url = this.serverUrl();
    const project = this.videoProject();
    $("target-name").textContent = `${s.video_target}${url ? " · " + url.replace(/^https?:\/\//, "").replace(/\/$/, "") : " · off"}`;
    const serverDot = window.companionBoxes.dot(url ? "ready" : "idle", "", "target");
    serverDot.id = "server-dot";
    $("server-dot").replaceWith(serverDot);
    updateAppFooter();
    const statusLine = url ? `Studio running at ${url} · click to stop` : "Studio off · click to start";
    const projectLine = project ? `Video project: ${project}` : "No video project yet · starting the studio creates it";
    $("target").title = `${statusLine}\n${projectLine}`;
  }

  async toggleServer() {
    if (!state.project) return;
    const on = !this.serverUrl();
    const startingDot = window.companionBoxes.dot("working", "pulse", "target");
    startingDot.id = "server-dot";
    $("server-dot").replaceWith(startingDot);
    try {
      if (on && state.data?.status?.video_target && !state.data.status.video_project_path) {
        await designCommand("video.init", {});
      }
      await designCommand(on ? "video.server_start" : "video.server_stop");
      toast(on ? "Studio started" : "Studio stopped");
    } catch (e) { toast(String(e)); }
    this.renderTarget();
    if (["studio", "split"].includes(state.view)) (this.serverUrl() ? this.showStudio() : this.setView(state.view));
  }

  renderInstanceIdentity(identity: any = {}) {
    $('footer-instance').textContent = `Studio ${identity.pid ?? 'unknown'} · ${identity.branch || 'unknown build'}`;
    $('footer-instance').title = `${identity.build_id || 'Unknown build'} · PID ${identity.pid ?? '?'} · ${identity.executable || ''} · Click to copy the CLI command`;
  }

  async load() {
    await loadComponents();
    const start: any = await invoke("startup");
    state.identity = start.identity;
    window.applicationFileMenu?.configure(start.menus.file);
    window.textContextMenu?.configure(start.menus.text);
    if (start.character && typeof motion !== "undefined") {
      motion.character = start.character;
      motion.explicitCharacter = true;
    }
    this.renderInstanceIdentity(start.identity);
    if (start.new_project) return beginNewProject();
    if (!start.project) return showHome();
    await this.openProject(start.project, start);
  }

  async openProject(path: string, start: any = {}) {
    try { return await this.applyOpenedProject(await invoke("project_open", { path }), start); }
    catch (e) { toast(String(e)); throw e; }
  }

  resetProjectTransientState(path: string) {
    if (state.project === path) return;
    animationReset();
    songStop();
    transcriptReaderClose();
    projectView.textDrafts.clear();
    projectView.error = null;
    state.storyboardScene = null;
    state.storyboardUndo = null;
  }

  updateWindowProjectTitle(path: string) {
    const title = path.split("/").pop()?.replace(/\.magic$/, "") || "";
    $("project-name").textContent = title;
    $("project-name").parentElement.title = title;
    document.title = `${title} · Magic Studio`;
  }

  async applyOpenedProject(opened: any, start: any = {}) {
    if (opened.reused_session) {
      toast(opened.opened_new_instance ? "Opened project in a separate window" : "Focused the existing project session");
      return;
    }
    const path = opened.project;
    if (!path) throw new Error('Project command did not return its canonical location');
    state.data = opened;
    state.projectReceipt = opened;
    state.animateOnly = opened.mode === "animation";
    document.body.classList.toggle("animation-mode", opened.mode === "animation");
    this.resetProjectTransientState(path);
    state.project = path;
    state.lyrics = null;
    loadSongLyrics();
    window.magicCompanions?.sync();
    state.page = 0;
    const startOpen = start.open && start.open !== "agent" ? start.open : "song";
    state.open = SONG_SECTIONS.includes(startOpen) ? "song" : startOpen;
    document.body.classList.remove("at-home");
    this.updateWindowProjectTitle(path);
    this.renderTarget();
    renderBlocks();
    renderStoryboard();
    await this.setView(start.view || (opened.mode === "animation" ? "animation" : "project"));
    window.workshopDrawers?.close(false);
    await acFeedback.acknowledge(state.data?._ui_engine_revision, path);
  }
}

const appShell = new AppShellController();

function placeInk() { appShell.placeInk(); }
async function showStudio() { return appShell.showStudio(); }
async function openWorkspace(view: string): Promise<void> { return appShell.openWorkspace(view); }
async function setView(view: string, action?: any) { return appShell.setView(view, action); }
function updateStoryboardAccess() { appShell.updateStoryboardAccess(); }
function serverUrl() { return appShell.serverUrl(); }
function videoProject() { return appShell.videoProject(); }
function renderTarget() { appShell.renderTarget(); }
async function toggleServer() { return appShell.toggleServer(); }
async function load() { return appShell.load(); }
async function openProject(path: string, start: any = {}) { return appShell.openProject(path, start); }
async function applyOpenedProject(opened: any, start: any = {}) { return appShell.applyOpenedProject(opened, start); }

$("target").addEventListener("click", toggleServer);
$("footer-server").addEventListener("click", toggleServer);
$("footer-instance").addEventListener("click", async () => {
  if (!state.project) { toast("Open a project to copy its command"); return; }
  try { await navigator.clipboard.writeText(magicCommandText("ui.feedback")); toast("Copied project feedback command"); }
  catch { toast("Could not copy the project command"); }
});
$("studio-start").addEventListener("click", toggleServer);
window.addEventListener("resize", placeInk);
placeInk();
window.addEventListener('DOMContentLoaded', () => load().catch((e) => {
  toast(String(e));
  acFeedback.errors.push({ message: String(e).slice(0, 300) });
  acFeedback.report('error');
}));
