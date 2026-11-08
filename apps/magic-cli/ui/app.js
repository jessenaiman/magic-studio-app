const { invoke: nativeInvoke, convertFileSrc } = window.__TAURI__.core;
function stampEngineParams(params, action, companion) {
  return { ...params, params: { ...params.params, _origin: action ? "UI click" : "UI callback", _client: "Studio user (unverified label)", _control: action?.control, _request_id: action?.request_id || params.params?._request_id || companion?.request } };
}
function stampOpenParams(params, action) {
  return { ...params, request: params?.request ?? action?.request_id ?? null, control: params?.control ?? action?.control ?? null };
}
function reportTransportError(command, params, action, error) {
  const message = String(error).slice(0, 300);
  acFeedback.last = { transport: command, id: params?.id || null, status: "error", error: message };
  acFeedback.report(null, null, { stage: "handler_error", command: params?.id || command, control: action?.control || null, request_id: action?.request_id || null, error: message });
}
function noteTransportSuccess(command, params, companion, result) {
  acFeedback.last = { transport: command, id: params?.id || null, status: "success" };
  if (companion)
    window.uiRegistry?.event({ ...companion, phase: "success", result });
  if (result?._ui_engine_revision)
    acFeedback.pendingRevision = result._ui_engine_revision;
}
async function buildProjectState(path, opened) {
  if (path && (!opened || opened.project !== path)) {
    const current = await nativeInvoke("execute_command", { id: "project.identity", params: {} }).catch(() => null);
    opened = current?.project === path ? current : await nativeInvoke("execute_command", { id: "project.open", params: { path } });
  }
  const [status, assets, trunk] = await Promise.all([
    nativeInvoke("execute_command", { id: "workshop.status", params: {} }),
    nativeInvoke("execute_command", { id: "project.list_assets", params: {} }),
    nativeInvoke("execute_command", { id: "project.trunk", params: {} }).catch(() => ({ files: [] }))
  ]);
  const projectPath = opened?.project || path || assets?.project || state.project;
  return {
    project: projectPath,
    name: opened?.name || projectPath?.split("/").pop()?.replace(/\.magic$/, "") || "",
    mode: opened?.mode || null,
    status,
    assets,
    trunk,
    storyboard: status?.storyboard || null,
    design: { placements: [], text_fields: [] }
  };
}
async function dispatchToEngine(command, params, _options) {
  switch (command) {
    case "engine_command":
      return nativeInvoke("execute_command", { id: params.id, params: params.params || {} });
    case "project_command":
      await nativeInvoke("execute_command", { id: params.id, params: params.params || {} });
      return buildProjectState(params.project || state.project);
    case "project_open": {
      const opened = await nativeInvoke("execute_command", { id: "project.open", params: { path: params.path } });
      return buildProjectState(opened?.project || params.path, opened);
    }
    case "project_create": {
      const folder = params.root ? `${params.root.replace(/\/+$/, "")}/${params.name}` : params.name;
      const opened = await nativeInvoke("execute_command", { id: "project.new", params: { path: folder, name: params.name } });
      if (params.audio) {
        await nativeInvoke("execute_command", { id: "workshop.set_audio", params: { path: params.audio } });
      }
      return buildProjectState(opened?.project, opened);
    }
    case "animation_create": {
      const folder = params.root ? `${params.root.replace(/\/+$/, "")}/${params.name}` : params.name;
      const opened = await nativeInvoke("execute_command", { id: "project.new", params: { path: folder, name: params.name, mode: "animation" } });
      return buildProjectState(opened?.project, opened);
    }
    case "project_state":
      return buildProjectState(params?.path || state.project);
    case "recent_projects": {
      const cat = await nativeInvoke("execute_command", { id: "project.catalog", params: {} });
      return cat?.projects || [];
    }
    case "recent_forget":
      return nativeInvoke("execute_command", { id: "project.catalog", params: { forget: params?.path } });
    case "projects_root": {
      const res = await nativeInvoke("execute_command", { id: "project.root", params: {} });
      return res?.root || "";
    }
    case "agent_config":
      return nativeInvoke("execute_command", { id: "agent.config", params: {} });
    case "agent_config_save":
      return nativeInvoke("execute_command", { id: "agent.use", params: params?.patch || {} });
    case "open_path":
      return nativeInvoke("execute_command", { id: "workspace.open", params: { path: params?.path } });
    case "startup": {
      const current = await nativeInvoke("execute_command", { id: "project.identity", params: {} }).catch(() => null);
      return { project: current?.project || null, identity: { pid: null, branch: "main" }, menus: { file: [], text: [] } };
    }
    case "close_documents":
      throw new Error("Closing other Craft apps from Magic is not available yet");
    case "new_window":
      throw new Error("Opening another Magic window is not available yet");
    default:
      if (command.includes(".")) {
        return nativeInvoke("execute_command", { id: command, params: params || {} });
      }
      return nativeInvoke(command, params);
  }
}
async function invoke(command, params, options) {
  const action = acFeedback.action;
  const companion = command === "engine_command" ? window.uiRegistry?.capture(params.id, { ...params.params, _request_id: action?.request_id || params.params?._request_id }) : null;
  if (companion)
    window.uiRegistry?.event({ ...companion, phase: "start" });
  acFeedback.pending++;
  await acFeedback.report();
  if (command === "engine_command" && params)
    params = stampEngineParams(params, action, companion);
  if (command === "project_create" || command === "project_open")
    params = stampOpenParams(params, action);
  acFeedback.report(null, null, { stage: "handler_dispatch", control: action?.control || null, request_id: action?.request_id || null, command: params?.id || command });
  try {
    const result = await dispatchToEngine(command, params, options);
    noteTransportSuccess(command, params, companion, result);
    if (command === "engine_command" && result?._ui_engine_revision && params?.id && window.magicUI) {
      window.magicUI.refresh(params.id, state.project, result._ui_engine_revision, result);
    }
    return result;
  } catch (error) {
    if (companion)
      window.uiRegistry?.event({ ...companion, phase: "error", error: String(error) });
    reportTransportError(command, params, action, error);
    throw error;
  } finally {
    acFeedback.pending--;
    await acFeedback.report();
  }
}
const $ = (id) => document.getElementById(id);
const ICONS = {
  song: '<svg viewBox="0 0 24 24"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>',
  lyrics: '<svg viewBox="0 0 24 24"><path d="M4 6h16M4 11h16M4 16h10"/></svg>',
  transcript: '<svg viewBox="0 0 24 24"><path d="M3 12h2M7 8v8M11 5v14M15 9v6M19 11v2"/></svg>',
  animate: '<svg viewBox="0 0 24 24"><path d="M12 3l9 5-9 5-9-5 9-5z"/><path d="M3 13l9 5 9-5"/></svg>',
  chev: '<svg viewBox="0 0 24 24"><path d="M9 6l6 6-6 6"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M8 5l11 7-11 7z"/></svg>',
  pause: '<svg viewBox="0 0 24 24"><path d="M8 5v14M16 5v14"/></svg>'
};
let state = { project: null, data: null, page: 0, open: "song", view: "project", blocksCollapsed: false };
window.state = state;
function updateMagicSelection() {
  const layer = motion?.meta?.layers?.find((part) => part.id === motion.selected);
  const text = layer ? `${layer.label || layer.id} · ${layer.id}` : "No part selected";
  updateAppFooter();
  for (const id of ["magic-target-image"]) {
    const node = $(id);
    if (node) {
      node.textContent = text;
      node.title = layer ? `Selected part ${layer.label || layer.id} (${layer.id}) on ${motion.character}` : "Select a part to target image generation";
    }
  }
}
// projectAssetGrid moved to workspace-elements.js (central, shared). Kept as wrapper for existing callers.
function projectAssetGridDelegated(assets, opts) { return projectAssetGrid(assets, opts); }
const FOOTER_STAGES = [["song", "Song"], ["lyrics", "Lyrics"], ["transcript", "Transcript"], ["storyboard", "Storyboard"], ["animate", "Animate"], ["studio", "Studio"]];
const STAGE_DOT = { ok: "ready", warn: "needs-attention", idle: "idle" };
const STAGE_NOTE = { ok: "ready", warn: "needs work", idle: "not started" };

class AppFooterController {
  queued = false;
  stageState(id) {
    if (id === "studio")
      return state.data?.status?.film_timeline?.exists ? "ok" : "warn";
    const seen = projectView.prerequisite(id).state;
    return seen === "ready" ? "ok" : seen ? "warn" : "idle";
  }
  async go(id) {
    if (id === "studio")
      return setView("studio");
    if (id === "storyboard")
      return setView("storyboard");
    if (state.view !== "project" && await setView("project") === false)
      return;
    state.open = SONG_SECTIONS.includes(id) ? "song" : id;
    state.blocksCollapsed = false;
    $("blocks").classList.remove("collapsed");
    renderBlocks();
  }
  animationText() {
    const meta = motion.meta;
    if (motion.mode === "composite")
      return "Composite · images arranged in time";
    if (!meta)
      return motion.character ? `${motion.character} · loading…` : "No object chosen";
    const clips = (meta.tracks || []).filter((t) => !t.standard).length;
    const track = motion.rest ? "Rest pose" : meta.tracks.find((t) => t.id === motion.track)?.label || motion.track.replaceAll("-", " ");
    const part = meta.layers.find((l) => l.id === motion.selected);
    return `${motion.character} · ${meta.layers.length} layers / ${clips} clips · ${track} · Selected: ${part ? part.label || part.id : "none"}`;
  }
  storyboardText() {
    const pages = state.data?.storyboard?.previews?.length || 0;
    const objects = (state.data?.assets?.assets || []).length;
    return [
      pages ? `Storyboard · Page ${state.page + 1} / ${pages}` : "Storyboard · not built yet",
      pages ? state.pageTitles?.[state.page] : null,
      `${objects} object${objects === 1 ? "" : "s"}`
    ].filter(Boolean).join(" · ");
  }
  studioText() {
    const timeline = state.data?.status?.film_timeline;
    return timeline?.exists ? timeline.path : "No final FilmCraft timeline linked";
  }
  contextText() {
    if (state.view === "home")
      return "Choose a project or start a new song";
    if (state.view === "animation")
      return this.animationText();
    if (state.view === "studio")
      return this.studioText();
    if (state.view === "analytics")
      return "Mayhem · code, usage and tool reports";
    return this.storyboardText();
  }
  scheduleUpdate() {
    if (this.queued)
      return;
    this.queued = true;
    queueMicrotask(() => {
      this.queued = false;
      this.render();
    });
  }
  renderStages() {
    $("footer-stages").replaceChildren(...FOOTER_STAGES.map(([id, label]) => {
      const tone = this.stageState(id);
      return el("button", {
        class: "footer-stage",
        id: `footer-stage-${id}`,
        title: `${label} · ${STAGE_NOTE[tone]}`,
        onclick: () => this.go(id).catch((e) => toast(String(e)))
      }, window.companionBoxes.dot(STAGE_DOT[tone], "", `footer-stage-${id}`), label);
    }));
  }
  saveStatus(unsaved) {
    if (projectView.pending > 0)
      return { tone: "working", label: "Saving…" };
    if (unsaved)
      return { tone: "needs-attention", label: "Unsaved edits" };
    return state.project ? { tone: "ready", label: "Project saved" } : { tone: "idle", label: "Not saved" };
  }
  isMotionUnsaved() {
    return state.view === "animation" && typeof magicTheatre !== "undefined" && (magicTheatre.interactionRevision > magicTheatre.savedRevision || Boolean(magicTheatre.gestureActive));
  }
  saveStateTitle(motionUnsaved, unsaved) {
    if (motionUnsaved)
      return "Theatre motion has unsaved keyframes · use Save motion";
    if (unsaved)
      return "Storyboard changes need saving";
    return "Magic Studio project state; HyperFrames saves composition edits in its own editor";
  }
  renderSaveState() {
    const motionUnsaved = this.isMotionUnsaved();
    const unsaved = motionUnsaved || state.data?.design?.dirty === true;
    const { tone, label } = this.saveStatus(unsaved);
    $("footer-save-state").replaceChildren(window.companionBoxes.dot(tone, "", "footer-save-state"), label);
    $("footer-save-state").title = this.saveStateTitle(motionUnsaved, unsaved);
  }
  render() {
    if (!$("app-footer"))
      return;
    this.renderStages();
    const context = this.contextText();
    $("footer-context").textContent = context;
    $("footer-context").title = context;
    this.renderSaveState();
    appShell.renderFilmTarget();
  }
}
const appFooter = new AppFooterController;
function footerStageState(id) {
  return appFooter.stageState(id);
}
async function footerGo(id) {
  return appFooter.go(id);
}
function animationFooter() {
  return appFooter.animationText();
}
function storyboardFooter() {
  return appFooter.storyboardText();
}
function studioFooter() {
  return appFooter.studioText();
}
function footerContext() {
  return appFooter.contextText();
}
function updateAppFooter() {
  appFooter.scheduleUpdate();
}
function renderFooterStages() {
  appFooter.renderStages();
}
function saveStatus(unsaved) {
  return appFooter.saveStatus(unsaved);
}
function renderFooterSaveState() {
  appFooter.renderSaveState();
}
function renderAppFooter() {
  appFooter.render();
}
window.addEventListener("message", (event) => {
  if (event.data?.magicTheatre && event.data.type !== "values")
    setTimeout(updateAppFooter);
});
let toastTimer;
function toast(text) {
  const t = $("toast");
  t.textContent = text;
  t.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("show"), 2400);
}
const uiConfig = { version: 0, looks: {}, densities: [], elements: {} };
window.uiConfig = uiConfig;
async function loadComponents() {
  const response = await fetch("components.json");
  if (!response.ok)
    throw new Error(`components.json ${response.status}`);
  Object.assign(uiConfig, await response.json());
  for (const [id, entry] of Object.entries(uiConfig.elements)) {
    const node = document.getElementById(id);
    if (node) {
      applyEntry(node, entry, id);
      node.setAttribute("data-box-id", id);
      window.companionBoxes?.register(id, { project: state.project, ...entry });
    }
  }
  for (const node of document.body.querySelectorAll("*"))
    window.uiRegistry?.registerElement(node);
  return uiConfig;
}
function applyEntry(node, entry, id = node.id) {
  if (!entry)
    return node;
  node.dataset.uiConfig = id;
  if (entry.component)
    node.className = [...new Set(`${node.className || ""} ${entry.component}`.trim().split(/\s+/))].join(" ");
  if (entry.look)
    node.dataset.look = entry.look;
  if (entry.density)
    node.dataset.density = entry.density;
  if (entry.scroll)
    node.dataset.scroll = entry.scroll;
  if (entry.component === "panel" && entry.rail && !node.querySelector("[data-companion-slot]")) {
    const head = node.querySelector(".panel-head");
    if (!head) {
      node.dataset.contractError = "Panel has no shared header";
      return node;
    }
    delete node.dataset.contractError;
    node.setAttribute("data-companion-rail", entry.rail);
    const slot = document.createElement("span");
    slot.className = "panel-rail";
    slot.setAttribute("data-companion-slot", entry.rail);
    slot.setAttribute("aria-hidden", "true");
    head.append(slot);
    window.uiRegistry?.registerElement(slot);
  }
  return node;
}
function runListener(listener, event) {
  const previous = acFeedback.action;
  acFeedback.action = event.acAction;
  try {
    return listener(event);
  } finally {
    acFeedback.action = previous;
  }
}
function magicCommandText(id, params = {}, project = state.project) {
  const quote = (value) => `'${String(value).replaceAll("'", "'\\''")}'`;
  return `magic-cli${project ? ` --project ${quote(project)}` : ""} exec ${id}${Object.entries(params).map(([key, value]) => ` ${quote(`${key}=${typeof value === "object" ? JSON.stringify(value) : value}`)}`).join("")}`;
}
const kindOf = (name) => (name.split(".").pop() || "file").toUpperCase().slice(0, 4);
function fileButton(chip) {
  return el("button", { class: "file", title: chip.path, "aria-label": `Open ${chip.name}`, onclick: () => openPath(chip.path) }, el("span", { class: "kind" }, kindOf(chip.name)), el("span", { class: "name" }, chip.name));
}
async function openPath(path) {
  try {
    await invoke("open_path", { path });
    toast(`Opened ${path.split("/").pop()}`);
  } catch (e) {
    toast(String(e));
  }
}
const SONG_SECTIONS = ["lyrics", "transcript"];
const WORKSPACE_OPENERS = {
  storyboard: () => renderStoryboard(),
  animation: () => animationOpen(),
  design: () => wordsOpen(),
  skills: () => {
    if (typeof renderResources === "function")
      renderResources();
  },
  analytics: () => {
    if (typeof renderAnalytics === "function")
      renderAnalytics();
  },
  studio: () => filmView.open()
};

class AppShellController {
  placeInk() {
    window.workspaceNavigation?.select(state.view);
  }
  async openWorkspace(view) {
    await WORKSPACE_OPENERS[view]?.();
  }
  canLeaveCurrentView(nextView) {
    if (state.view === "animation" && nextView !== "animation" && typeof theatreCanLeave === "function") {
      return Boolean(theatreCanLeave());
    }
    return true;
  }
  cleanupPreviousView(nextView) {
    if (typeof animationPause === "function")
      animationPause();
    if (nextView !== "animation")
      animationReview.dispose();
    if (nextView !== "animation" && typeof animationJobsDispose === "function")
      animationJobsDispose();
    if (nextView !== "animation" && typeof tripo3DStage !== "undefined")
      tripo3DStage.dispose();
  }
  async setView(view, action) {
    const errors = acFeedback.errors.length;
    if (view === "split")
      view = "studio";
    if (!this.canLeaveCurrentView(view))
      return false;
    this.cleanupPreviousView(view);
    state.view = view;
    window.workspaceNavigation?.select(view);
    const page = view;
    document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${page}`));
    this.placeInk();
    await this.openWorkspace(view);
    updateAppFooter();
    window.magicCompanions?.sync();
    if (action && feedbackSnapshot().rendered && acFeedback.errors.length === errors) {
      await acFeedback.report(null, null, { stage: "ui_navigation_applied", ...action, view: state.view });
    }
  }
  renderFilmTarget() {
    const timeline = state.data?.status?.film_timeline;
    const label = timeline?.exists ? "FilmCraft timeline" : "FilmCraft timeline missing";
    for (const id of ["target", "footer-server"]) {
      const button = $(id);
      button.disabled = !state.project;
      button.title = timeline?.path || "Open the Film workspace";
      button.replaceChildren(window.companionBoxes.dot(timeline?.exists ? "ready" : "idle", "", "target"), label);
    }
  }
  renderInstanceIdentity(identity = {}) {
    $("footer-instance").textContent = `Studio ${identity.pid ?? "unknown"} · ${identity.branch || "unknown build"}`;
    $("footer-instance").title = `${identity.build_id || "Unknown build"} · PID ${identity.pid ?? "?"} · ${identity.executable || ""} · Click to copy the CLI command`;
  }
  async load() {
    await loadComponents();
    const start = await invoke("startup");
    state.identity = start.identity;
    window.applicationFileMenu?.configure(start.menus.file);
    window.textContextMenu?.configure(start.menus.text);
    if (start.character && typeof motion !== "undefined") {
      motion.character = start.character;
      motion.explicitCharacter = true;
    }
    this.renderInstanceIdentity(start.identity);
    state.launchAnimationOnly = start.animation_only === true;
    if (start.new_project)
      return beginNewProject();
    if (!start.project)
      return showHome();
    await this.openProject(start.project, start);
  }
  async openProject(path, start = {}) {
    try {
      return await this.applyOpenedProject(await invoke("project_open", { path }), start);
    } catch (e) {
      toast(String(e));
      throw e;
    }
  }
  resetProjectTransientState(path) {
    if (state.project === path)
      return;
    animationReset();
    songStop();
    transcriptReaderClose();
    projectView.error = null;
  }
  updateWindowProjectTitle(path, name) {
    const title = name || path.split("/").pop()?.replace(/\.magic$/, "") || "";
    $("project-name").textContent = title;
    $("project-name").parentElement.title = title;
    document.title = `${title} · Magic Studio`;
  }
  async applyOpenedProject(opened, start = {}) {
    if (opened.reused_session) {
      toast(opened.opened_new_instance ? "Opened project in a separate window" : "Focused the existing project session");
      return;
    }
    const path = opened.project;
    if (!path)
      throw new Error("Project command did not return its canonical location");
    state.data = opened;
    state.projectReceipt = opened;
    const animateOnly = opened.mode === "animation" || state.launchAnimationOnly === true;
    state.animateOnly = animateOnly;
    document.body.classList.toggle("animation-mode", animateOnly);
    this.resetProjectTransientState(path);
    state.project = path;
    state.lyrics = null;
    loadSongLyrics();
    window.magicCompanions?.sync();
    state.page = 0;
    const startOpen = start.open && start.open !== "agent" ? start.open : "song";
    state.open = SONG_SECTIONS.includes(startOpen) ? "song" : startOpen;
    document.body.classList.remove("at-home");
    this.updateWindowProjectTitle(path, opened.name);
    this.renderFilmTarget();
    renderBlocks();
    renderStoryboard();
    await this.setView(animateOnly ? "animation" : start.view || "project");
    window.workshopDrawers?.close(false);
    await acFeedback.acknowledge(state.data?._ui_engine_revision, path);
  }
}
const appShell = new AppShellController;
function placeInk() {
  appShell.placeInk();
}
async function openWorkspace(view) {
  return appShell.openWorkspace(view);
}
async function setView(view, action) {
  return appShell.setView(view, action);
}
function serverUrl() {
  return wordsPanel.studio?.preview_url || null;
}
function renderTarget() {
  appShell.renderFilmTarget();
}
async function load() {
  return appShell.load();
}
async function openProject(path, start = {}) {
  return appShell.openProject(path, start);
}
async function applyOpenedProject(opened, start = {}) {
  return appShell.applyOpenedProject(opened, start);
}
$("target")?.addEventListener("click", () => setView("studio"));
$("footer-server")?.addEventListener("click", () => setView("studio"));
$("footer-instance")?.addEventListener("click", async () => {
  if (!state.project) {
    toast("Open a project to copy its command");
    return;
  }
  try {
    await window.__TAURI__.clipboardManager.writeText(magicCommandText("ui.feedback"));
    toast("Copied project feedback command");
  } catch {
    toast("Could not copy the project command");
  }
});
window.addEventListener("resize", placeInk);
placeInk();
window.addEventListener("DOMContentLoaded", () => {
  if (!document.querySelector(".app-bar")) return;
  load().catch((e) => {
    toast(String(e));
    acFeedback.errors.push({ message: String(e).slice(0, 300) });
    acFeedback.report("error");
  });
});
