// The engine samples motion; this view only draws images using returned matrices.
function loadStoredOpenSections(): Record<string, boolean> {
  try { return JSON.parse(localStorage.getItem('animation.openSections') || '{}') || {}; }
  catch { return {}; }
}

class MotionStateController {
  project: string | null = null;
  mode = 'subject';
  character: string | null = null;
  meta: any = null;
  sample: any = null;
  images: HTMLImageElement[] = [];
  assets: any[] = [];
  time = 0;
  playing = false;
  tick = 0;
  playId = 0;
  resizeObserver: ResizeObserver | null = null;
  zoom = 1;
  fit = true;
  track = '';
  rest = true;
  amount = 100;
  speed = 1;
  selected: string | null = null;
  partsTab = 'compare';
  photoDrafts: Record<string, any> = {};
  openSections: Record<string, boolean> = loadStoredOpenSections();
  loading = false;
  request = 0;
  grid = localStorage.getItem("animation.grid") === "true";
  anchors = false;
  labels = false;
  pen = false;
  pathVisible = true;
  points: number[][] = [];
  pathDuration = 5;
  startSize = 100;
  endSize = 100;
  pathUndo: string | null = null;
  pathLayer: string | null = null;
  previewLoop: boolean | null = null;
  [key: string]: any;

  feedback() {
    const selectedDepth = this.meta?.layers?.find((l: any) => l.id === this.selected)?.depth ?? null;
    return {
      backend: this.mode, character: this.character, track: this.track, rest: this.rest,
      time: this.time, playing: this.playing, selected_layer: this.selected,
      speed: this.speed, amount: this.amount, duration: this.sample?.duration ?? null,
      loop: this.sample?.loop ?? null, rig_sha256: this.meta?.rig_sha256 ?? null,
      layer_order: this.meta?.layer_order || null, selected_layer_depth: selectedDepth,
      open_sections: { ...this.openSections }, refresh_error: this.refreshError || null,
      last_export: this.lastExport?.project === state.project ? this.lastExport : null,
      last_edit: this.lastEdit?.project === state.project ? this.lastEdit : null,
      last_pose: this.lastPose?.project === state.project ? this.lastPose : null,
      placed_tracks: this.meta?.placed_tracks || [], last_placement: this.lastPlacement || null,
    };
  }

  remember() {
    if (!state.project || this.project !== state.project || !this.character) return;
    const value = {
      project: state.project, character: this.character, track: this.track,
      selected: this.selected, rest: this.rest, time: this.time, speed: this.speed,
      amount: this.amount, previewLoop: this.previewLoop,
      points: this.points.map((p) => [...p]), pathUndo: this.pathUndo,
      pathLayer: this.pathLayer, pathDuration: this.pathDuration,
      startSize: this.startSize, endSize: this.endSize,
    };
    const serialized = JSON.stringify(value);
    if (this.remembered === serialized) return;
    this.remembered = serialized;
    try {
      const prior = JSON.parse(localStorage.getItem('animation.sessions') || '[]');
      const filtered = (Array.isArray(prior) ? prior : []).filter((v: any) => v.project !== state.project);
      localStorage.setItem('animation.sessions', JSON.stringify([value, ...filtered].slice(0, 20)));
    } catch {}
  }

  restore() {
    try {
      const entries = JSON.parse(localStorage.getItem('animation.sessions') || '[]');
      const v = Array.isArray(entries) ? entries.find((item: any) => item.project === state.project) : null;
      if (!v || typeof v.character !== 'string' || typeof v.track !== 'string') return null;
      return buildRestoredSession(v);
    } catch { return null; }
  }

  reset() {
    theatreRequireSaved();
    this.remember();
    animationJobsDispose();
    animationPause();
    Object.assign(this, {
      project: null, character: null, meta: null, sample: null, images: [], assets: [],
      time: 0, track: '', rest: true, selected: null, photoDrafts: {}, loading: false,
      request: this.request + 1, points: [], pathUndo: null, pathLayer: null,
      previewLoop: null, pen: false, fit: true, zoom: 1,
    });
    $('view-animation').replaceChildren();
  }
}

const motion: Record<string, any> = new MotionStateController();

function animationFeedback() { return motion.feedback(); }
function animationRemember() { motion.remember(); }

function clampSessionNumber(v: any, key: string, fallback: number, min: number, max: number): number {
  return Number.isFinite(v[key]) && v[key] >= min && v[key] <= max ? v[key] : fallback;
}
function parseSessionPoints(points: any): number[][] {
  if (!Array.isArray(points) || points.length > 512) return [];
  if (!points.every((p) => Array.isArray(p) && p.length === 2 && p.every(Number.isFinite))) return [];
  return points.map((p) => [...p]);
}
function optionalSessionString(val: any): string | null {
  return typeof val === 'string' ? val : null;
}
function buildRestoredSession(v: any) {
  return {
    character: v.character, track: v.track, selected: optionalSessionString(v.selected), rest: v.rest === true,
    time: clampSessionNumber(v, 'time', 0, 0, 600), speed: clampSessionNumber(v, 'speed', 1, 0.25, 2.25),
    amount: clampSessionNumber(v, 'amount', 100, 0, 100),
    previewLoop: typeof v.previewLoop === 'boolean' ? v.previewLoop : null,
    points: parseSessionPoints(v.points),
    pathUndo: optionalSessionString(v.pathUndo), pathLayer: optionalSessionString(v.pathLayer),
    pathDuration: clampSessionNumber(v, 'pathDuration', 5, 0.1, 600),
    startSize: clampSessionNumber(v, 'startSize', 100, 1, 1000),
    endSize: clampSessionNumber(v, 'endSize', 100, 1, 1000),
  };
}
function animationRestore() { return motion.restore(); }
function animationReset() { motion.reset(); }
const motionCommand = (id: string, params: any = {}) => invoke('engine_command', { id, params });
const motionAssetUrl = (path: string, sha?: string) => {
  const src = String(convertFileSrc(path));
  return `${src}${src.includes('?') ? '&' : '?'}v=${sha || motion.meta?.rig_sha256 || motion.meta?.image_revision || 0}`;
};
const MOTION_ICONS: Record<string, string> = {
  left:'<path d="M19 12H5m6-7-7 7 7 7"/>',
  right:'<path d="M5 12h14m-6-7 7 7-7 7"/>',
  up:'<path d="M12 19V5m-7 6 7-7 7 7"/>',
  down:'<path d="M12 5v14m-7-6 7 7 7-7"/>',
  idle:'<circle cx="12" cy="5" r="2"/><path d="M12 8v8m-5-5 5-3 5 3m-5 5-3 5m3-5 3 5"/>',
  move:'<path d="M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3"/>',
  jump:'<path d="M12 17V3m-5 5 5-5 5 5M4 21h16"/>',
  walk:'<ellipse cx="7" cy="7" rx="3" ry="5" transform="rotate(-20 7 7)"/><ellipse cx="17" cy="17" rx="3" ry="5" transform="rotate(20 17 17)"/>',
  run:'<circle cx="15" cy="4" r="2"/><path d="m4 10 5-1 4-3 3 5h4M13 7l-3 7 5 3-2 5M10 14l-4 6H2"/>',
  wave:'<path d="M8 13V6a2 2 0 0 1 4 0v5-7a2 2 0 0 1 4 0v8-5a2 2 0 0 1 4 0v9a6 6 0 0 1-11 3l-5-6a2 2 0 0 1 3-3l3 3M3 3l2 2M21 2l-2 2"/>',
  talk:'<path d="M21 11a8 8 0 0 1-8 8H7l-5 3 2-6a8 8 0 1 1 17-5Z"/><path d="M8 10h8M8 14h5"/>',
  sway:'<path d="M3 12c3-10 6-10 9 0s6 10 9 0M3 5v2m18 10v2"/>',
  rest:'<path d="M20 14A9 9 0 0 1 10 3a9 9 0 1 0 10 11Z"/>',
  pen:'<path d="m16 3 5 5-12 12-6 1 1-6zM14 5l5 5"/>', grid:'<path d="M3 3h18v18H3zM3 9h18M3 15h18M9 3v18M15 3v18"/>',
  play:'<path d="m8 5 11 7-11 7z"/>', pause:'<path d="M8 5v14M16 5v14"/>',
  reset:'<path d="M3 10a9 9 0 1 1 2 9M3 4v6h6"/>', fit:'<path d="M8 3H3v5M16 3h5v5M3 16v5h5M21 16v5h-5"/>',
  minus:'<path d="M5 12h14"/>', plus:'<path d="M5 12h14M12 5v14"/>', eye:'<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7z"/><circle cx="12" cy="12" r="3"/>',
  layers:'<path d="m12 3 9 5-9 5-9-5zM3 13l9 5 9-5M3 18l9 5 9-5"/>', export:'<path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5"/>', step:'<path d="m5 5 10 7-10 7zM19 5v14"/>',
  trash:'<path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6h14z"/>'
};
function motionButton(id: string, label: string, icon: string, fn: any) {
  return el('button', { id, class: 'icon', title: label, 'aria-label': label, html: `<svg viewBox="0 0 24 24">${MOTION_ICONS[icon]}</svg>`, onclick: fn });
}
function motionSection(title: string, body: any, open = false, componentId: string | null = null) {
  const id = componentId || uiConfig.sections?.[title];
  if (!id || !uiConfig.elements[id]) throw Error(`Unconfigured animation panel: ${title}`);
  const heading = new PanelHeader({
    id: `${id}-head`, title, collapsible: true,
    icon: `<svg viewBox="0 0 24 24">${MOTION_ICONS[uiConfig.elements[id].icon]}</svg>`,
  }).element;
  const node = el('details', { id, class: 'motion-section' }, heading, el('div', { class: 'motion-section-body' }, body));
  node.open = Object.hasOwn(motion.openSections, title) ? motion.openSections[title] === true : open;
  node.addEventListener('toggle', () => {
    motion.openSections[title] = node.open;
    localStorage.setItem('animation.openSections', JSON.stringify(motion.openSections));
  });
  return node;
}
function motionRange(id: string, label: string, min: number, max: number, step: number, value: number, unit: string, change: (v: number) => void) {
  const out = el('output', {}, `${Number(Number(value).toFixed(2))}${unit}`);
  const input = el('input', { id, type: 'range', min, max, step, value, 'aria-label': label, title: `${label}: ${min}–${max}${unit}` });
  const fill = () => input.style.setProperty('--range-fill', `${max > min ? 100 * (Number(input.value) - min) / (max - min) : 0}%`);
  fill();
  input.addEventListener('input', () => { out.textContent = `${input.value}${unit}`; fill(); change(Number(input.value)); });
  const node = el('label', { class: 'motion-range' }, el('span', {}, label, out), input);
  node.input = input;
  return node;
}
class AnimationViewController {
  async openCharacter(character: string) {
    if (!theatreCanLeave()) return false;
    if (state.view === 'animation' && motion.project === state.project && motion.mode === 'subject') {
      if (motion.character === character && motion.meta) { animationDraw(); return true; }
      motion.character = character;
      motion.selected = null;
      updateMagicSelection();
      await this.load();
      return true;
    }
    motion.character = character;
    motion.selected = null;
    motion.project = null;
    motion.explicitCharacter = true;
    await setView('animation');
  }

  async open() {
    motion.mode = 'subject';
    if (motion.project === state.project && motion.meta) { animationDraw(); return; }
    theatreRequireSaved();
    motion.project = state.project;
    motion.meta = null;
    motion.sample = null;
    motion.time = 0;
    motion.rest = true;
    motion.resume = motion.explicitCharacter ? null : animationRestore();
    motion.explicitCharacter = false;
    if (motion.resume) Object.assign(motion, motion.resume);
    this.buildShell();
    try {
      const assets = await motionCommand('project.list_assets');
      const list = assets.assets || assets.project_assets || [];
      motion.assets = list;
      if (!list.some((a: any) => a.id === motion.character)) motion.character = list[0]?.id;
      if (!motion.character) {
        this.renderEmpty('Bring in your artwork', 'Choose an object from the Storyboard view.');
        return;
      }
      await this.load();
    } catch (e) { this.renderEmpty('Artwork needs attention', String(e)); }
  }

  renderEmpty(title: string, detail: string, loading = false) {
    const stage = $('motion-stage');
    if (!stage) return;
    $('motion-stage-status')?.remove();
    const canvas = $('motion-canvas');
    if (canvas) canvas.hidden = true;
    stage.append(el('div', { id: 'motion-stage-status', class: 'panel-empty motion' },
      el('strong', {}, title),
      el('p', { class: 'muted' }, detail),
      loading ? null : el('button', { class: 'ghost', onclick: () => setView('project') }, 'Open Project')));
  }

  buildStageToolbar() {
    const grid = motionButton('motion-grid', 'Show or hide grid', 'grid', () => {
      motion.grid = !motion.grid;
      localStorage.setItem('animation.grid', String(motion.grid));
      grid.setAttribute('aria-pressed', String(motion.grid));
      animationDraw();
    });
    grid.setAttribute('aria-pressed', String(motion.grid));
    const penBtn = motionButton('motion-pen', 'Pen · draw a motion path', 'pen', () => {
      animationPause();
      motion.pen = !motion.pen;
      $('motion-pen').setAttribute('aria-pressed', String(motion.pen));
      $('motion-canvas').classList.toggle('drawing', motion.pen);
      $('motion-path-section')?.setAttribute('open', '');
    });
    const pathVisBtn = motionButton('motion-path-visible', 'Show or hide the motion path', 'eye', () => {
      motion.pathVisible = !motion.pathVisible;
      $('motion-path-visible').setAttribute('aria-pressed', String(motion.pathVisible));
      animationDraw();
    });
    return new PanelHeader({
      id: 'motion-stagebar', titleId: 'motion-caption', title: 'Loading artwork…',
      actions: [
        penBtn, pathVisBtn, grid,
        motionButton('motion-fit', 'Fit artwork', 'fit', () => { motion.fit = true; motion.zoom = 1; animationDraw(); }),
        el('button', { id: 'motion-actual', title: 'Actual size · 100%', 'aria-label': 'Actual size', onclick: () => { motion.fit = false; motion.zoom = 1; animationDraw(); } }, '1:1'),
        motionButton('motion-minus', 'Zoom out', 'minus', () => { motion.zoom /= 1.2; animationDraw(); }),
        motionButton('motion-plus', 'Zoom in', 'plus', () => { motion.zoom *= 1.2; animationDraw(); }),
        el('button', {
          id: 'btn-generate-all-sheets', class: 'icon', 'aria-label': 'Generate missing character sheets',
          html: `<svg viewBox="0 0 24 24">${MOTION_ICONS.layers}</svg>`,
          'data-commands': 'assets.sheet_plan',
          title: 'Review complete asset sheets and edit drawing prompts',
          onclick: () => reviewAssetSheets(motion.character, null),
        }),
      ],
    });
  }

  buildTransportBar() {
    return el('div', { class: 'motion-transport' },
      motionButton('motion-play', 'Play motion', 'play', animationPlay),
      motionButton('motion-rewind', 'Return to start', 'reset', () => { animationPause(); motion.time = 0; animationDraw(); }),
      motionButton('motion-step-back', 'Previous frame', 'step', () => animationStep(-1)),
      motionButton('motion-step', 'Next frame', 'step', () => animationStep(1)),
      el('span', { id: 'motion-time', class: 'motion-time' }, '0:00.000'),
      el('input', { id: 'motion-scrub', type: 'range', min: 0, max: 1, step: 0.001, value: 0, 'aria-label': 'Scrub motion', oninput: (e: any) => animationSeek(Number(e.target.value)) }));
  }

  buildShell() {
    theatreRequireSaved();
    const view = $('view-animation');
    view.replaceChildren();
    const assets = el('aside', { id: 'motion-asset-browser', 'aria-label': 'Animation assets' });
    const inspector = el('aside', { id: 'motion-inspector', 'aria-label': 'Animation controls' });
    const stage = el('div', { class: 'motion-stage', id: 'motion-stage' }, el('canvas', { id: 'motion-canvas', 'aria-label': 'Character animation canvas', tabindex: 0 }));
    const toolbar = this.buildStageToolbar();
    motion.stageHeader = toolbar;
    const transport = this.buildTransportBar();
    const partBar = el('div', { class: 'motion-part-bar', id: 'motion-part-bar', hidden: '' });
    view.append(assets, el('div', { class: 'motion-main' }, toolbar.element, stage, transport, partBar), inspector);
    if (motion.mode !== 'composite') {
      const theatre = el('section', { class: 'motion-theatre', 'aria-label': 'Theatre timeline' });
      view.append(theatre);
      theatreMount(theatre);
    }
    motion.resizeObserver?.disconnect();
    motion.resizeObserver = new ResizeObserver(() => animationDraw());
    motion.resizeObserver.observe(stage);
    $('motion-canvas').addEventListener('click', () => { if (!motion.pen) $('motion-layers')?.setAttribute('open', ''); });
    setupPathPen($('motion-canvas'));
    $('motion-canvas').addEventListener('keydown', (e: KeyboardEvent) => { if (e.key === ' ') { e.preventDefault(); animationPlay(); } });
  }

  loadImages(meta: any): Promise<HTMLImageElement[]> {
    return Promise.all(meta.layers.map((l: any) => new Promise<HTMLImageElement>((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error(`Could not load ${l.label || l.id}`));
      img.src = motionAssetUrl(l.file, l.image_sha256);
    })));
  }

  async applyMetadata(meta: any) {
    const images = await this.loadImages(meta);
    Object.assign(motion, { meta, images, sample: null });
  }

  async load(opts: any = {}) {
    const refresh = Boolean(opts.refresh);
    animationPause();
    const request = ++motion.request, project = state.project, character = motion.character, resume = motion.resume;
    motion.resume = null;
    Object.assign(motion, { loading: true, loadError: null, meta: null, sample: null, images: [], photoDrafts: {} });
    $('motion-inspector')?.replaceChildren(el('p', { class: 'muted' }, 'Loading artwork…'));
    this.renderAssets();
    updateMagicSelection();
    this.renderEmpty('Loading artwork…', '', true);
    if (typeof timelineRender === 'function') timelineRender();
    try {
      const meta = await motionCommand('motion.tracks', { character });
      if (request !== motion.request || project !== state.project) return;
      const images = await this.loadImages(meta);
      if (request !== motion.request || project !== state.project) return;
      Object.assign(motion, { meta, images, loading: false });
      applyLoadedTrackSelection(meta, resume, refresh);
      $('motion-stage-status')?.remove();
      const canvas = $('motion-canvas');
      if (canvas) canvas.hidden = false;
      this.renderControls();
      await this.sample();
      this.refreshPartDrafts();
    } catch (error) {
      if (request === motion.request && project === state.project) handleAnimationLoadError(character, error);
    } finally {
      if (request === motion.request) motion.loading = false;
    }
  }

  renderAssets() {
    const picker = el('select', {
      id: 'motion-object', 'aria-label': 'Artwork', title: 'Choose artwork to preview or animate',
      onchange: async (e: any) => {
        if (!theatreCanLeave()) { e.target.value = motion.character; return; }
        motion.character = e.target.value;
        motion.selected = null;
        updateMagicSelection();
        await this.load();
      },
    }, (motion.assets || []).map((a: any) => el('option', { value: a.id }, a.name || a.id)));
    picker.value = motion.character;
    const removeCharacterBtn = motionButton('motion-remove-asset', `Delete ${motion.character || 'asset'} from project and try again`, 'trash', async () => {
      if (!motion.character || !theatreCanLeave()) return;
      const target = motion.character;
      try {
        await designCommand('project.remove_asset', { id: target });
        await this.refresh();
        toast(`Removed ${target} · ready to re-import or rebuild`);
      } catch (err) { toast(String(err)); }
    });
    removeCharacterBtn.setAttribute('data-commands', 'project.remove_asset');
    const rebuildRigBtn = el('button', {
      id: 'motion-rebuild-rig', class: 'ghost', 'data-commands': 'assets.sheet_plan',
      title: 'Review source sheets and prepare artwork',
      onclick: () => reviewAssetSheets(motion.character, ['skeleton']),
    }, 'Prepare artwork…');
    const artworkDot = window.companionBoxes.dot(motion.loading ? 'busy' : motion.loadError ? 'needs-attention' : 'ready');
    artworkDot.title = motion.loading ? 'Loading artwork' : motion.loadError || 'Artwork loaded';
    $('motion-asset-browser').replaceChildren(
      el('div', { class: 'panel-head inline' }, artworkDot, picker, removeCharacterBtn),
      rebuildRigBtn,
      motionSection('Assets', projectAssetGrid(motion.assets || [], { active: motion.character }), true),
    );
  }

  buildTrackSelector(meta: any) {
    const isStandardOrRest = () => motion.rest || meta.tracks.find((t: any) => t.id === motion.track)?.standard;
    const tracks = el('select', {
      id: 'motion-track', 'aria-label': 'Motion track', title: 'Choose a motion track',
      onchange: async (e: any) => {
        if (!e.target.value) return;
        if (!theatreCanLeave()) { e.target.value = isStandardOrRest() ? '' : motion.track; return; }
        animationPause();
        motion.track = e.target.value;
        motion.previewLoop = null;
        acFeedback.report(null, null, { stage: 'clip_switched', clip: motion.track });
        motion.rest = false;
        motion.time = 0;
        const selected = meta.tracks.find((t: any) => t.id === motion.track);
        motion.points = [];
        motion.pathLayer = null;
        if (selected?.path?.points) {
          motion.points = selected.path.points.map((p: number[]) => [...p]);
          motion.pathLayer = selected.path.layer;
          motion.pathDuration = selected.duration;
          motion.startSize = selected.path.start_size;
          motion.endSize = selected.path.end_size;
        }
        this.renderControls();
        await this.sample();
      },
    },
    el('option', { value: '' }, 'Choose a custom track…'),
    meta.tracks.filter((t: any) => !t.standard).map((t: any) => el('option', { value: t.id }, t.label || t.id.replaceAll('-', ' '))));
    tracks.value = isStandardOrRest() ? '' : motion.track;
    return tracks;
  }

  async pickActionTrack(action: string, overrides: any = {}, play = true) {
    if (!theatreCanLeave()) return;
    animationPause();
    try {
      const updated = await motionCommand('motion.action_track', { character: motion.character, action, ...overrides });
      await this.applyMetadata(updated);
      motion.track = updated.track;
      motion.rest = updated.rest;
      motion.time = 0;
      motion.previewLoop = null;
      acFeedback.report(null, null, { stage: 'clip_switched', clip: updated.track });
      this.renderControls();
      await this.sample();
      if (play) await animationPlay();
    } catch (err) { toast(String(err)); }
  }

  buildActionsGrid(meta: any) {
    return el('div', { class: 'motion-actions-grid' },
      meta.actions.map((t: any) => {
        const key = t.action;
        return el('button', {
          id: `motion-action-${key}`, class: 'motion-action', 'aria-label': t.label,
          html: `<svg viewBox="0 0 24 24" aria-hidden="true">${MOTION_ICONS[key] || MOTION_ICONS.play}</svg><span class="motion-action-word">${t.label}</span>`,
          'aria-pressed': String(!motion.rest && motion.track === t.track),
          title: `${t.label} · ${t.standard ? 'scripted motion' : 'single character animation'}`,
          onclick: () => this.pickActionTrack(key),
        });
      }),
      el('button', {
        id: 'motion-action-rest', class: 'motion-action', 'aria-label': 'Rest pose',
        html: `<svg viewBox="0 0 24 24" aria-hidden="true">${MOTION_ICONS.rest}</svg><span class="motion-action-word">Rest</span>`,
        'aria-pressed': String(motion.rest), title: 'Rest pose',
        onclick: () => this.pickActionTrack('rest', { track: motion.track, rest: true }, false),
      }));
  }

  buildLayersList(meta: any) {
    const layers = el('div', { class: 'motion-layers' });
    for (const l of [...meta.layers].reverse()) {
      const shown = l.state?.visible !== false;
      const row = el('div', { class: `motion-layer${motion.selected === l.id ? ' selected' : ''}`, id: `motion-layer-${l.id}`, title: l.id });
      const visBtn = meta.rig_sha256 ? [motionButton(`motion-visible-${l.id}`, `${shown ? 'Hide' : 'Show'} ${l.label}`, 'eye', async () => {
        await motionCommand('part.set_state', { expected_project: state.project, character: motion.character, part: l.id, visible: !shown });
        await this.refresh();
      })] : [];
      row.append(
        el('img', { src: motionAssetUrl(l.file, l.image_sha256), alt: '' }),
        el('button', {
          class: 'motion-layer-name', 'aria-pressed': String(motion.selected === l.id),
          'aria-label': `Select part ${l.label} (${l.id})`,
          onclick: () => { motion.selected = l.id; this.renderControls(); animationDraw(); updateMagicSelection(); },
        }, l.label),
        ...visBtn,
      );
      if (meta.rig_sha256) row.lastChild.setAttribute('aria-pressed', String(shown));
      if (!shown) row.classList.add('hidden-layer');
      layers.append(row);
    }
    return layers;
  }

  buildSelectedPartCompare(meta: any, selected: any, candidate: any) {
    const name = selected.label || selected.id, sourceSize = selected.source_size || [];
    const size = (w: number, h: number) => (w && h ? ` · ${w}×${h}` : '');
    const dimensionsMatch = Boolean(candidate && candidate.result.width === sourceSize[0] && candidate.result.height === sourceSize[1]);
    const grid = el('div', { class: 'magic-compare-grid' },
      el('figure', { class: 'magic-compare-frame' },
        el('img', { src: motionAssetUrl(selected.file, selected.image_sha256), alt: `Original ${name}` }),
        el('figcaption', {}, `Original${size(sourceSize[0], sourceSize[1])}`)),
      el('figure', { class: 'magic-compare-frame' },
        candidate ? el('img', { src: candidateUrl(candidate), alt: `Candidate for ${name}` }) : el('div', { class: 'panel-empty compare' }, 'No candidate yet'),
        el('figcaption', {}, candidate ? `${candidate.source === 'photocraft' ? 'PhotoCraft draft' : 'Candidate'}${size(candidate.result.width, candidate.result.height)}` : 'Candidate')));
    const statusText = dimensionsMatch
      ? 'Sizes match · the original stays until you replace it'
      : candidate
        ? `Cannot replace: candidate is ${candidate.result.width}×${candidate.result.height}; ${name} needs ${sourceSize[0] || '?'}×${sourceSize[1] || '?'}`
        : 'Generate a candidate or save an edit in PhotoCraft';
    const actionStatus = el('p', { class: 'panel-status tight', id: 'part-image-status', title: statusText, role: 'status' }, statusText);
    const undoKey = `magic.part-image-undo.${state.project}.${motion.character}.${selected.id}`;
    const savedUndo = localStorage.getItem(undoKey);
    const replaceButton = el('button', {
      class: 'start', id: 'part-image-replace', 'aria-label': `Replace ${name} with reviewed candidate`,
      title: !dimensionsMatch ? 'Candidate must have the same pixel dimensions as this part' : 'Replace only this captured part; motion and pivot are preserved',
      onclick: async () => {
        replaceButton.disabled = true;
        actionStatus.textContent = 'Replacing selected part…';
        try {
          const result = await motionCommand('part.replace_image', {
            asset: candidate.assetId, part: candidate.partId, candidate: candidate.image,
            expected_rig_sha256: candidate.rigSha256, expected_part_sha256: candidate.partSha256,
            expected_candidate_sha256: candidate.result.sha256, expected_revision: candidate.revision,
          });
          localStorage.setItem(undoKey, result.undo);
          actionStatus.textContent = 'Part replaced · Undo restores the previous image';
          await this.refresh();
        } catch (error) { actionStatus.textContent = String(error); replaceButton.disabled = false; }
      },
    }, 'Replace');
    replaceButton.disabled = !dimensionsMatch || !candidate.rigSha256 || !candidate.partSha256 || !Number.isSafeInteger(candidate.revision) || candidate.revision !== meta.image_revision;
    const undoButton = el('button', {
      class: 'ghost', id: 'part-image-undo', 'aria-label': `Undo image replacement for ${name}`,
      title: 'Undo the latest image replacement',
      onclick: async () => {
        undoButton.disabled = true;
        actionStatus.textContent = 'Restoring original part image…';
        try {
          await motionCommand('part.undo_image', { asset: motion.character, undo: savedUndo });
          localStorage.removeItem(undoKey);
          actionStatus.textContent = 'Original part image restored';
          await this.refresh();
        } catch (error) { actionStatus.textContent = String(error); undoButton.disabled = false; }
      },
    }, 'Undo');
    undoButton.disabled = !savedUndo;
    const bar = el('div', { class: 'magic-part-actionbar' },
      partImageEditorButton(selected, 'part-open-photocraft'),
      replaceButton, undoButton,
      el('button', { class: 'ghost', id: 'part-inspect-original', title: selected.file || '', 'aria-label': `Inspect original ${name}`, onclick: () => selected.file && openPath(selected.file) }, 'Original'),
      el('button', {
        class: 'ghost', id: 'part-copy-path', title: 'Copy file path to clipboard', 'aria-label': `Copy asset path for ${name}`,
        onclick: () => { if (selected.file && navigator.clipboard) navigator.clipboard.writeText(selected.file).then(() => toast('Copied asset path')).catch(() => {}); },
      }, 'Copy path'));
    return [grid, actionStatus, bar];
  }

  buildPartComparePanel(meta: any, selected: any, candidate: any) {
    const partsStrip = el('div', { class: 'magic-parts-strip', 'aria-label': 'Parts strip' },
      ...meta.layers.map((part: any) => el('button', {
        class: `magic-part-thumb${motion.selected === part.id ? ' active' : ''}`,
        id: `magic-thumb-${part.id}`, title: `Select ${part.label || part.id} (${part.id})`,
        onclick: () => { motion.selected = part.id; this.renderControls(); animationDraw(); updateMagicSelection(); },
      }, el('img', { src: motionAssetUrl(part.file, part.image_sha256), alt: '' }), el('span', {}, part.id))));
    const compare = el('div', { class: 'magic-part-compare', id: 'magic-part-compare' }, partsStrip);
    if (!selected) compare.append(el('p', { class: 'why' }, 'Select a part to compare its original with a candidate.'));
    else compare.append(...this.buildSelectedPartCompare(meta, selected, candidate));
    return compare;
  }

  renderControls() {
    const box = $('motion-inspector');
    box.replaceChildren();
    const meta = motion.meta;
    const authoring = motionAuthoringPanels(meta);
    this.renderAssets();
    box.append(authoring.status);
    updateMagicSelection();
    const tracks = this.buildTrackSelector(meta);
    const actions = this.buildActionsGrid(meta);
    const genMissingPosesBtn = el('button', {
      id: 'btn-generate-missing-poses', class: 'ghost', 'data-commands': 'assets.sheet_plan',
      title: 'Review and create whole pose sheets', onclick: () => reviewAssetSheets(motion.character, ["poses"]),
    }, 'Pose sheets…');
    const charMagicPanel = renderCharacterMagicPanel(meta, (act: string, ov?: any, pl?: boolean) => this.pickActionTrack(act, ov, pl));
    box.append(motionSection('Action', [actions, charMagicPanel, genMissingPosesBtn, el('span', { class: 'group-label' }, 'Custom tracks'), tracks, authoring.action], true));
    box.append(pathControls());
    const loop = el('input', {
      id: 'motion-preview-loop', type: 'checkbox',
      onchange: (e: any) => { motion.previewLoop = e.target.checked; e.target.defaultChecked = e.target.checked; animationRemember(); },
    });
    loop.checked = motion.previewLoop ?? meta.tracks.find((t: any) => t.id === motion.track)?.loop ?? false;
    loop.defaultChecked = loop.checked;
    box.append(motionSection('Motion & timing', [
      el('label', {}, loop, 'Loop preview'),
      motionRange('motion-speed', 'Playback speed', 0.25, 2.25, 0.05, motion.speed, '×', (v) => { motion.speed = v; animationRemember(); }),
      motionRange('motion-amount', 'Motion amount', 0, 100, 1, motion.amount, '%', (v) => {
        motion.amount = v;
        animationRemember();
        clearTimeout(motion.amountTimer);
        motion.amountTimer = setTimeout(() => this.sample(), 140);
      }),
    ], false));
    const layers = this.buildLayersList(meta);
    const section = motionSection('Layers', authoring.layers, true);
    section.id = 'motion-layers';
    const selected = meta.layers.find((part: any) => part.id === motion.selected);
    const candidate = selected ? partCandidate(selected.id) : null;
    const compare = this.buildPartComparePanel(meta, selected, candidate);
    const genLimbSheetBtn = el('button', {
      id: 'btn-generate-limb-sheet', class: 'ghost', 'data-commands': 'assets.sheet_plan',
      title: 'Review complete part sheets before extracting individual pieces',
      onclick: () => reviewAssetSheets(motion.character, ["skeleton"]),
    }, 'Part sheets…');
    const genTurnaroundBtn = el('button', {
      id: 'btn-generate-turnaround-sheet', class: 'ghost', 'data-commands': 'assets.sheet_plan',
      title: 'Review a four-view turnaround sheet and its drawing prompt',
      onclick: () => reviewAssetSheets(motion.character, ["turnaround"]),
    }, 'Turnaround sheets…');
    authoring.layers.append(layers);
    box.append(section, motionSection('Part artwork', el('div', { id: 'magic-parts-slot' }, genLimbSheetBtn, ...(meta.rig_sha256 ? [partsDrawer(compare)] : [])), false, 'magic-parts-pane'));
    const controls = meta.controls?.joints || [];
    if (meta.rig_sha256) {
      box.append(motionSection('Expression & pose', [
        el('div', { class: 'anim-part-inspector', id: 'anim-selected-part-inspector', 'aria-label': 'Selected part controls' }),
        genTurnaroundBtn,
        el('p', { class: 'muted' }, controls.length ? 'This rig exposes joint bindings.' : 'Standard actions move the whole object. Custom tracks add detailed acting to its layers.'),
      ], false));
    }
    const pf = meta.preflight || {};
    const pfSummary = pf.total_checks
      ? `${pf.passed_count}/${pf.total_checks} pre-flight checks passed · ${pf.sheets?.turnaround_views || 0} turnaround views · ${pf.sheets?.pose_frames || 0} pose frames`
      : `${meta.layers.length} layers · ${meta.tracks.length} motion tracks`;
    const pfList = el('div', { class: 'why' }, ...(pf.checks || []).map((c: any) => el('div', { class: c.ok ? 'muted' : 'why' }, `${c.ok ? '✓' : '✗'} ${c.label}`)));
    box.append(motionSection('Quality checks', [el('p', { class: 'muted' }, pfSummary), pfList], false));
    const { button: exportButton, status: exportStatus } = motionExportControl();
    box.append(motionSection('Export & integrations', [exportButton, exportStatus, authoring.pose], false));
    const authoredCount = meta.tracks.filter((t: any) => !t.standard).length;
    box.append(el('div', { class: 'motion-source-context', id: 'motion-source-context' },
      el('strong', {}, `${motion.assets?.find((a: any) => a.id === motion.character)?.name || motion.character} · Subject / shared engine · ${meta.layers.length} layers / ${authoredCount} authored clips`),
      el('span', { class: 'muted' }, selected ? `Selected: ${selected.label || selected.id}` : 'Select a part in Layers or Parts & assets')));
    if (meta.rig_sha256) renderPartControlsBar(selected, authoring.part);
    if (typeof theatreSyncClips === 'function') theatreSyncClips();
    animationRemember();
  }

  async refreshPartDrafts() {
    const character = motion.character, found: Record<string, any> = {};
    for (const part of motion.meta?.layers || []) {
      let result;
      try { result = await motionCommand('part.edit_image', { asset: character, part: part.id }); } catch { break; }
      if (result.draft) found[part.id] = { ...result.draft, current_sha256: result.sha256 };
    }
    if (motion.character !== character || JSON.stringify(found) === JSON.stringify(motion.photoDrafts)) return;
    motion.photoDrafts = found;
    this.renderControls();
  }

  async sample() {
    if (!motion.meta?.tracks?.length) return;
    const request = ++motion.request;
    animationPause();
    motion.stageHeader.rename('Sampling motion…');
    try {
      const sample = await motionCommand('motion.sample', {
        character: motion.character,
        track: motion.track || motion.meta.tracks[0].id,
        rest: motion.rest,
        amount: motion.amount,
      });
      if (request !== motion.request) return;
      if (sample.layer_ids && (sample.layer_ids.length !== motion.meta.layers.length || sample.layer_ids.some((id: string, i: number) => id !== motion.meta.layers[i].id))) {
        throw Error('Layer order changed during sampling; refresh the object before playback.');
      }
      motion.sample = sample;
      if (typeof timelineRender === 'function') timelineRender();
      motion.time = Math.min(motion.time, sample.duration);
      $('motion-scrub').max = sample.duration;
      animationDraw();
    } catch (e) { toast(String(e)); }
  }

  async refresh() {
    if (state.view !== 'animation') return;
    if (motion.mode === 'composite') { seqRenderInspector(); seqDraw(); return; }
    if (motion.mode === 'blender') { await openAnimationJobs(); return; }
    const project = state.project, request = ++motion.request;
    animationPause();
    const catalog = await motionCommand('project.list_assets');
    if (state.project !== project || request !== motion.request) return;
    motion.assets = catalog.assets || catalog.project_assets || [];
    const character = selectRefreshedCharacter(catalog, motion.assets);
    if (!character) return handleEmptyAnimationCatalog();
    if (character !== motion.character) {
      theatreRequireSaved();
      motion.selected = null;
      motion.time = 0;
      motion.rest = true;
    }
    motion.character = character;
    await this.load({ refresh: true });
  }
}

const animationView = new AnimationViewController();

async function openAnimation(character: string) { return animationView.openCharacter(character); }
async function animationOpen() { return animationView.open(); }
function animationEmpty(title: string, detail: string, loading = false) { animationView.renderEmpty(title, detail, loading); }
function animationShell() { animationView.buildShell(); }
function animationLoadImages(meta: any) { return animationView.loadImages(meta); }
async function animationApplyMetadata(meta: any) { return animationView.applyMetadata(meta); }
function applyLoadedTrackSelection(meta: any, resume: any, refresh: boolean) {
  if (!meta.layers?.some((part: any) => part.id === motion.selected)) motion.selected = null;
  if (refresh && meta.configured_action) {
    motion.track = meta.configured_action.track;
    motion.rest = meta.configured_action.rest;
    return;
  }
  const tracks = meta.tracks || [];
  if (resume && tracks.some((t: any) => t.id === resume.track)) motion.track = resume.track;
  else if (!tracks.some((t: any) => t.id === motion.track)) motion.track = tracks[0]?.id || '';
  if (!refresh) { motion.time = resume?.time || 0; motion.rest = resume?.rest ?? true; }
}
function handleAnimationLoadError(character: any, error: any) {
  Object.assign(motion, { loading: false, loadError: String(error), meta: null, sample: null, images: [] });
  renderAnimationAssets();
  updateMagicSelection();
  $('motion-inspector')?.replaceChildren(el('p', { class: 'muted' }, 'Choose another asset or review the complete source artwork.'));
  motion.stageHeader?.rename(character, 'Source artwork is unavailable');
  animationEmpty('Source artwork is unavailable', 'Check its source in Project or choose another asset.');
}
async function animationLoad(opts: any = {}) { return animationView.load(opts); }
function partBadge(stateName: string, title: string) {
  const node = window.companionBoxes.dot(stateName);
  node.classList.add('sm');
  node.title = title;
  return node;
}
function renderAnimationAssets() { animationView.renderAssets(); }
function animationControls() { animationView.renderControls(); }
function motionExportControl() {
  const status = el('div', { id: 'motion-export-status', class: 'why', role: 'status' });
  let busy = false;
  const button = el('button', {
    class: 'start', id: 'motion-export', 'data-commands': 'animation.render',
    title: 'Export the current rest/action preview as a transparent track',
    onclick: async () => {
      if (busy) return;
      const project = state.project, character = motion.character;
      try {
        theatreRequireSaved();
        const duration = motion.sample?.duration / motion.speed;
        if (!character || !motion.track || !Number.isFinite(duration) || duration <= 0) {
          throw Error('Choose a sampled action before exporting.');
        }
        const params = { character, track: motion.track, rest: motion.rest, amount: motion.amount, duration };
        busy = true;
        button.disabled = true;
        status.textContent = 'Rendering transparent track…';
        const result = await motionCommand('animation.render', params);
        motion.lastExport = { project, character, params, result };
        if (state.project !== project || motion.character !== character) return;
        const file = result.outputs?.[0]?.file;
        if (result.ok !== true || typeof file !== 'string' || !file) {
          throw Error(result.error || 'Export returned no confirmed output file; inspect the retained receipt.');
        }
        status.textContent = 'Track ready';
        status.append(fileButton({ path: file, name: file.split('/').pop() }));
      } catch (error) {
        if (state.project === project && motion.character === character) status.textContent = String(error);
      } finally {
        busy = false;
        button.disabled = false;
      }
    },
  }, 'Export track');
  return { button, status };
}
function renderCharacterMagicPanel(meta: any, pickActionTrack: any) {
  const cfg = meta?.character_panel;
  if (!cfg.available) return el('div', { class: 'char-magic-panel', hidden: '' });
  const angleEntries = cfg.angles;
  const poseEntries = cfg.poses;
  const switchAngle = async (angleId: string) => {
    if (!theatreCanLeave()) return;
    animationPause();
    try {
      const updated = await motionCommand('character.configure', { character: motion.character, angle: angleId });
      await animationApplyMetadata(updated);
      animationControls();
      await animationSample();
    } catch (err) { toast(String(err)); }
  };
  const angleCards = el('div', { class: 'char-angle-grid', 'aria-label': 'Turnaround angles' }, ...angleEntries.map((a: any) => el('button', {
    id: `char-angle-${a.id}`,
    class: `char-thumb-card${cfg.active_angle === a.id ? ' active' : ''}`,
    'aria-pressed': String(cfg.active_angle === a.id),
    title: `Turn model to ${a.label} (${a.degrees ?? 0}°) · ${a.image || ''}`,
    onclick: () => switchAngle(a.id),
  }, a.image_path ? el('img', { src: motionAssetUrl(a.image_path, meta.rig_sha256), alt: a.label }) : null, el('span', {}, a.label))));
  const poseCards = el('div', { class: 'char-pose-grid', 'aria-label': 'Generated pose & sprite frames' }, ...poseEntries.map((p: any) => {
    const active = !motion.rest && p.track_ids.includes(motion.track);
    return el('button', {
      id: `char-pose-${p.id}`,
      class: `char-thumb-card${active ? ' active' : ''}`,
      'aria-pressed': String(active),
      title: `${p.word} single animation (${p.angle || 'front'}) · ${p.image || ''}`,
      onclick: () => pickActionTrack(p.id),
    }, p.image_path ? el('img', { src: motionAssetUrl(p.image_path, meta.rig_sha256), alt: p.word }) : null, el('span', {}, p.word));
  }));
  const sheetsRow = el('div', { class: 'char-sheets-row' });
  for (const [k, label] of [['turnaround', 'Turnaround Sheet'], ['poses', 'Pose Sheet'], ['skeleton', 'Limb Sheet']]) {
    const sp = cfg.sheets?.[k];
    if (sp) {
      sheetsRow.append(el('figure', { class: 'char-sheet-card', title: sp, onclick: () => reviewAssetSheets(motion.character, [k], sp) },
        el('img', { src: motionAssetUrl(sp, meta.rig_sha256), alt: label }),
        el('figcaption', {}, label)));
    }
  }
  return el('div', { class: 'char-magic-panel', id: 'char-magic-panel' },
    el('div', { class: 'char-magic-header' },
      el('strong', {}, `${cfg.name || motion.character}.character.magic`),
      el('span', { class: 'muted' }, `Angle: ${cfg.active_angle} · Dir: ${cfg.direction}`)),
    el('span', { class: 'group-label' }, 'Turnaround Angles (turn model slightly for Left / Right)'),
    angleCards,
    el('span', { class: 'group-label' }, 'Single Animation Pose Sheets (matches UI actions)'),
    poseCards,
    sheetsRow);
}
function renderPartControlsBar(selected: any, controls: any) {
  const bar = $('anim-selected-part-inspector');
  if (!bar) return;
  bar.replaceChildren(controls);
  if (!selected) return;
  const character = motion.character, project = state.project, shown = selected.state?.visible !== false;
  const set = async (values: any) => {
    try {
      await motionCommand('part.set_state', { expected_project: project, character, part: selected.id, ...values });
      await animationRefresh();
    } catch (error) { toast(String(error)); }
  };
  bar.append(
    el('p', { class: 'muted' }, `Layer ${selected.order ?? '?'} · parent ${selected.parent || 'root'} · pivot ${(selected.pivot || []).join(', ')}`),
    motionButton('motion-bar-visible', `${shown ? 'Hide' : 'Show'} ${selected.label || selected.id}`, 'eye', () => set({ visible: !shown })),
    motionButton('motion-bar-solo', `${selected.state?.solo ? 'Unsolo' : 'Solo'} ${selected.label || selected.id}`, 'layers', () => set({ solo: !selected.state?.solo })),
    el('button', {
      id: 'motion-show-all', class: 'ghost', 'data-commands': 'part.show_all',
      onclick: async () => {
        try { await motionCommand('part.show_all', { expected_project: project, character }); await animationRefresh(); }
        catch (error) { toast(String(error)); }
      },
    }, 'Show all parts'),
  );
}
function partsDrawer(compare: any) {
  const meta = motion.meta;
  const cards = meta.layers.map((part: any) => {
    const shown = part.state?.visible !== false, name = part.label || part.id, picked = motion.selected === part.id;
    const candidate = partCandidate(part.id);
    const select = () => { motion.selected = part.id; animationControls(); animationDraw(); updateMagicSelection(); };
    const badge = candidate
      ? partBadge('needs-attention', candidate.source === 'photocraft' ? 'A saved PhotoCraft draft awaits review' : 'A generated candidate awaits review')
      : picked ? partBadge('ready', 'Selected') : null;
    const visible = motionButton(`magic-part-visible-${part.id}`, `${shown ? 'Hide' : 'Show'} ${name}`, 'eye', async () => {
      await motionCommand('part.set_state', { expected_project: state.project, character: motion.character, part: part.id, visible: !shown });
      await animationRefresh();
    });
    visible.setAttribute('aria-pressed', String(shown));
    return el('article', { class: `magic-part-card${picked ? ' active' : ''}${shown ? '' : ' hidden-layer'}`, id: `magic-part-card-${part.id}`, title: `${name} (${part.id})` },
      el('button', { class: 'magic-part-card-thumb', tabindex: '-1', 'aria-hidden': 'true', onclick: select }, el('img', { src: motionAssetUrl(part.file, part.image_sha256), alt: '' })),
      el('button', { class: 'magic-part-card-pick', 'aria-pressed': String(picked), 'aria-label': `Select ${name} (${part.id})`, onclick: select },
        el('span', { class: 'magic-part-card-name' }, name),
        part.label && part.label !== part.id ? el('span', { class: 'magic-part-card-id' }, part.id) : null,
        badge),
      el('div', { class: 'magic-part-card-actions' }, visible, partImageEditorButton(part, `part-open-photocraft-${part.id}`)));
  });
  const panels: Record<string, any> = { parts: cards.length ? el('div', { class: 'magic-part-cards' }, cards) : el('p', { class: 'why' }, 'This object has no rig parts.'), compare };
  if (!Object.hasOwn(panels, motion.partsTab)) motion.partsTab = 'parts';
  const body = el('div', { class: 'magic-parts-panel', role: 'tabpanel' }, panels[motion.partsTab] || compare);
  const tab = (key: string, label: string) => el('button', {
    class: motion.partsTab === key ? 'active' : '', role: 'tab', 'aria-selected': String(motion.partsTab === key),
    id: `magic-parts-tab-${key}`, onclick: () => { motion.partsTab = key; animationControls(); },
  }, label);
  return el('div', { class: 'magic-parts-drawer' },
    el('div', { class: 'segmented', role: 'tablist', 'aria-label': 'Parts and assets' },
      tab('parts', `Parts (${meta.layers.length})`), tab('compare', 'Compare')),
    body);
}
function partImageEditorButton(part: any, id: string) {
  const capability = part.image_edit;
  const button = el('button', {
    id, class: 'ghost magic-part-card-open', 'data-commands': 'part.edit_image',
    'data-value-source': 'motion.tracks.layers.image_edit',
    title: capability?.supported ? 'Edit a draft copy in PhotoCraft; save it to use as the candidate' : capability?.reason || 'Image editing capability is unknown',
    'aria-label': `Open ${part.label || part.id} in PhotoCraft`,
    onclick: () => openPartImageEditor(part),
  }, 'PhotoCraft');
  button.disabled = capability?.supported !== true;
  return button;
}
async function openPartImageEditor(part: any) {
  if (part.image_edit?.supported !== true) { toast(part.image_edit?.reason || 'Image editing capability is unknown'); return; }
  try {
    const result = await motionCommand('part.edit_image', { asset: motion.character, part: part.id, spawn: true });
    toast(`PhotoCraft: ${(result.draft?.file || result.file).split('/').pop()} · keep it ${result.width}×${result.height} · save to make it the candidate`);
    await refreshPartDrafts();
  } catch (error) { toast(String(error)); }
}
function photoDraftCandidate(partId: string) {
  const draft = motion.photoDrafts[partId];
  if (!draft || draft.matches_current) return null;
  return {
    assetId: motion.character, partId, image: draft.file,
    result: { width: draft.width, height: draft.height, sha256: draft.sha256 },
    rigSha256: motion.meta?.rig_sha256, partSha256: draft.current_sha256,
    revision: motion.meta?.image_revision, source: 'photocraft',
  };
}
const partCandidate = (partId: string) => (typeof imageCandidateForPart === 'function' ? imageCandidateForPart(motion.character, partId) : null) || photoDraftCandidate(partId);
const candidateUrl = (candidate: any) => {
  const url = String(convertFileSrc(candidate.image));
  return `${url}${url.includes('?') ? '&' : '?'}v=${candidate.result?.sha256 || ''}`;
};
async function refreshPartDrafts() { return animationView.refreshPartDrafts(); }
window.addEventListener('focus', () => { if (state.view === 'animation' && motion.mode !== 'composite' && motion.meta) refreshPartDrafts(); });
async function animationSample() { return animationView.sample(); }
function selectRefreshedCharacter(catalog: any, assets: any[]) {
  if (catalog.active_asset && assets.some((a) => a.id === catalog.active_asset)) return catalog.active_asset;
  if (assets.some((a) => a.id === motion.character)) return motion.character;
  return assets[0]?.id || null;
}
function handleEmptyAnimationCatalog() {
  Object.assign(motion, { character: null, selected: null, track: '', meta: null, sample: null, images: [] });
  $('motion-inspector')?.replaceChildren();
  animationEmpty('Bring in your artwork', 'Choose an object from the Storyboard view.');
  if (typeof timelineRender === 'function') timelineRender();
}
async function animationRefresh() { return animationView.refresh(); }

class AnimationStageController {
  seek(time: number) {
    if (!Number.isFinite(time)) return;
    this.pause();
    motion.time = Math.max(0, Math.min(motion.sample?.duration || 0, time));
    this.draw();
    animationRemember();
  }

  step(direction: number) {
    this.seek(motion.time + direction / (motion.sample?.fps || 30));
  }

  pause() {
    if (motion.mode === 'composite') { seqPause(); return; }
    const wasPlaying = motion.playing;
    motion.playing = false;
    motion.playId = (motion.playId || 0) + 1;
    if (typeof cancelAnimationFrame === 'function' && motion.tick) {
      cancelAnimationFrame(motion.tick);
      motion.tick = 0;
    }
    const btn = $('motion-play');
    if (btn) {
      btn.innerHTML = `<svg viewBox="0 0 24 24">${MOTION_ICONS.play}</svg>`;
      btn.title = 'Play motion';
      btn.setAttribute('aria-label', 'Play motion');
    }
    if (wasPlaying) animationRemember();
    if (wasPlaying && typeof theatreSeek === 'function' && motion.sample) theatreSeek(motion.time);
  }

  async play() {
    if (motion.mode === 'composite') { seqPlay(); return; }
    if (motion.playing) { this.pause(); return; }
    if (motion.rest) {
      motion.rest = false;
      motion.track = motion.track || motion.meta?.tracks?.[0]?.id;
      animationControls();
      await animationSample();
    }
    if (!motion.sample) return;
    if (motion.time >= motion.sample.duration) motion.time = 0;
    const runId = (motion.playId = (motion.playId || 0) + 1);
    motion.playing = true;
    const btn = $('motion-play');
    if (btn) {
      btn.innerHTML = `<svg viewBox="0 0 24 24">${MOTION_ICONS.pause}</svg>`;
      btn.title = 'Pause motion';
      btn.setAttribute('aria-label', 'Pause motion');
    }
    let last = performance.now();
    const tick = (now: number) => {
      if (!motion.playing || motion.playId !== runId) return;
      motion.time += (now - last) / 1000 * motion.speed;
      last = now;
      if (motion.time >= motion.sample.duration) {
        if (motion.previewLoop ?? motion.sample.loop) motion.time %= motion.sample.duration;
        else { motion.time = motion.sample.duration; this.pause(); }
      }
      this.draw();
      if (motion.playing && motion.playId === runId) motion.tick = requestAnimationFrame(tick);
    };
    motion.tick = requestAnimationFrame(tick);
  }

  drawGrid(ctx: CanvasRenderingContext2D, b: number[], w: number, h: number, scale: number) {
    if (!motion.grid) return;
    const gap = 50;
    ctx.save();
    ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--line');
    ctx.lineWidth = 1 / scale;
    ctx.beginPath();
    const left = (b[0] + b[2]) / 2 - w / (2 * scale);
    const top = (b[1] + b[3]) / 2 - h / (2 * scale);
    for (let x = Math.floor(left / gap) * gap; x < left + w / scale; x += gap) {
      ctx.moveTo(x, top);
      ctx.lineTo(x, top + h / scale);
    }
    for (let y = Math.floor(top / gap) * gap; y < top + h / scale; y += gap) {
      ctx.moveTo(left, y);
      ctx.lineTo(left + w / scale, y);
    }
    ctx.stroke();
    ctx.restore();
  }

  drawFrameLayers(ctx: CanvasRenderingContext2D, frame: any, meta: any) {
    frame.layers.forEach((l: any, i: number) => {
      if (!l.opacity) return;
      ctx.save();
      ctx.transform(...(l.matrix as [number, number, number, number, number, number]));
      theatreTransform(ctx, meta.layers[i]);
      ctx.globalAlpha = l.opacity;
      const crop = meta.layers[i].crop;
      ctx.drawImage(motion.images[i], crop[0], crop[1]);
      ctx.restore();
    });
  }

  syncReadouts(meta: any) {
    const trackLabel = motion.rest
      ? 'Rest pose'
      : (meta.tracks.find((t: any) => t.id === motion.track)?.label || motion.track.replaceAll('-', ' '));
    motion.stageHeader.rename(`${motion.character} · ${trackLabel}`);
    $('motion-time').textContent = `${Math.floor(motion.time / 60)}:${(motion.time % 60).toFixed(3).padStart(6, '0')}`;
    $('motion-scrub').value = motion.time;
    const angleInput = $('motion-part-angle');
    if (angleInput && document.activeElement !== angleInput) {
      const deg = Math.round((magicTheatre.rotation || 0) * 10) / 10;
      angleInput.value = String(deg);
      if ($('motion-part-angle-out')) $('motion-part-angle-out').textContent = `${deg}°`;
    }
    if (!motion.playing && !motion.loading) animationRemember();
    if (!motion.playing || magicTheatre.interactionRevision > magicTheatre.savedRevision) theatreSeek(motion.time);
    if (typeof timelineSeek === 'function') timelineSeek(motion.time);
  }

  draw() {
    if (motion.mode === 'composite') { seqDraw(); return; }
    const canvas = $('motion-canvas'), sample = motion.sample, meta = motion.meta;
    if (!canvas || !sample || !meta) return;
    const rect = canvas.parentElement.getBoundingClientRect();
    if (!rect.width || !rect.height) return;
    const dpr = devicePixelRatio || 1, w = rect.width, h = rect.height;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    const ctx = canvas.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const b = sample.bounds || meta.bounds;
    const scale = (motion.fit ? Math.min((w - 64) / (b[2] - b[0]), (h - 64) / (b[3] - b[1])) : 1) * motion.zoom;
    motion.viewTransform = { scale, cx: (b[0] + b[2]) / 2, cy: (b[1] + b[3]) / 2, w, h };
    ctx.translate(w / 2, h / 2);
    ctx.scale(scale, scale);
    ctx.translate(-(b[0] + b[2]) / 2, -(b[1] + b[3]) / 2);
    this.drawGrid(ctx, b, w, h, scale);
    const frame = sample.frames[Math.min(sample.frames.length - 1, Math.floor(motion.time * sample.fps))];
    this.drawFrameLayers(ctx, frame, meta);
    drawMotionPath(ctx, scale);
    this.syncReadouts(meta);
  }
}

class MotionPathController {
  buildControls() {
    const apply = el('button', { id: 'motion-path-apply', class: 'start', onclick: () => this.apply() }, 'Apply path');
    apply.disabled = motion.points.length < 2;
    const undo = motionButton('motion-path-undo', 'Undo the last drawn path', 'reset', async () => {
      if (!motion.pathUndo) return;
      try {
        await motionCommand('motion.undo_path', { character: motion.character, undo: motion.pathUndo });
        motion.pathUndo = null;
        motion.points = [];
        await animationLoad();
      } catch (e) { toast(String(e)); }
    });
    undo.disabled = !motion.pathUndo;
    const roots = (motion.meta?.layers || []).filter((l: any) => !l.parent);
    if (!roots.some((l: any) => l.id === motion.pathLayer)) motion.pathLayer = roots[0]?.id || null;
    const target = el('select', {
      id: 'motion-path-layer', 'aria-label': 'Root layer moved by path',
      onchange: () => { motion.pathLayer = target.value; },
    }, ...roots.map((l: any) => el('option', { value: l.id }, l.label || l.id)));
    target.value = motion.pathLayer || '';
    const point = el('select', { id: 'motion-path-point', 'aria-label': 'Edit path point' },
      ...motion.points.map((p: number[], i: number) => el('option', { value: String(i) }, `${i + 1} · ${p.map((v) => Number(v.toFixed(1))).join(', ')}`)));
    const x = el('input', { id: 'motion-path-x', type: 'number', step: 'any', 'aria-label': 'Path point X' });
    const y = el('input', { id: 'motion-path-y', type: 'number', step: 'any', 'aria-label': 'Path point Y' });
    const read = () => {
      const p = motion.points[Number(point.value)];
      x.disabled = y.disabled = !p;
      if (p) { x.value = p[0]; y.value = p[1]; }
    };
    point.onchange = read;
    for (const [input, index] of [[x, 0], [y, 1]] as [any, number][]) {
      input.onchange = () => {
        const p = motion.points[Number(point.value)], value = Number(input.value);
        if (p && String(input.value).trim() && Number.isFinite(value)) {
          p[index] = value;
          animationDraw();
        } else read();
      };
    }
    read();
    const clearBtn = motionButton('motion-path-clear', 'Clear the drawn line', 'reset', () => {
      motion.points = [];
      $('motion-path-apply').disabled = true;
      animationDraw();
    });
    const section = motionSection('Path & depth', [
      el('label', {}, 'Path root', target),
      el('label', {}, 'Edit point', point),
      el('div', { class: 'motion-coordinate-row' }, el('label', {}, 'X', x), el('label', {}, 'Y', y)),
      el('p', { class: 'motion-help', title: 'Select Pen above the stage, then draw. Playback moves from the first point at zero to the last point at the chosen duration.' }, 'Draw a route. Set its timing.'),
      motionRange('motion-path-duration', 'Duration', 0.1, Math.max(600, motion.pathDuration), 0.1, motion.pathDuration, 's', (v) => { motion.pathDuration = v; animationDraw(); }),
      motionRange('motion-size-start', 'Start size', 1, Math.max(1000, motion.startSize), 1, motion.startSize, '%', (v) => { motion.startSize = v; }),
      motionRange('motion-size-end', 'End size', 1, Math.max(1000, motion.endSize), 1, motion.endSize, '%', (v) => { motion.endSize = v; }),
      el('div', { class: 'motion-path-actions' }, clearBtn, undo, apply),
    ], false);
    section.id = 'motion-path-section';
    return section;
  }

  setupPen(canvas: any) {
    let drawing = false;
    const point = (e: PointerEvent) => {
      const v = motion.viewTransform;
      if (!v) return;
      const r = canvas.getBoundingClientRect();
      const p = [(e.clientX - r.left - v.w / 2) / v.scale + v.cx, (e.clientY - r.top - v.h / 2) / v.scale + v.cy];
      const last = motion.points.at(-1);
      if (!last || Math.hypot(p[0] - last[0], p[1] - last[1]) * v.scale > 4) {
        if (motion.points.length < 512) motion.points.push(p);
        $('motion-path-apply').disabled = motion.points.length < 2;
        animationDraw();
      }
    };
    canvas.addEventListener('pointerdown', (e: PointerEvent) => {
      if (!motion.pen) return;
      e.preventDefault();
      canvas.setPointerCapture(e.pointerId);
      drawing = true;
      point(e);
    });
    canvas.addEventListener('pointermove', (e: PointerEvent) => { if (drawing) point(e); });
    canvas.addEventListener('pointerup', (e: PointerEvent) => {
      if (!drawing) return;
      point(e);
      drawing = false;
      const section = $('motion-path-section');
      if (section) section.replaceWith(this.buildControls());
    });
    canvas.addEventListener('pointercancel', () => { drawing = false; });
  }

  draw(ctx: CanvasRenderingContext2D, scale: number) {
    if (!motion.pathVisible || !motion.points.length) return;
    ctx.save();
    ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--accent');
    ctx.fillStyle = ctx.strokeStyle;
    ctx.lineWidth = 1.5 / scale;
    ctx.beginPath();
    motion.points.forEach((p: number[], i: number) => (i ? ctx.lineTo(p[0], p[1]) : ctx.moveTo(p[0], p[1])));
    ctx.stroke();
    for (const i of [0, motion.points.length - 1]) {
      const p = motion.points[i];
      ctx.beginPath();
      ctx.arc(p[0], p[1], 4 / scale, 0, Math.PI * 2);
      ctx.fill();
      ctx.font = `${12 / scale}px system-ui`;
      ctx.fillText(i === 0 ? '0' : `${motion.pathDuration}s`, p[0] + 8 / scale, p[1] - 8 / scale);
    }
    ctx.restore();
  }

  async apply() {
    if (!theatreCanLeave()) return;
    animationPause();
    const button = $('motion-path-apply');
    button.disabled = true;
    try {
      const result = await motionCommand('motion.path', {
        character: motion.character,
        track: motion.track || motion.meta.tracks[0].id,
        layer: motion.pathLayer,
        points: motion.points,
        duration: motion.pathDuration,
        start_size: motion.startSize,
        end_size: motion.endSize,
      });
      motion.pathUndo = result.undo;
      motion.sample = null;
      motion.track = result.track;
      motion.rest = false;
      motion.pen = false;
      motion.time = 0;
      motion.meta = await motionCommand('motion.tracks', { character: motion.character });
      animationControls();
      $('motion-pen').setAttribute('aria-pressed', 'false');
      $('motion-canvas').classList.remove('drawing');
      await animationSample();
      toast('Path ready · play to review');
    } catch (e) {
      toast(String(e));
    } finally {
      if ($('motion-path-apply')) $('motion-path-apply').disabled = motion.points.length < 2;
    }
  }
}

const animationStage = new AnimationStageController();
const motionPathController = new MotionPathController();

function animationSeek(time: number) { animationStage.seek(time); }
function animationStep(direction: number) { animationStage.step(direction); }
function animationPause() { animationStage.pause(); }
async function animationPlay() { return animationStage.play(); }
function animationDraw() { animationStage.draw(); }
function pathControls() { return motionPathController.buildControls(); }
function setupPathPen(canvas: any) { motionPathController.setupPen(canvas); }
function drawMotionPath(ctx: CanvasRenderingContext2D, scale: number) { motionPathController.draw(ctx, scale); }
async function applyMotionPath() { return motionPathController.apply(); }

window.addEventListener('pagehide', animationRemember);
