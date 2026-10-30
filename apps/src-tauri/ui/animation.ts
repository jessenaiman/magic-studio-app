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
