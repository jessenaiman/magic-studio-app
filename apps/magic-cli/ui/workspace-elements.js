
const WORKSPACE_NAV = [
  ["project", "Project", "projects.html"],
  ["storyboard", "Storyboard", "storyboard-workspace.html"],
  ["design", "Lyrics", "lyrics.html"],
  ["animation", "Animation", "animation-controls.html"],
  ["motion", "Motion", "motion.html"],
  ["studio", "Studio", "studio.html"],
];
function renderWorkspaceNav(current, projectName) {
  const header = el("header", { id: "app-bar", class: "bar" },
    el("div", { class: "brand" }, el("button", { id: "brand", class: "ghost", onclick: () => location.assign("projects.html") }, "✦ Magic Studio")),
    el("span", { id: "project-name", class: "muted" }, projectName || "No project"),
    el("nav", { id: "mode-tabs", "aria-label": "Workspace views" },
      ...WORKSPACE_NAV.map(([id, label, href]) => el("button", {
        id: `tab-${id}`, class: current === id ? "start" : "ghost",
        "aria-current": current === id ? "page" : null,
        onclick: () => { if (current !== id) location.assign(href); }
      }, label))));
  document.body.prepend(header);
  return header;
}
function isOverlayMedia(asset, project) {
  const projectPath = project?.replaceAll("\\", "/");
  const path = asset.path?.replaceAll("\\", "/");
  const folder = projectPath?.slice(0, projectPath.lastIndexOf("/"));
  const relative = folder && path?.startsWith(`${folder}/`) ? path.slice(folder.length + 1) : "";
  return /^(motion|lettering)\//i.test(relative) ||
    (/\.(ecproj|mp4|mov|webm)$/i.test(path || "") && /^(motion_|kinetic_lyrics_)/i.test(asset.id || ""));
}

function applyAttribute(node, key, value) {
  if (key === "class")
    node.className = value;
  else if (key === "html")
    node.innerHTML = value;
  else if (key.startsWith("on"))
    node.addEventListener(key.slice(2), (event) =>
      typeof runListener === "function" ? runListener(value, event) : value(event));
  else if (typeof value === "boolean" && key in node)
    node[key] = value;
  else
    node.setAttribute(key, value);
}

function registerElement(node, boxId) {
  const entry = boxId && typeof uiConfig !== "undefined" ? uiConfig.elements[boxId] : null;
  if (entry)
    applyEntry(node, entry, boxId);
  if (boxId) {
    node.setAttribute("data-box-id", boxId);
    const project = typeof state === "undefined" ? null : state.project;
    window.companionBoxes?.register(boxId, { project, label: entry?.label || null, look: entry?.look || null, density: entry?.density || null });
  }
  window.uiRegistry?.registerElement(node);
}

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  let box = null;
  for (const [key, value] of Object.entries(attrs)) {
    if (value === null || value === undefined)
      continue;
    if (key === "box")
      box = value;
    else
      applyAttribute(node, key, value);
  }
  for (const child of children.flat())
    if (child != null)
      node.append(child);
  registerElement(node, box || attrs.id || null);
  return node;
}

function isBackgroundPlateAsset(asset) {
  const path = String(asset?.path || "").replaceAll("\\", "/").toLowerCase();
  const name = String(asset?.name || asset?.id || "").toLowerCase();
  return path.includes("/backgrounds/") || path.includes("/plates/") || /\b(background|backdrop|plate|world|clearing|river)\b/.test(name);
}

function magicAssetTile(asset, { active = false, candidate = false, prefix = "magic-asset", variant = "compact", subtitle = null, onclick, onremove = null, canDrag = true, placeholder = null }) {
  const name = asset.name || asset.id;
  const fallbackBadge = placeholder || (asset.kind === "model_3d" || /\.(glb|gltf)$/i.test(asset.path || "") ? "3D RIG" : /\.(mp4|mov|webm)$/i.test(asset.path || "") ? "VIDEO" : "PART");
  const image = /\.(png|jpe?g|webp|gif|svg)$/i.test(asset.path || "") ? el("img", { src: window.__TAURI__.core.convertFileSrc(asset.path), alt: "", draggable: "false" }) : el("span", { class: "magic-asset-placeholder" }, fallbackBadge);
  const remove = onremove ? el("span", {
    class: "forget asset-forget",
    id: `${prefix}-remove-${asset.id}`,
    role: "button",
    tabindex: "0",
    "data-commands": "project.remove_asset",
    "aria-label": `Remove ${name} (${asset.id})`,
    title: `Remove ${name} from project`,
    html: '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>',
    onclick: (e) => {
      e.stopPropagation();
      onremove();
    },
    onkeydown: (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        e.stopPropagation();
        onremove();
      }
    }
  }) : null;
  const subText = subtitle || asset.id;
  const tile = el("button", {
    class: `magic-asset-tile${variant === "card" ? " magic-asset-card" : ""}${active ? " active" : ""}${candidate ? " candidate" : ""}`,
    id: `${prefix}-${asset.id}`,
    draggable: String(canDrag),
    "aria-pressed": String(active),
    title: candidate ? `${name} · generated candidate awaiting review` : `${name} (${asset.id})${canDrag ? " · drag onto a storyboard page or click to animate" : ""}`,
    onclick
  }, variant === "card" ? el("span", { class: "magic-asset-card-thumb" }, image) : image, el("span", { class: "magic-asset-name" }, name), candidate ? el("span", { class: "magic-asset-badge" }, "Candidate") : el("span", { class: "magic-asset-id" }, subText), remove);
  if (canDrag) tile.addEventListener("dragstart", (event) => {
    event.dataTransfer.setData("application/x-magicstudio-asset", asset.id);
    event.dataTransfer.setData("text/plain", asset.id);
    event.dataTransfer.effectAllowed = "copy";
  });
  return tile;
}

// Central assets component — shared between Projects, Storyboard, Animation, Motion, Studio.
// Single source: project.list_assets. All views render via this grid + magicAssetTile.
function projectAssetGrid(assets, { active = null, prefix = "magic-asset", variant = "compact", subtitle = null, onOpen, onRemove } = {}) {
  return el("div", { class: `magic-assets-grid${variant === "card" ? " magic-assets-cards" : ""}` }, (assets || []).map((asset) => magicAssetTile(asset, {
    active: asset.id === active, prefix, variant,
    subtitle: typeof subtitle === "function" ? subtitle(asset) : subtitle,
    onclick: () => onOpen ? onOpen(asset.id) : (typeof openAnimation === "function" ? openAnimation(asset.id) : null),
    onremove: onRemove ? () => onRemove(asset.id) : null,
  })));
}

function stageAssetsPanel(assets, { project = null, tab = "characters", onTabChange, onAddAsset, onOpen, onRemove, prefix = "stage-asset" } = {}) {
  const all = (assets || []).filter((asset) => !isOverlayMedia(asset, project));
  const plates = all.filter(isBackgroundPlateAsset);
  const characters = all.filter((asset) => !isBackgroundPlateAsset(asset));
  const activeTab = tab === "plates" ? "plates" : "characters";
  const visible = activeTab === "plates" ? plates : characters;
  const grid = visible.length
    ? projectAssetGrid(visible, {
        prefix,
        variant: "card",
        subtitle: (asset) => isBackgroundPlateAsset(asset)
          ? "Scene Plate · Storyboard →"
          : (asset.kind === "model_3d" || /\.(glb|gltf)$/i.test(asset.path || "") ? "3D Rig · Animate →" : "Cutout · Animate →"),
        onOpen,
        onRemove,
      })
    : el("div", { class: "pf-empty" }, activeTab === "plates" ? "No background plates registered yet. Click + Add Asset to import scene art." : "No character cutouts registered yet. Click + Add Asset to import a cutout or 3D rig.");
  return el("section", { class: "pf-panel stage-assets-panel", id: "stage-assets-panel", "aria-label": "Stage Assets & Animate" },
    el("div", { class: "stage-assets-head" },
      el("div", { class: "stage-assets-title-wrap" },
        el("strong", { class: "pf-title" }, "Stage Assets & Animate"),
        el("span", { class: "pf-count" }, `(${all.length})`)
      ),
      el("button", {
        id: "stage-add-asset",
        class: "pf-btn",
        onclick: (event) => onAddAsset && onAddAsset(event)
      }, "+ Add Asset")
    ),
    el("div", { class: "stage-assets-tabs", role: "tablist", "aria-label": "Asset categories" },
      el("button", {
        id: "stage-tab-characters",
        role: "tab",
        class: `stage-tab${activeTab === "characters" ? " active" : ""}`,
        "aria-selected": String(activeTab === "characters"),
        onclick: () => onTabChange && onTabChange("characters")
      }, "Characters / Animate ", el("span", { class: "stage-tab-count" }, `(${characters.length})`)),
      el("button", {
        id: "stage-tab-plates",
        role: "tab",
        class: `stage-tab${activeTab === "plates" ? " active" : ""}`,
        "aria-selected": String(activeTab === "plates"),
        onclick: () => onTabChange && onTabChange("plates")
      }, "Backgrounds & Plates ", el("span", { class: "stage-tab-count" }, `(${plates.length})`))
    ),
    grid
  );
}

function centralAssetsRail(assets, opts = {}) {
  return el("aside", { id: opts.id || "motion-asset-browser", "aria-label": opts.label || "Project assets" },
    el("div", { class: "panel-head inline" }, el("span", { class: "panel-title" }, `Assets (${(assets||[]).length})`)),
    projectAssetGrid(assets, opts));
}
