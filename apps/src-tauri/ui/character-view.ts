// The Animation tab's data, from the `character.*` commands: Scribbles marks the drawing,
// PhotoCraft cuts it, EffectCraft rigs, keys and renders it. This file only calls commands and
// shapes their answers for the existing panels; it computes no motion.

/** The Action buttons, the pose-table action each plays and the drawn view it needs. */
const CHARACTER_ACTIONS: { button: string; label: string; action: string }[] = [
  { button: 'left', label: 'Left', action: 'walk_left' },
  { button: 'right', label: 'Right', action: 'walk_right' },
  { button: 'up', label: 'Up', action: 'walk_up' },
  { button: 'down', label: 'Down', action: 'walk_down' },
  { button: 'idle', label: 'Idle', action: 'idle' },
  { button: 'walk', label: 'Walk', action: 'walk_down' },
  { button: 'jump', label: 'Jump', action: 'jump' },
  { button: 'sway', label: 'Turn', action: 'turn' },
  { button: 'wave', label: 'Wave', action: 'wave' },
];
const CHARACTER_FPS = 24;

/** Characters that have at least one drawn view, as tiles for the asset browser. */
async function characterCatalog() {
  const list = await motionCommand('character.list');
  return (list.characters || []).map((c: any) => ({
    id: c.id, name: c.id.replace(/^./, (s: string) => s.toUpperCase()), kind: 'character',
    path: c.views.find((v: any) => v.drawing)?.drawing || '', views: c.views,
  }));
}

/** The view to show: the one the last action used, else the first rigged, else the first drawn. */
function characterPickView(views: any[], wanted?: string) {
  return views.find((v) => v.view === wanted) || views.find((v) => v.rigged) || views[0] || null;
}

/** `character.meta` shaped as the panels expect: layers, (no) tracks, actions, quality checks. */
function characterMeta(raw: any, wanted?: string) {
  const view = characterPickView(raw.views || [], wanted);
  const parts = Array.isArray(view?.parts) ? view.parts : [];
  const steps = [
    ['Parts and joints marked (Scribbles)', Boolean(view?.marked)],
    ['Parts cut into layers (PhotoCraft)', Boolean(view?.cut)],
    ['Puppet rigged (EffectCraft)', Boolean(view?.rigged)],
  ];
  return {
    raw, view, views: raw.views || [], actions: raw.actions || [],
    layers: parts.map((p: any, i: number) => ({ id: p.id, label: p.id, file: p.file || view?.drawing, parent: p.parent, pivot: p.joint, order: i, image_sha256: view?.revision })),
    tracks: [], placed_tracks: [], controls: {}, character_panel: { available: false },
    rig_sha256: view?.rigged ? String(view.revision) : null, image_revision: view?.revision || 0,
    canvas: view?.size,
    preflight: { total_checks: steps.length, passed_count: steps.filter((s) => s[1]).length, checks: steps.map(([label, ok]) => ({ label, ok })) },
  };
}

/** Which drawn view an action plays in, given what has been rigged. */
function characterActionView(action: string) {
  const entry = motion.meta?.actions?.find((a: any) => a.id === action);
  const rigged = (motion.meta?.views || []).filter((v: any) => v.rigged).map((v: any) => v.view);
  return (entry?.views || []).find((v: string) => rigged.includes(v)) || null;
}

/** Key the action into the rig (once), render its frames with EffectCraft, then play them. */
async function characterPlayAction(action: string, play = true) {
  const view = characterActionView(action);
  if (!view) { toast(`Draw, mark and rig the ${motion.meta?.actions?.find((a: any) => a.id === action)?.views?.join(' or ') || 'needed'} view first`); return; }
  animationPause();
  motion.stageHeader.rename(`${motion.character} · keying ${action.replaceAll('_', ' ')} in EffectCraft…`);
  try {
    const keyed = await motionCommand('character.action', { character: motion.character, view, action });
    const render = await motionCommand('character.render', { character: motion.character, view, start: keyed.start, end: keyed.end, fps: CHARACTER_FPS });
    await characterStage.preload(render.frames);
    Object.assign(motion, {
      track: action, rest: false, time: 0, view,
      sample: { duration: keyed.end - keyed.start, loop: keyed.loop === true, fps: CHARACTER_FPS, frames: render.frames, start: keyed.start },
    });
    acFeedback.report(null, null, { stage: 'clip_switched', clip: action });
    $('motion-scrub').max = motion.sample.duration;
    animationControls();
    animationDraw();
    if (play) await animationPlay();
  } catch (error) { toast(String(error)); motion.stageHeader.rename(`${motion.character} · ${error}`); }
}

/** Rest: the drawing itself, with its marks when it has not been rigged yet. */
function characterShowRest() {
  const view = motion.meta?.view;
  if (!view?.drawing) return;
  characterStage.setSize(view.size);
  characterStage.fit();
  characterStage.show(view.drawing);
  characterStage.showMarks(view.rigged ? null : view.parts, !view.cut);
}

/** "Prepare artwork": mark a drawing (Scribbles), review the marks, then cut and rig. */
function characterPreparePanel() {
  const view = motion.meta?.view;
  const status = el('p', { id: 'character-prepare-status', class: 'muted', role: 'status' },
    !view ? 'Choose a drawing of this character to start.'
      : !view.marked ? 'Mark the parts and joints of this drawing.'
        : !view.cut ? 'Drag joints and outline points until they sit right, then cut and rig.'
          : view.rigged ? `${view.view} view is rigged in EffectCraft.` : 'Cut; rig it next.');
  const busy = async (label: string, run: () => Promise<any>) => {
    status.textContent = label;
    try { await run(); await animationRefresh(); } catch (error) { status.textContent = String(error); }
  };
  const viewPicker = el('select', { id: 'character-prepare-view', 'aria-label': 'Drawn view' },
    ...['front', 'side', 'back'].map((v) => el('option', { value: v }, v)));
  viewPicker.value = view?.view || 'front';
  const mark = el('button', {
    id: 'character-mark', class: 'ghost', 'data-commands': 'character.mark', title: 'Scribbles marks parts and joints on a drawing',
    onclick: async () => {
      const image = await window.__TAURI__.dialog.open({ title: 'Choose the drawing to animate', filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] });
      if (!image) return;
      await busy('Scribbles is marking the parts… (a few minutes)', () => motionCommand('character.mark', { character: motion.character || 'character', view: viewPicker.value, image }));
    },
  }, 'Mark parts…');
  const save = el('button', {
    id: 'character-save-marks', class: 'ghost', 'data-commands': 'character.save_marks',
    onclick: () => busy('Saving marks…', () => motionCommand('character.save_marks', { character: motion.character, view: view.view, parts: characterStage.marks })),
  }, 'Save marks');
  const cutRig = el('button', {
    id: 'character-cut-rig', class: 'start', 'data-commands': 'character.cut character.rig',
    onclick: () => busy('PhotoCraft is cutting, EffectCraft is rigging…', async () => {
      if (characterStage.marks) await motionCommand('character.save_marks', { character: motion.character, view: view.view, parts: characterStage.marks });
      await motionCommand('character.cut', { character: motion.character, view: view.view });
      await motionCommand('character.rig', { character: motion.character, view: view.view });
    }),
  }, 'Cut & rig');
  save.disabled = cutRig.disabled = !view?.marked;
  const open = (app: string) => el('button', {
    id: `character-open-${app}`, class: 'ghost', 'data-commands': 'character.open',
    onclick: () => motionCommand('character.open', { character: motion.character, view: view.view, app }).catch((e: any) => toast(String(e))),
  }, app === 'effectcraft' ? 'Open in EffectCraft' : 'Open in PhotoCraft');
  const openEc = open('effectcraft'), openPc = open('photocraft');
  openEc.disabled = !view?.rigged;
  openPc.disabled = !view?.cut;
  const scribbles = el('button', { id: 'character-scribbles', class: 'ghost', 'data-commands': 'scribbles.templates', onclick: () => openScribbles() }, 'Scribbles…');
  return el('div', { class: 'panel-fields', id: 'character-prepare' },
    scribbles, new InspectorField('View', viewPicker).node, mark, save, cutRig, status, el('div', { class: 'motion-path-actions' }, openPc, openEc));
}

/** Save a clip the film can use: ProRes 4444 with transparency, rendered by EffectCraft. */
async function characterExportClip(status: HTMLElement) {
  if (!motion.sample || motion.rest) throw Error('Choose an action before saving.');
  status.textContent = 'EffectCraft is rendering the clip…';
  const start = motion.sample.start, end = start + motion.sample.duration;
  const result = await motionCommand('character.render', { character: motion.character, view: motion.view, start, end, fps: CHARACTER_FPS, format: 'clip' });
  motion.lastExport = { project: state.project, character: motion.character, action: motion.track, result };
  status.textContent = 'Clip saved';
  status.append(fileButton({ path: result.clip, name: String(result.clip).split('/').pop() }));
}
