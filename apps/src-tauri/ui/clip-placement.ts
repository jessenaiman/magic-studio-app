// Place only a completed full receipt. The backend verifies media and maps the canvas rectangle.
function placementField(name, label, value, type = 'number') {
  const input = el('input', {id: `blender-place-${name}`, type, value: String(value), step: 'any', 'aria-label': label});
  return {input, node: el('label', {}, label, input)};
}
function placementNumber(input, label) {
  if (!String(input.value).trim()) throw Error(`Enter ${label}`);
  const v = Number(input.value);
  if (!Number.isFinite(v)) throw Error(`Invalid ${label}`);
  return v;
}
function placementValidateId(idValue) {
  if (!/^[a-zA-Z0-9_-]+$/.test(idValue)) throw Error('Placement ID must use letters, numbers, underscores or hyphens.');
}
function placementValidateTiming(t, d, r, box, clipSeconds) {
  if (t < 0 || d <= 0) throw Error('Start must be nonnegative and duration positive.');
  if (r <= 0 || r > 16) throw Error('Playback rate must be in (0,16].');
  if (box[2] <= 0 || box[3] <= 0) throw Error('Placement rectangle width and height must be positive.');
  if (!Number.isFinite(clipSeconds) || clipSeconds <= 0) throw Error('Completed receipt has no valid clip duration.');
  if (d > clipSeconds / r + 1e-6) throw Error(`Clip covers ${(clipSeconds / r).toFixed(3)}s at this playback rate; shorten the duration or lower the rate.`);
}
function placementValidateReceipt(job, jobParameter) {
  if (!job.job || (jobParameter === 'blender_job' && (!job.character || !job.track))) {
    throw Error('Completed receipt is missing its job or source action identity.');
  }
}
function placementRequest(ctx) {
  const job = ctx.getJob();
  if (!job || job.state !== 'completed' || job.partial !== false) {
    throw Error('Render a completed full action before placing it on the video timeline.');
  }
  if (ctx.renderBusy) throw Error('Wait for the current render before placing.');
  placementValidateId(ctx.id.input.value);
  const t = placementNumber(ctx.start.input, 'start');
  const d = placementNumber(ctx.duration.input, 'duration');
  const r = placementNumber(ctx.rate.input, 'playback rate');
  const box = ctx.rect.map(f => placementNumber(f.input, 'rectangle'));
  placementValidateTiming(t, d, r, box, job.clip_seconds);
  placementValidateReceipt(job, ctx.jobParameter);
  return {
    id: ctx.id.input.value,
    [ctx.jobParameter]: job.job,
    ...(ctx.scene ? {scene: ctx.scene.scene} : {}),
    start: t,
    duration: d,
    rect: box,
    playback_rate: r,
    ...(job.character && job.track ? {character: job.character, track: job.track} : {}),
  };
}
function placementConfirmed(result, params, jobParameter) {
  return result.id === params.id
    && result[jobParameter] === params[jobParameter]
    && result.start === params.start
    && result.duration === params.duration
    && result.playback_rate === params.playback_rate
    && JSON.stringify(result.canvas_rect) === JSON.stringify(params.rect)
    && result.media
    && result.backup
    && Number.isInteger(result.track_index);
}
async function placementPlace(ctx) {
  if (ctx.placing || !ctx.current()) return;
  let params;
  try { params = placementRequest(ctx); } catch (error) { ctx.message(String(error), true); return; }
  ctx.placing = true;
  ctx.update();
  ctx.message('Placing rendered action…');
  try {
    const result = await motionCommand('video.place_track', params);
    if (!ctx.current() || ctx.getJob()?.job !== params[ctx.jobParameter]) return;
    ctx.readback.textContent = JSON.stringify(result, null, 2);
    if (result.ok !== true) throw Error(result.error || 'Placement command did not confirm success.');
    if (!placementConfirmed(result, params, ctx.jobParameter)) {
      throw Error('Placement returned incomplete or mismatched confirmation; inspect the result before retrying.');
    }
    ctx.message(`Placed ${result.id} · ${params.scene ? `scene ${params.scene}` : `start ${params.start}s`} · track ${result.track_index}.`);
  } catch (error) {
    if (ctx.current() && ctx.getJob()?.job === params[ctx.jobParameter]) ctx.message(String(error), true);
  } finally {
    ctx.placing = false;
    if (ctx.current()) ctx.update();
  }
}

class ClipPlacementController {
  command: any;
  copy: any;
  current: any;
  defaultId = '';
  duration: any;
  getJob: any;
  id: any;
  inputs: any[];
  jobInfo: any;
  jobParameter: string;
  lastJob: any = null;
  node: any;
  place: any;
  placing = false;
  project: any;
  rate: any;
  readback: any;
  rect: any[];
  renderBusy = false;
  scene: any = null;
  scenePicker: any;
  sceneRevision = 0;
  start: any;
  status: any;
  validationError = false;

  constructor({project, getJob, current, jobParameter = 'blender_job'}) {
    this.project = project;
    this.getJob = getJob;
    this.current = current;
    this.jobParameter = jobParameter;
    this.scenePicker = el('select', {id: 'blender-place-scene', 'aria-label': 'Destination scene'});
    this.id = placementField('id', 'Placement ID', '', 'text');
    this.start = placementField('start', 'Start (absolute seconds)', 0);
    this.duration = placementField('duration', 'Duration (seconds)', 1);
    this.rate = placementField('rate', 'Playback rate', 1);
    this.rect = [
      placementField('x', 'X (video pixels)', 0),
      placementField('y', 'Y (video pixels)', 0),
      placementField('width', 'Width (video pixels)', 0),
      placementField('height', 'Height (video pixels)', 0),
    ];
    this.jobInfo = el('p', {class: 'muted', id: 'blender-place-job'});
    this.status = el('p', {id: 'blender-place-status', role: 'status', 'aria-live': 'polite', class: 'muted'});
    this.command = el('code', {id: 'blender-place-command'});
    this.readback = el('pre', {id: 'blender-place-result', 'aria-label': 'Placement result'});
    this.place = el('button', {id: 'blender-place-submit', class: 'ghost', 'data-commands': 'video.place_track', disabled: true}, 'Place in scene');
    this.copy = el('button', {id: 'blender-place-copy', class: 'ghost', disabled: true}, 'Copy placement command');
    this.inputs = [this.scenePicker, this.id.input, this.start.input, this.duration.input, this.rate.input, ...this.rect.map(f => f.input)];
    this.bindEvents();
    this.node = this.buildNode();
    this.refresh();
  }

  message = (text: string, error = false) => {
    this.status.textContent = text;
    this.status.className = error ? 'image-error' : 'muted';
  };

  request() {
    return placementRequest(this);
  }

  update = () => {
    for (const input of this.inputs) input.disabled = this.placing || this.renderBusy;
    try {
      const params = this.request();
      this.command.textContent = magicCommandText('video.place_track', params, this.project);
      this.place.disabled = this.copy.disabled = this.placing;
      if (this.validationError) {
        this.validationError = false;
        this.message('Ready to place. Start uses absolute project time.');
      }
    } catch (error) {
      this.place.disabled = this.copy.disabled = true;
      this.command.textContent = '';
      if (!this.placing) {
        this.validationError = true;
        this.message(String(error), true);
      }
    }
  };

  sceneId() {
    const tag = this.scene ? `scene-${this.scene.scene}` : 'timeline';
    const next = `${this.jobParameter === 'blender_job' ? 'blender' : 'animation'}-${tag}-${this.getJob()?.track || this.getJob()?.backend || 'action'}`.replace(/[^a-zA-Z0-9_-]/g, '-');
    if (!this.id.input.value || this.id.input.value === this.defaultId) this.id.input.value = next;
    this.defaultId = next;
  }

  refresh = () => {
    const job = this.getJob();
    if ((job?.job || null) !== this.lastJob) {
      this.lastJob = job?.job || null;
      this.readback.textContent = '';
      const canvas = job?.canvas;
      if (Array.isArray(canvas) && canvas.length === 2 && canvas.every(v => Number.isFinite(v) && v > 0)) {
        this.rect.forEach((f, i) => { f.input.value = String([0, 0, ...canvas][i]); });
      }
      this.sceneId();
    }
    this.jobInfo.textContent = job
      ? `${job.character || job.backend || 'Native source'}${job.track ? ` · ${job.track}` : ''} · ${job.job} · ${job.partial === false ? 'full action' : 'preview'}`
      : 'No completed action selected.';
    this.update();
  };

  async selectScene() {
    const revision = ++this.sceneRevision;
    if (!this.scenePicker.value) {
      this.scene = null;
      this.sceneId();
      this.message('Ready to place at any movable start time.');
      this.refresh();
      return;
    }
    const n = Number(this.scenePicker.value);
    this.scene = null;
    this.update();
    try {
      const value = await motionCommand('storyboard.read', {scene: n});
      if (!this.current() || revision !== this.sceneRevision) return;
      if (value.scene !== n || !Number.isFinite(value.start) || !Number.isFinite(value.end) || value.end <= value.start) {
        throw Error('Scene has no valid authored timing.');
      }
      this.scene = value;
      this.start.input.value = String(value.start);
      this.duration.input.value = String(value.end - value.start);
      this.sceneId();
      this.message('Ready to place. Start uses absolute project time.');
      this.refresh();
    } catch (error) {
      if (this.current() && revision === this.sceneRevision) {
        this.message(String(error), true);
        this.place.disabled = this.copy.disabled = true;
      }
    }
  }

  load = async () => {
    try {
      const toc = await motionCommand('storyboard.toc');
      if (!this.current()) return;
      const chapters = (toc.chapters || []).filter(s => Number.isSafeInteger(s.scene) && s.scene > 0);
      this.scenePicker.replaceChildren(
        el('option', {value: ''}, 'Movable timeline (no scene required)'),
        ...chapters.map(s => el('option', {value: String(s.scene)}, s.title || `Scene ${s.scene}`)),
      );
      if (!chapters.length) {
        this.scenePicker.value = '';
        await this.selectScene();
        return;
      }
      this.scenePicker.value = String(chapters[0].scene);
      await this.selectScene();
    } catch {
      if (this.current()) {
        this.scenePicker.replaceChildren(el('option', {value: ''}, 'Movable timeline (no scene required)'));
        this.scenePicker.value = '';
        await this.selectScene();
      }
    }
  };

  setBusy = (value: boolean) => {
    this.renderBusy = value;
    this.update();
  };

  bindEvents() {
    this.scenePicker.addEventListener('change', () => this.selectScene());
    for (const input of this.inputs.filter(n => n !== this.scenePicker)) {
      input.addEventListener('input', () => { this.message('Ready to place.'); this.update(); });
    }
    this.copy.onclick = async () => {
      try {
        this.request();
        await navigator.clipboard.writeText(this.command.textContent);
        if (this.current()) this.message('Exact placement command copied; nothing executed.');
      } catch (error) {
        if (this.current()) this.message(String(error), true);
      }
    };
    this.place.onclick = async () => { await placementPlace(this); };
  }

  buildNode() {
    return el('details', {id: 'blender-placement'},
      el('summary', {id: 'blender-placement-head'}, el('span', {class: 'panel-title'}, 'Place in scene')),
      el('div', {id: 'blender-placement-body'},
        this.jobInfo,
        el('label', {}, 'Authored scene', this.scenePicker),
        this.id.node,
        el('p', {class: 'muted'}, 'An existing placement with this ID is replaced. Rectangle frames the original canvas in video pixels; the backend maps the rendered frame.'),
        this.start.node,
        this.duration.node,
        this.rate.node,
        el('div', {id: 'blender-place-coordinates'}, ...this.rect.map(f => f.node)),
        this.place,
        this.status,
        el('details', {}, el('summary', {}, 'Exact CLI command'), this.command, this.copy),
        el('details', {}, el('summary', {}, 'Placement result'), this.readback)));
  }
}

function clipPlacementControls(options) {
  return new ClipPlacementController(options);
}
