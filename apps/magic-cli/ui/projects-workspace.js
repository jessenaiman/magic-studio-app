renderWorkspaceNav("project");
const { invoke: projectInvoke, convertFileSrc } = window.__TAURI__.core;
const FILM = '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4"/></svg>';
const CROSS = '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>';
const projectField = (id) => document.getElementById(id);
const projectCommand = (id, params = {}) => projectInvoke("execute_command", { id, params });
const projectForm = projectField("new-form");
let projectMode = "project";

async function projectAction(action) {
  try {
    projectField("home-status").textContent = "";
    await action();
  } catch (error) {
    projectField("home-status").textContent = String(error.message || error);
  }
}

async function pickProjectPath(kind) {
  const selected = await projectCommand("ui.projects.pick", { kind });
  return selected?.path || null;
}

function openRecentProject(item, action = {}) {
  const view = action.open_block === "lyrics" ? "lyrics" : action.view;
  return projectCommand("ui.projects.open", { path: item.path, view });
}

async function showRecentProjects() {
  const catalog = await projectCommand("project.catalog");
  const cards = (catalog.projects || []).map(item => projectCard(item, {
    open: openRecentProject,
    locate: async () => {
      const path = await pickProjectPath("project");
      if (path) await openRecentProject({ path });
    },
    forget: async (project) => {
      await projectCommand("project.catalog", { forget: project.path });
      await showRecentProjects();
    },
  }));
  projectField("recent").replaceChildren(...(cards.length ? cards : [
    el("div", { class: "panel-empty boxed" }, "No projects yet. Start with New song."),
  ]));
}

function showProjectLyrics(lyrics, transcript) {
  const sections = lyrics.sections || [];
  const lyricsText = sections.map((section, index) => el("details", { open: index === 0 },
    el("summary", {}, section.label || section.title || section.name || section.section || "Lyrics"),
    el("p", { class: "project-lyric-lines" }, (section.lines || []).map(line => line.text).join("\n"))));
  const paragraphs = transcript?.paragraphs?.map(paragraph => paragraph.text).filter(Boolean) || [];
  const fallback = paragraphs.length ? paragraphs.join("\n\n") : transcript?.words?.map(word => word.text || word.word).join(" ");
  return el("section", {}, el("h3", {}, "Lyrics"), ...(lyricsText.length ? lyricsText : [
    el("p", { class: "muted" }, fallback ? "From the transcript" : "No lyrics in this project yet."),
    fallback ? el("p", { class: "project-lyric-lines" }, fallback) : null,
  ]));
}

function showProjectFiles(files, projectDir) {
  const groups = { audio: "Audio", liner_notes: "Lyrics & notes", transcript: "Timing", storyboard: "Storyboard", assets: "Assets", film: "Studio", other: "Other files" };
  return el("section", {}, el("h3", {}, "Project files"), ...Object.entries(groups).map(([group, label]) => {
    const items = files.filter(file => file.group === group);
    if (!items.length) return null;
    return el("details", { open: ["audio", "film"].includes(group) }, el("summary", {}, `${label} · ${items.length}`),
      el("div", { class: "project-file-list" }, items.map(file => el("button", {
        type: "button", class: "ghost", title: file.path, disabled: file.exists === false,
        onclick: () => projectAction(() => projectCommand("workspace.open", { path: file.path })),
      }, file.path.startsWith(`${projectDir}/`) ? file.path.slice(projectDir.length + 1) : file.name))));
  }), files.length ? null : el("p", { class: "muted" }, "No project files yet."));
}

async function showCurrentProject() {
  const { project, assets = [] } = await projectCommand("project.list_assets");
  if (!project) return;
  const [workshop, lyrics, film] = await Promise.all([
    projectCommand("workshop.files"),
    projectCommand("song.lyrics"),
    projectCommand("video.film").catch(() => null),
  ]);
  const transcript = lyrics.source === "transcript"
    ? (await projectCommand("workshop.status")).stages?.find(stage => stage.id === "transcript")?.document
    : null;
  const files = new Map(workshop.files.map(file => [file.path, file]));
  for (const asset of assets) {
    if (asset.path && !files.has(asset.path)) files.set(asset.path, { ...asset, group: "assets" });
  }
  for (const file of [film?.timeline, film?.preview, film?.export]) {
    if (file?.exists) files.set(file.path, { ...file, name: file.path.split("/").at(-1), group: "film" });
  }
  const overview = projectField("project-overview");
  overview.replaceChildren(el("header", {},
    el("h2", {}, project.split("/").at(-1).replace(/\.magic$/i, "")),
    el("button", { type: "button", class: "ghost", onclick: () => projectAction(() => projectCommand("ui.set_view", { view: "lyrics" })) }, "Edit lyrics")),
    el("div", { class: "project-overview-grid" }, showProjectLyrics(lyrics, transcript),
      showProjectFiles([...files.values()], workshop.project_dir)));
  overview.hidden = false;
}

async function showProjectForm(mode) {
  projectMode = mode;
  projectForm.hidden = false;
  projectField("new-media-label").textContent = mode === "animation" ? "Character art" : "Audio";
  projectField("new-media").placeholder = mode === "animation" ? "Optional character image…" : "Audio file…";
  projectField("new-media").value = "";
  projectField("new-media").required = mode === "project";
  if (!projectField("new-root").value) {
    const settings = await projectCommand("project.root");
    projectField("new-root").value = settings.root || "";
  }
  projectField("new-name").focus();
}

async function createProject() {
  const name = projectField("new-name").value.trim();
  if (!name || name === "." || name === ".." || /[/\\]/.test(name))
    throw Error("Enter a project name without a path.");
  const root = projectField("new-root").value.trim();
  const media = projectField("new-media").value.trim();
  if (!root) throw Error("Choose a project folder first.");
  const button = projectField("new-create");
  button.disabled = true;
  try {
    await projectCommand("project.new", {
      path: `${root.replace(/\/+$/, "")}/${name}/${name}.magic`,
      mode: projectMode,
      ...(projectMode === "project" && media ? { audio: media, process_audio: false } : {}),
    });
    if (projectMode === "animation") {
      if (media) await projectCommand("project.add_asset", { path: media, character: name, name });
      await projectCommand("ui.set_view", { view: "animation" });
    }
    projectForm.reset();
    projectForm.hidden = true;
  } finally {
    button.disabled = false;
  }
}

projectField("home-new").onclick = () => projectAction(() => showProjectForm("project"));
projectField("home-new-animation").onclick = () => projectAction(() => showProjectForm("animation"));
projectField("home-open").onclick = () => projectAction(async () => {
  const path = await pickProjectPath("project");
  if (path) await openRecentProject({ path });
});
projectField("new-media-pick").onclick = () => projectAction(async () => {
  const media = await pickProjectPath(projectMode === "animation" ? "image" : "audio");
  if (media) projectField("new-media").value = media;
});
projectField("new-root-pick").onclick = () => projectAction(async () => {
  const root = await pickProjectPath("folder");
  if (root) projectField("new-root").value = root;
});
projectField("new-cancel").onclick = () => { projectForm.hidden = true; };
projectForm.onsubmit = (event) => {
  event.preventDefault();
  projectAction(createProject);
};
document.addEventListener("keydown", (event) => {
  if (!(event.ctrlKey || event.metaKey) || event.target.closest("input, textarea")) return;
  const button = { n: "home-new", o: "home-open" }[event.key];
  if (button) {
    event.preventDefault();
    projectField(button).click();
  }
});
projectAction(() => Promise.all([showRecentProjects(), showCurrentProject()]));
