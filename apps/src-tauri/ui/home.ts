// Home: shown when no project is open (and from the brand mark). New song, Open, and the recent
// projects as cards. Opening and creating run the engine's `project.open` / `project.new`.
const dialog = window.__TAURI__.dialog;

const FILM = '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4"/></svg>';
const CROSS = '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>';

class HomeViewController {
  newProjectForm = $("new-form");

  constructor() {
    this.bindEvents();
  }

  async show({closeProject = true} = {}) {
    if (typeof theatreCanLeave === "function" && !theatreCanLeave()) return;
    if(closeProject && state.project){try{await invoke("engine_command",{id:"project.close",params:{}});}catch(e){toast(String(e));throw e;}}
    state.creating = false;
    this.newProjectForm.hidden = true;
    animationReset();
    window.workshopDrawers?.close(false);
    Object.assign(agent, { loaded: false, session: null, messages: [], permissions: [], forms: [], attachments: [] });
    state.project = null;
    state.view = 'home';
    window.magicCompanions?.sync();
    state.data = null;
    document.body.classList.add("at-home");
    document.body.classList.remove("split");
    $("project-name").textContent = "No project";
    document.title = "Magic Studio";
    document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === "view-home"));
    await invoke("studio_hide").catch(() => {});
    await this.renderRecent();
    updateAppFooter();
    requestAnimationFrame(()=>acFeedback.report('ready'));
  }

  async renderRecent() {
    const list = await invoke("recent_projects");
    $("recent-label").hidden = !list.length;
    if (!list.length) {
      $("recent").replaceChildren(el("div", { class: "panel-empty boxed" }, "No projects yet. Start with New song."));
      return;
    }
    $("recent").replaceChildren(...list.map(project => projectCard(project, {
      open: (item, action = {}) => openProject(item.path, {view: action.view, open: action.open_block}),
      locate: async item => {
        const path = await dialog.open({title: `Locate ${item.name}`, filters: [{name: 'Magic Studio project', extensions: ['magic']}]});
        if (!path) return;
        await openProject(path);
        if (path !== item.path) await invoke('recent_forget', {path: item.path});
      },
      forget: async item => { await invoke('recent_forget', {path: item.path}); await this.renderRecent(); }
    })));
  }

  async openDialog() {
    const path = await dialog.open({ title: "Open project", filters: [{ name: "Magic Studio project", extensions: ["magic"] }] });
    if (path) await openProject(path);
  }

  async beginNewProject() {
    if (state.project) { await invoke("new_window"); return; }
    await this.show();
    Object.assign(state, {creating:true, open:"song", view:"project", blocksCollapsed:false});
    document.body.classList.remove("at-home");
    document.querySelectorAll(".view").forEach(v => v.classList.toggle("active", v.id === "view-project"));
    $("blocks").classList.remove("collapsed");
    $("project-name").textContent = "New project";
    $("sb-chips").replaceChildren();
    $("sb-page").textContent = "—";
    $("sb-cards").replaceChildren(projectSetupCard(true));
    renderBlocks();
    try { const productionRoot = await invoke("projects_root"); $("new-root").value = productionRoot; $("new-root").defaultValue = productionRoot; }
    catch (error) { $("new-root").value = ""; toast(String(error)); }
    $("new-name").focus();
  }

  renderNewProjectBlocks(box) {
    this.newProjectForm.hidden = false;
    box.append(block("song", ICONS.song, "Song", "New project", false, [this.newProjectForm], "missing"));
    for (const [id, title] of [["storyboard", "Storyboard"], ["assets", "Assets"], ["animate", "Animate"]]) {
      box.append(block(id, ICONS.animate, title, "", false,
        [el("span", {class:"why"}, "After recording")], "missing"));
    }
  }

  // New animation: pick the approved character image; the study is named after it,
  // created in the production folder, and opens straight into Animation.
  async beginNewAnimation(action) {
    const image = await dialog.open({ title: "Approved character image", filters: [{ name: "Image", extensions: ["png", "webp", "jpg", "jpeg", "psd"] }] });
    if (!image) return;
    const name = image.split("/").pop().replace(/\.[^.]+$/, "").replace(/[^A-Za-z0-9]+/g, " ").trim() || "Character";
    const opened = await invoke("animation_create", {request:action?.request_id, control:action?.control, name, root: await invoke("projects_root")});
    await applyOpenedProject(opened);
    await designCommand("project.add_asset", {path: image, character: name.toLowerCase().replace(/ /g, "-"), name});
    toast(`Animation created · ${name}`);
  }

  async pickNewAudio() {
    const file = await dialog.open({ title: "Song audio", filters: [{ name: "Audio", extensions: ["wav", "mp3", "flac", "m4a", "aac", "ogg"] }] });
    if (!file) return;
    $("new-audio").value = file;
    if (!$("new-name").value.trim()) {
      $("new-name").value = file.split("/").pop().replace(/\.[^.]+$/, "").replace(/[._-]+/g, " ").replace(/\bcdda\b/i, "").trim();
    }
  }

  async pickNewRoot() {
    const dir = await dialog.open({ title: "Folder for this project", directory: true, defaultPath: $("new-root").value || undefined });
    if (dir) $("new-root").value = dir;
  }

  async submitNewProject(e) {
    e.preventDefault();
    if (!$("new-audio").value) return toast("Choose the song's audio first");
    $("new-create").disabled = true;
    try {
      const action = acFeedback.actions.get(e.submitter) as any;
      const opened = await invoke("project_create", {request:action?.request_id,control:action?.control, name: $("new-name").value, audio: $("new-audio").value, root: $("new-root").value });
      state.creating = false;
      this.newProjectForm.hidden = true;
      this.newProjectForm.reset();
      await applyOpenedProject(opened);
      toast(opened.problems?.length ? `Project created · ${opened.problems.join('; ')}` : "Project created");
    } catch (err) { toast(String(err)); }
    this.newProjectForm.querySelector("#new-create").disabled = false;
  }

  bindEvents() {
    $("new-audio-pick").addEventListener("click", () => this.pickNewAudio());
    $("new-root-pick").addEventListener("click", () => this.pickNewRoot());
    $("new-cancel").addEventListener("click", () => this.show().catch(error => toast(String(error))));
    $("new-form").addEventListener("submit", e => this.submitNewProject(e));
    $("home-new").addEventListener("click", () => this.beginNewProject().catch(error => toast(String(error))));
    $("home-new-animation").addEventListener("click", event => this.beginNewAnimation(acFeedback.actions.get(event.currentTarget)).catch(error => toast(String(error))));
    $("home-open").addEventListener("click", () => this.openDialog());
    $("brand").addEventListener("click", () => this.show().catch(error => toast(String(error))));
    $("brand").addEventListener("keydown", (e) => { if (e.key === "Enter") this.show(); });
    document.addEventListener("keydown", (e) => {
      if (!(e.ctrlKey || e.metaKey) || e.target.closest("input, textarea")) return;
      if (e.key === "n") { e.preventDefault(); this.beginNewProject().catch(error => toast(String(error))); }
      if (e.key === "o") { e.preventDefault(); this.openDialog(); }
    });
  }
}

const homeViewController = new HomeViewController();
const newProjectForm = homeViewController.newProjectForm;

async function showHome(opts = {}) { return homeViewController.show(opts); }
async function renderRecent() { return homeViewController.renderRecent(); }
async function openDialog() { return homeViewController.openDialog(); }
async function beginNewProject() { return homeViewController.beginNewProject(); }
function renderNewProjectBlocks(box) { return homeViewController.renderNewProjectBlocks(box); }
async function beginNewAnimation(action) { return homeViewController.beginNewAnimation(action); }
