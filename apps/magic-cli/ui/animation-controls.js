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
let durationChoice = "3";
let artworkDraft = null;

document.querySelector("#animation-controls-title").textContent = animationAssetsRail ? "Assets" : "Animation";

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
  }, el("summary", {}, name), el("div", { class: "motion-section-body" }, body));
}

function animationSelect(label, options, current, change) {
  const input = el("select", { onchange: (event) => change(event.target.value) },
    options.map(({ id, name }) => el("option", { value: id }, name)));
  input.value = current;
  return new InspectorField(label, input).node;
}

function renderAnimationAssets({ characters, assets }, state) {
  const chooseCharacter = animationSelect("Character",
    [{ id: "", name: "Choose a character…" }, ...characters.map(character => ({ id: character.id, name: character.id }))],
    state.character, (character) => character && animationDo(() => animationControl("select", { character })));
  const cards = assets.map(asset => {
    const composition = /\.ecproj$/i.test(asset.path || "");
    const art = /\.(png|jpe?g|webp|gif)$/i.test(asset.path || "");
    const active = composition ? state.composition === asset.path : state.character === asset.id;
    return el("button", {
      type: "button", class: `magic-asset-tile${active ? " active" : ""}`,
      id: `animation-asset-${asset.id}`, title: `${asset.name} · ${asset.path || asset.kind}`,
      "aria-pressed": String(active),
      onclick: () => animationDo(() => animationControl("asset", { id: asset.id })),
    }, art ? el("img", { src: animationFileUrl(asset.path), alt: "" }) :
      el("span", { class: "magic-asset-placeholder" }, composition ? "COMP" : asset.kind === "model_3d" ? "3D" : "ASSET"),
    el("span", { class: "magic-asset-name" }, asset.name || asset.id));
  });
  animationContent.replaceChildren(chooseCharacter,
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
  const duration = el("input", { type: "number", min: "0.1", max: "30", step: "0.1", value: durationChoice,
    oninput: (event) => { durationChoice = event.target.value; } });
  const note = el("p", { class: "muted" }, options.find(option => option.id === paceChoice)?.description || "");
  return animationPanel("Motion & timing", [
    animationSelect("Clip", clips.map(id => ({ id, name: id.split("|").at(-1) })), clipChoice, value => { clipChoice = value; }),
    animationSelect("Motion option", options.map(option => ({ id: option.id, name: option.label })), paceChoice, value => {
      paceChoice = value;
      note.textContent = options.find(option => option.id === value)?.description || "";
    }),
    note,
    new InspectorField("Duration (s)", duration).node,
    el("button", { type: "button", class: "start", onclick: () => animationDo(() => {
      if (!duration.reportValidity() || !duration.value) return;
      return animationControl("prepare-motion", { clip: clipChoice, option: paceChoice, duration: Number(duration.value) });
    }) }, "Prepare in EffectCraft"),
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
    const artwork = [...animationContent.querySelectorAll("details")].find(section => section.querySelector("summary").textContent === "Part artwork");
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
