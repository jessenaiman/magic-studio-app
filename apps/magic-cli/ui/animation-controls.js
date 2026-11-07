const animationAssetsRail = location.hash === "#assets";
const animationContent = document.querySelector("#animation-controls-content");
const animationStatus = document.querySelector("#animation-status");
const { invoke: animationInvoke, convertFileSrc: animationFileUrl } = window.__TAURI__.core;
const animationPanels = new Map();
let animationState = null;
let animationVersion = null;
let animationRequest = false;
let animationError = "";
let animationRefreshId = 0;
let clipChoice = "";
let paceChoice = "baseline";
let durationChoice = 3;
let artworkDraft = null;

const rail = animationContent.closest("main");
rail.id = animationAssetsRail ? "motion-asset-browser" : "motion-inspector";
rail.classList.add("motion-inspector");
rail.setAttribute("aria-label", animationAssetsRail ? "Animation assets" : "Animation inspector");

function animationControl(action, fields = {}) {
  return animationInvoke("execute_command", {
    id: "ui.animation.controls",
    params: { action, project: animationState?.project, ...fields },
  });
}

async function animationDo(run) {
  if (animationRequest) return;
  animationRequest = true;
  animationError = "";
  try {
    await run();
    await updateAnimationRail();
  } catch (error) {
    animationError = String(error.message || error);
    animationStatus.textContent = animationError;
  } finally {
    animationRequest = false;
  }
}

function animationButton(label, action, fields = {}, disabled = false, className = "ghost") {
  return el("button", {
    class: className, type: "button", disabled,
    onclick: () => animationDo(() => animationControl(action, fields)),
  }, label);
}

function animationPanel(name, body, initiallyOpen = true) {
  return el("details", {
    class: "motion-section", open: animationPanels.get(name) ?? initiallyOpen,
    ontoggle: (event) => animationPanels.set(name, event.currentTarget.open),
  }, el("summary", { class: "panel-head", "data-look": "control" },
    el("span", { class: "title" }, name),
    el("span", { class: "chev", "aria-hidden": "true", html: '<svg viewBox="0 0 24 24"><path d="m9 6 6 6-6 6" fill="none" stroke="currentColor" stroke-width="2"/></svg>' })),
    el("div", { class: "motion-section-body" }, body));
}

function animationSelect(label, options, current, change) {
  const input = el("select", { onchange: (event) => change(event.target.value) },
    options.map(({ id, name }) => el("option", { value: id }, name)));
  input.value = current;
  return new InspectorField(label, input).node;
}

function renderAnimationAssets({ characters, assets }, state) {
  const available = assets.filter(asset => !isOverlayMedia(asset, state.project));
  const isComposition = asset => /\.ecproj$/i.test(asset.path || "");
  const artwork = new Map(available.filter(asset => !isComposition(asset)).map(asset => [asset.id, asset]));
  const projectArtwork = new Set(artwork.keys());
  const artworkId = id => id.replaceAll("_", "-");
  const aliases = new Map([...projectArtwork].map(id => [artworkId(id), id]));
  for (const character of characters) {
    if (!aliases.has(artworkId(character.id))) artwork.set(character.id, character);
  }
  const selected = artwork.has(state.character) ? state.character : aliases.get(artworkId(state.character)) || "";
  const chooseArtwork = animationSelect("Artwork",
    [{ id: "", name: "Choose artwork…" }, ...[...artwork.values()].map(asset => ({ id: asset.id, name: asset.name || asset.id }))],
    selected, (id) => {
      if (id) animationDo(() => projectArtwork.has(id) ? animationControl("asset", { id }) : animationControl("select", { character: id }));
    });
  const cards = available.map(asset => {
    const composition = isComposition(asset);
    return magicAssetTile(asset, {
      prefix: "animation-asset", canDrag: false,
      active: composition ? state.composition === asset.path : state.character === asset.id,
      placeholder: composition ? "COMP" : asset.kind === "model_3d" ? "3D" : "PART",
      onclick: () => animationDo(() => animationControl("asset", { id: asset.id })),
    });
  });
  animationContent.replaceChildren(el("div", { class: "panel-head inline" }, chooseArtwork),
    animationButton("Prepare artwork…", "prepare-artwork"),
    animationPanel("Assets", cards.length ? el("div", { class: "magic-assets-grid" }, cards) :
      el("p", { class: "muted" }, "Add artwork or an EffectCraft composition to this project.")));
}

function artworkPanel(metadata, state) {
  const view = metadata?.views?.find(item => item.view === state.view);
  const character = el("input", { type: "text", value: state.character, placeholder: "New character name" });
  const mark = el("button", { class: "ghost", type: "button", onclick: () => animationDo(async () => {
    const chosen = character.value.trim();
    if (!chosen) throw Error("Enter a character name first.");
    const picked = await animationInvoke("execute_command", { id: "ui.animation.pick_drawing", params: {} });
    if (picked?.path) await animationControl("mark", { image: picked.path, character: chosen });
  }) }, "Mark parts…");
  const views = ["front", "side", "back"].map(id => ({ id, name: id }));
  const key = `artwork-marks:${JSON.stringify([state.project, state.character, state.view, view?.revision])}`;
  if (artworkDraft?.key !== key) {
    const savedDraft = sessionStorage.getItem(key);
    artworkDraft = { key, parts: savedDraft ? JSON.parse(savedDraft) : structuredClone(view?.parts || []), dirty: Boolean(savedDraft) };
  }
  const draft = artworkDraft;
  const cut = animationButton("Cut & rig", "cut-rig", {}, !view?.marked || draft.dirty, "start");
  const save = el("button", { type: "button", class: "ghost", disabled: !draft.dirty,
    onclick: () => animationDo(async () => {
      await animationControl("save-marks", { parts: draft.parts, revision: view.revision });
      sessionStorage.removeItem(draft.key);
      draft.dirty = false;
      save.disabled = true;
    }) }, "Save marks");
  const editor = view?.drawing && view.size?.length === 2 && draft.parts.length ? artworkMarkEditor(view, draft, () => {
    draft.dirty = true;
    sessionStorage.setItem(draft.key, JSON.stringify(draft.parts));
    save.disabled = false;
    cut.disabled = true;
  }) : null;
  return animationPanel("Part artwork", [
    new InspectorField("Character", character).node,
    animationSelect("View", views, state.view, view => animationDo(() => animationControl("view", { view }))),
    editor || (view?.drawing ? el("img", { src: animationFileUrl(view.drawing), alt: `${state.view} artwork`, width: "180" }) : null),
    mark, editor ? save : null, cut,
    animationButton("Open in PhotoCraft", "open-craft", { app: "photocraft" }, !view?.cut),
    animationButton("Open Character Rig", "rig", {}, !view?.rigged),
  ], !metadata?.tripo_3d?.available && !view?.rigged);
}

function actionPanel(metadata, state) {
  const view = metadata?.views?.find(item => item.view === state.view);
  const labels = { walk_left: "Left", walk_right: "Right", walk_up: "Up", walk_down: "Down", idle: "Idle", jump: "Jump", turn: "Turn", wave: "Wave" };
  const actions = Object.entries(labels).map(([id, name]) =>
    animationButton(name, "key-action", { name: id }, !view?.rigged ||
      !metadata?.actions?.find(action => action.id === id)?.views?.includes(state.view), "motion-action"));
  return animationPanel("Action", [
    el("div", { class: "motion-actions-grid" }, actions),
    animationButton("Rest pose", "rest", {}, !view?.rigged),
  ], Boolean(view?.rigged));
}

function modelMotionPanel(metadata, options) {
  const clips = metadata?.tripo_3d?.model_clips || [];
  if (!clips.length) return null;
  if (!clips.includes(clipChoice)) clipChoice = clips[0];
  if (!options.some(option => option.id === paceChoice)) paceChoice = options[0]?.id;
  const duration = new InspectorNumberField({
    id: "animation-clip-duration", label: "Duration", unit: "s",
    min: 0.1, max: 30, step: 0.1, control: "dial", value: durationChoice,
    change: (value) => { durationChoice = value; },
  });
  return animationPanel("Motion & timing", [
    animationSelect("Clip", clips.map(id => ({ id, name: id.split("|").at(-1) })), clipChoice, value => { clipChoice = value; }),
    animationSelect("Pacing", options.map(option => ({ id: option.id, name: option.label })), paceChoice, value => { paceChoice = value; }),
    duration.node,
    el("button", { type: "button", class: "start", onclick: () => animationDo(() =>
      animationControl("prepare-motion", { clip: clipChoice, option: paceChoice, duration: durationChoice })) }, "Prepare in EffectCraft"),
  ]);
}

function renderAnimationInspector({ metadata, options }, state) {
  const view = metadata?.views?.find(item => item.view === state.view);
  const parts = (view?.parts || []).slice().reverse().map(part => el("div", { class: "motion-layer" }, part.id));
  animationContent.replaceChildren(...[
    modelMotionPanel(metadata, options),
    actionPanel(metadata, state),
    parts.length ? animationPanel("Layers", el("div", { class: "motion-layers" }, parts), false) : null,
    artworkPanel(metadata, state),
    animationPanel("Export & integrations", [
      animationButton("Render to Studio", "render-to-studio", {}, !state.composition, "start"),
      animationButton("Open in Blender", "open-craft", { app: "blender" }, !metadata?.tripo_3d?.blend_files?.length),
    ], false),
  ].filter(Boolean));
}

async function updateAnimationRail() {
  const refreshId = ++animationRefreshId;
  const next = await animationControl("state");
  if (refreshId !== animationRefreshId) return;
  const showArtwork = !animationAssetsRail && animationState && next.artwork_request !== animationState.artwork_request;
  animationState = next;
  const version = JSON.stringify([next.project, next.character, next.view, next.composition, next.revision]);
  if (version !== animationVersion) {
    const details = await animationControl(animationAssetsRail ? "catalog" : "inspector");
    if (refreshId !== animationRefreshId) return;
    if (animationAssetsRail) renderAnimationAssets(details, next);
    else renderAnimationInspector(details, next);
    animationVersion = version;
  }
  if (showArtwork) {
    const artwork = [...animationContent.querySelectorAll("details")].find(section => section.querySelector("summary .title").textContent === "Part artwork");
    if (artwork) {
      artwork.open = true;
      artwork.scrollIntoView({ block: "nearest" });
      artwork.querySelector("input")?.focus();
    }
  }
  animationStatus.textContent = animationError || next.error || (next.busy ? `Running ${next.busy}…` : "");
}

async function pollAnimationRail() {
  if (!animationRequest) {
    try {
      await updateAnimationRail();
    } catch (error) {
      if (!animationState) animationStatus.textContent = String(error.message || error);
    }
  }
  setTimeout(pollAnimationRail, 650);
}
pollAnimationRail();
