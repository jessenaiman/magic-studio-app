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
function characterMeta(raw: Record<string, any>, wanted?: string) {
  const view = characterPickView(raw.views || [], wanted);
  const parts = Array.isArray(view?.parts) ? view.parts : [];
  const tripo3d = raw.tripo_3d || { available: false };
  const steps: Array<[string, boolean]> = [
    ['Parts and joints marked (Scribbles)', Boolean(view?.marked)],
    ['Parts cut into layers (PhotoCraft)', Boolean(view?.cut)],
    ['Puppet rigged (EffectCraft)', Boolean(view?.rigged)],
    ['3D mesh & rig ready (Tripo / TextureCraft / Blender)', Boolean(tripo3d?.available && tripo3d?.rigged)],
  ];
  return {
    raw, view, views: raw.views || [], actions: raw.actions || [], tripo_3d: tripo3d,
    layers: parts.map((p: Record<string, any>, i: number) => ({ id: p.id, label: p.id, file: p.file || view?.drawing, parent: p.parent, pivot: p.joint, order: i, image_sha256: view?.revision })),
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
  const open = (app: string, label: string) => el('button', {
    id: `character-open-${app}`, class: 'ghost', 'data-commands': 'character.open',
    onclick: () => motionCommand('character.open', { character: motion.character, view: view?.view || 'front', app }).catch((e: unknown) => toast(String(e))),
  }, label);
  const openEc = open('effectcraft', 'Open in EffectCraft'), openPc = open('photocraft', 'Open in PhotoCraft'), openBl = open('blender', 'Open in Blender');
  openEc.disabled = !view?.rigged;
  openPc.disabled = !view?.cut;
  openBl.disabled = !(motion.meta?.tripo_3d?.blend_files?.length > 0);
  const scribbles = el('button', { id: 'character-scribbles', class: 'ghost', 'data-commands': 'scribbles.templates', onclick: () => openScribbles() }, 'Scribbles…');
  const tripoBreakout = characterTripoBreakoutControls(busy);
  const tripoRigAnim = characterTripoRigAndAnimControls(busy);
  return el('div', { class: 'panel-fields', id: 'character-prepare' },
    scribbles, new InspectorField('View', viewPicker).node, mark, save, cutRig, status,
    el('div', { class: 'motion-path-actions' }, openPc, openEc, openBl),
    tripoBreakout, tripoRigAnim);
}

const TRIPO_BREAKOUT_KINDS: Array<[string, string]> = [
  ['t_pose', 'T-Pose Breakout (Rig Ready)'],
  ['character_extraction', 'Character Extraction (Multi-Subject)'],
  ['asset_extraction', 'Asset / Costume Extraction'],
  ['split_image', 'Auto-Split Cutout (PhotoCraft)'],
  ['multiview_images', 'Generate 4-View Sheet (Front/L/Back/R)'],
  ['character_completion', 'Character Completion'],
  ['head_extraction', 'Head Extraction'],
  ['3d_enhance', '3D Structure Enhance'],
  ['upscale_image', '4K Upscale'],
  ['ai_segmentation', '3D AI Mesh Segmentation'],
  ['mesh_completion', '3D Fill Parts (Quick Cap / AI)'],
];

const TRIPO_PRESETS_QUICK: Array<[string, string]> = [
  ['wave_goodbye_01', 'Wave Goodbye'],
  ['bow', 'Bow'],
  ['idle', 'Idle'],
  ['walk', 'Walk'],
  ['run', 'Run'],
  ['jump', 'Jump'],
  ['cheer', 'Cheer'],
  ['clap', 'Clap'],
  ['dance_01', 'Dance 01'],
  ['sing_01', 'Sing 01'],
  ['laugh_01', 'Laugh'],
  ['turn', 'Turn'],
];

function characterTripoBreakoutControls(busy: (label: string, run: () => Promise<unknown>) => Promise<void>) {
  const kindSelect = el('select', { id: 'tripo-breakout-kind', 'aria-label': 'Tripo breakout kind' },
    ...TRIPO_BREAKOUT_KINDS.map(([val, label]) => el('option', { value: val }, label)));
  const granularitySelect = el('select', { id: 'tripo-segment-granularity', 'aria-label': '3D segmentation granularity' },
    ...['low', 'medium', 'high'].map((g) => el('option', { value: g }, `${g} granularity`)));
  granularitySelect.value = 'medium';
  const runBreakoutBtn = el('button', {
    id: 'tripo-breakout-run', class: 'ghost', 'data-commands': 'character.tripo.breakout',
    title: 'Run post-image breakout (Template, PhotoCraft Cutout, Multiview or 3D Segmentation)',
    onclick: () => busy(`Running Tripo breakout (${kindSelect.value})…`, () => motionCommand('character.tripo.breakout', {
      character: motion.character,
      kind: kindSelect.value,
      granularity: granularitySelect.value,
      reference: motion.meta?.view?.drawing || '',
    })),
  }, 'Run Breakout Step');
  return el('div', { class: 'seq-side-block', id: 'tripo-breakout-panel' },
    el('strong', {}, 'Tripo Breakout & Parts'),
    new InspectorField('Breakout', kindSelect).node,
    new InspectorField('Parts', granularitySelect).node,
    runBreakoutBtn);
}

function characterTripoRigAndAnimControls(busy: (label: string, run: () => Promise<unknown>) => Promise<void>) {
  const t3d = motion.meta?.tripo_3d || {};
  const rigTypeSelect = el('select', { id: 'tripo-rig-type', 'aria-label': 'Tripo rig type' },
    ...['biped', 'quadruped', 'hexapod', 'octopod', 'avian', 'serpentine', 'aquatic', 'others'].map((r) => el('option', { value: r }, r)));
  rigTypeSelect.value = t3d.rig_type || 'biped';
  const skelSelect = el('select', { id: 'tripo-skeleton-preset', 'aria-label': 'Tripo skeleton preset' },
    ...[['mixamo', 'Mixamo'], ['actorcore', 'ActorCore'], ['unreal', 'UE5 Mannequin'], ['vrm', 'VRM 1.0 Humanoid'], ['unity', 'Unity Humanoid']].map(([v, l]) => el('option', { value: v }, l)));
  skelSelect.value = t3d.skeleton_preset || 'mixamo';
  const rigBtn = el('button', {
    id: 'tripo-rig-run', class: 'ghost', 'data-commands': 'character.tripo.rig',
    title: 'Run Tripo Pre-Rig Check (TextureCraft mesh/UV check) and configure 3D Auto-Rigging',
    onclick: () => busy('Checking 3D mesh in TextureCraft & configuring Tripo rig…', () => motionCommand('character.tripo.rig', {
      character: motion.character,
      rig_type: rigTypeSelect.value,
      skeleton_preset: skelSelect.value,
    })),
  }, 'Pre-Rig Check & Auto-Rig 3D');

  const presetSelect = el('select', { id: 'tripo-preset-select', 'aria-label': 'Tripo animation preset' },
    ...TRIPO_PRESETS_QUICK.map(([v, l]) => el('option', { value: v }, l)));
  const fbxPresetSelect = el('select', { id: 'tripo-fbx-preset', 'aria-label': 'FBX export preset' },
    ...['blender', '3dsmax', 'maya', 'unreal', 'unity'].map((p) => el('option', { value: p }, p)));
  fbxPresetSelect.value = t3d.fbx_preset || 'blender';
  const applyPresetBtn = el('button', {
    id: 'tripo-animate-preset-run', class: 'start', 'data-commands': 'character.tripo.animate',
    title: 'Save split .anim.json clip and build EffectCraft Advanced 3D model comp',
    onclick: () => busy(`Retargeting 3D preset ${presetSelect.value}…`, () => motionCommand('character.tripo.animate', {
      character: motion.character,
      preset: presetSelect.value,
      rig_type: rigTypeSelect.value,
      fbx_preset: fbxPresetSelect.value,
      animate_in_place: true,
    })),
  }, 'Add 3D Preset Clip (.glb + .anim)');

  const stagePrompt = el('input', {
    id: 'tripo-multistage-prompt', type: 'text', placeholder: 'e.g. settle, wave twice, bow',
    'aria-label': 'Multi-stage motion prompt', value: 'two friendly waves, then small bow',
  }) as HTMLInputElement;
  const applyMultiStageBtn = el('button', {
    id: 'tripo-multistage-run', class: 'ghost', 'data-commands': 'character.tripo.animate',
    title: 'Author Tripo Multi-Stage Motion clip (.anim.json + EffectCraft 3D comp)',
    onclick: () => {
      const parts = (stagePrompt.value || 'wave').split(',').map((s) => s.trim()).filter(Boolean).slice(0, 5);
      const stages = parts.map((p, idx) => ({ name: idx === 0 ? 'Start' : String.fromCharCode(64 + idx), prompt: p, duration_seconds: 3, waypoints: [[0, 0], [0, 0]] }));
      return busy('Saving Tripo Multi-Stage Motion clip…', () => motionCommand('character.tripo.animate', {
        character: motion.character,
        name: parts.join('_').slice(0, 32),
        stages,
        rig_type: rigTypeSelect.value,
        fbx_preset: fbxPresetSelect.value,
        animate_in_place: true,
      }));
    },
  }, 'Author Multi-Stage Motion');

  const clipsList = el('div', { class: 'tripo-clip-list', id: 'tripo-clip-list' },
    ...(t3d.clips || []).map((c: Record<string, any>) => el('div', { class: 'tripo-clip-row' },
      el('span', {}, `${c.name} (${c.duration || 0}s · ${c.source || '3d'})`),
      el('button', {
        class: 'ghost', title: 'Preview 3D clip in viewport',
        onclick: async () => {
          motion.viewport3d = true;
          const c2d = $('motion-canvas'), c3d = $('motion-canvas-3d');
          if (c2d) c2d.hidden = true;
          if (c3d) { c3d.hidden = false; if (!tripo3DStage.renderer) tripo3DStage.mount(c3d); }
          if (c.glb) await tripo3DStage.loadGlb(c.glb);
          const dur = tripo3DStage.duration || c.duration || 8;
          motion.rest = false;
          motion.track = c.id || '3d_motion';
          motion.sample = { duration: dur, loop: true, fps: 24, frames: [], start: 0 };
          if ($('motion-scrub')) $('motion-scrub').max = dur;
          animationDraw();
        },
      }, '3D View'))));

  return el('div', { class: 'seq-side-block', id: 'tripo-rig-anim-panel' },
    el('strong', {}, `Tripo 3D Rig & Motion (${t3d.available ? `${t3d.import_summary?.bones || 65} bones` : 'no mesh'})`),
    new InspectorField('Rig Type', rigTypeSelect).node,
    new InspectorField('Skeleton', skelSelect).node,
    rigBtn,
    new InspectorField('Preset', presetSelect).node,
    new InspectorField('FBX Target', fbxPresetSelect).node,
    applyPresetBtn,
    new InspectorField('Multi-Stage', stagePrompt).node,
    applyMultiStageBtn,
    clipsList);
}

/** Save a clip the film can use: ProRes 4444 with transparency, rendered by EffectCraft. */
async function characterExportClip(status: HTMLElement) {
  if (!motion.sample || motion.rest) throw Error('Choose an action before saving.');
  status.textContent = 'EffectCraft is rendering the clip…';
  const start = motion.sample.start, end = start + motion.sample.duration;
  const result = await motionCommand('character.render', { character: motion.character, view: motion.view, start, end, fps: CHARACTER_FPS, format: 'clip' });
  motion.lastExport = { project: state.project, character: motion.character, action: motion.track, result };
  const from = Number(($('authoring-place-start') as HTMLInputElement | null)?.value || 0);
  status.textContent = 'Placing the clip on the film (FilmCraft V2)…';
  await motionCommand('video.place_clip', { clip: result.clip, from, track: 'V2' });
  status.textContent = `Clip saved and placed on the film at ${from.toFixed(2)} s · open Studio to watch`;
  status.append(fileButton({ path: result.clip, name: String(result.clip).split('/').pop() }));
}
