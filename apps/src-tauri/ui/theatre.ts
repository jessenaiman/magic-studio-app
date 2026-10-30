// Stock Theatre UI is contained inside the current Animation surface, so it
// cannot cover the native inspector or Sproket workstation.
class TheatreController {
  frame: any = null;
  key: string | null = null;
  sheet: string | null = null;
  part: string | null = null;
  rotation = 0;
  rotations: Record<string, number> = {};
  ready = false;
  syncing = false;
  availability = 'unavailable';
  error: string | null = null;
  objects: any[] = [];
  playing: any = null;
  position: number | null = null;
  valueUpdate: any = null;
  project: string | null = null;
  stateSentProject: string | null = null;
  load: Promise<any> | null = null;
  savedAt: number | null = null;
  persistence = 'not_loaded';
  exports = new Map<number, any>();
  nextRequest = 0;
  interactionRevision = 0;
  savedRevision = 0;
  gestureActive = false;

  feedback() {
    const liveEdits = this.interactionRevision > this.savedRevision || this.gestureActive
      ? 'needs_save'
      : this.savedAt || this.persistence === 'saved'
        ? 'saved_snapshot'
        : 'no_observed_interaction';
    return {
      availability: this.availability, package_version: '0.7.0', error: this.error,
      selected_objects: this.objects, native_selected_part: motion.selected,
      sequence_position_seconds: this.position, sequence_playing: this.playing,
      native_playing: motion.playing, value_update: this.valueUpdate,
      persistence: this.persistence, saved_snapshot_unix_ms: this.savedAt,
      live_edits: liveEdits, visual_quality: 'not_assessed',
    };
  }

  emitEvent(stage: string, detail: Record<string, any> = {}) {
    if (typeof acFeedback !== 'undefined') {
      acFeedback.report(null, null, { stage: `theatre_${stage}`, ...detail });
    }
  }

  requireSaved() {
    if (this.interactionRevision > this.savedRevision || this.gestureActive) {
      throw new Error('Save motion before changing the animation workspace; Theatre interactions may contain unsaved keyframes.');
    }
  }

  canLeave() {
    try {
      this.requireSaved();
      return true;
    } catch (error) {
      if (typeof toast === 'function') toast(String(error));
      return false;
    }
  }

  async guardIdentityChange(project: string) {
    this.requireSaved();
    const frame = this.frame;
    if (!frame) return;
    if (!this.ready || this.project !== project) {
      throw new Error('Wait for the motion timeline before changing the project.');
    }
    const request = ++this.nextRequest;
    const response = await new Promise<any>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.exports.delete(request);
        reject(new Error('Motion timeline did not confirm its state; project was not changed.'));
      }, 5000);
      this.exports.set(request, { project, resolve, reject, timer, barrier: true });
      frame.contentWindow.postMessage({ magicTheatre: true, type: 'interaction_barrier', request }, location.origin);
    });
    if (frame !== this.frame || state.project !== project || response.project !== project) {
      throw new Error('Project or timeline changed; save was not submitted.');
    }
    if (response.revision > this.interactionRevision) this.interactionRevision = response.revision;
    this.gestureActive = response.active === true;
    this.requireSaved();
  }

  mount(container: any) {
    this.requireSaved();
    const status = el('span', { id: 'theatre-status', class: 'muted' }, 'Loading Theatre…');
    const clips = el('div', { id: 'theatre-clips', class: 'theatre-clips', 'aria-label': 'Authored clips' });
    const frame = el('iframe', { id: 'theatre-timeline', title: 'Theatre motion timeline', src: 'theatre-frame.html' });
    Object.assign(this, {
      frame, key: null, sheet: null, ready: false, rotation: 0, rotations: {},
      availability: 'loading', error: null, objects: [], playing: null, position: null,
      valueUpdate: null, project: null, stateSentProject: null, load: null,
      savedAt: null, persistence: 'not_loaded', interactionRevision: 0,
      savedRevision: 0, gestureActive: false,
    });
    const save = el('button', {
      id: 'theatre-save', class: 'ghost', disabled: true,
      title: 'Save native Theatre keyframes and curves into the current project',
      onclick: () => this.save().catch((error) => toast(String(error))),
    }, 'Save motion');
    let view = 'curves';
    try { view = localStorage.getItem('theatre.view') === 'keys' ? 'keys' : 'curves'; } catch {}
    const views = el('div', { class: 'theatre-views', role: 'tablist', 'aria-label': 'Theatre view' },
      ...[['keys', 'Keys', 'Edit keyframes in Theatre'], ['curves', 'Curves', 'Every animated channel of this clip over time']].map(([key, label, title]) =>
        el('button', {
          class: `theatre-view${view === key ? ' active' : ''}`, id: `theatre-view-${key}`,
          role: 'tab', 'aria-selected': String(view === key), title,
          onclick: () => this.setView(key),
        }, label)));
    const icon = el('span', { class: 'theatre-icon', 'aria-hidden': 'true', html: '<svg viewBox="0 0 24 24"><path d="M3 3h18v18H3zM3 9h18M3 15h18M9 3v18M15 3v18"/></svg>' });
    const curves = el('div', { class: 'theatre-curves', id: 'theatre-curves', 'aria-label': 'Animated channels' });
    if (container.dataset) container.dataset.view = view;
    container.append(
      el('div', { class: 'theatre-heading' }, icon, el('strong', {}, 'Theatre'), views, el('span', { id: 'theatre-info', class: 'theatre-info muted' }), el('div', { class: 'theatre-actions' }, status, save)),
      el('div', { class: 'theatre-cliprow' }, el('span', { class: 'theatre-label' }, 'Clips'), clips),
      el('div', { class: 'theatre-body' }, frame, curves),
    );
    this.syncClips();
  }

  setView(view: string) {
    const section = $('theatre-curves')?.closest?.('.motion-theatre');
    if (!section) return;
    section.dataset.view = view;
    try { localStorage.setItem('theatre.view', view); } catch {}
    for (const key of ['keys', 'curves']) {
      const tab = $(`theatre-view-${key}`);
      tab?.classList.toggle('active', key === view);
      tab?.setAttribute('aria-selected', String(key === view));
    }
    if (view === 'curves' && typeof timelineRender === 'function') timelineRender();
  }

  async selectClipPill(trackId: string | null, isRest: boolean) {
    if (!this.canLeave()) return;
    if (trackId) motion.track = trackId;
    motion.rest = isRest;
    motion.time = 0;
    if (typeof acFeedback !== 'undefined') acFeedback.report(null, null, { stage: 'clip_switched', clip: isRest ? '@rest' : trackId });
    if (typeof animationControls === 'function') animationControls();
    if (typeof animationSample === 'function') await animationSample();
  }

  syncClips() {
    const bar = $('theatre-clips');
    if (!bar || typeof bar.replaceChildren !== 'function' || !motion.meta?.tracks) return;
    const authored = (motion.meta.tracks || []).filter((t: any) => !t.standard);
    const pills = authored.map((t: any) => {
      const active = !motion.rest && motion.track === t.id;
      const label = t.id.replace(/^.*?-beat\d+-/, '').replaceAll('-', ' ');
      const dur = Number.isFinite(t.duration) ? ` · ${Number(t.duration).toFixed(2)}s` : '';
      return el('button', {
        class: `theatre-clip-pill${active ? ' active' : ''}`,
        title: `Open clip sheet ${t.id}`, 'aria-pressed': String(active),
        onclick: () => this.selectClipPill(t.id, false),
      }, `${label}${dur}`);
    });
    pills.push(el('button', {
      class: `theatre-clip-pill${motion.rest ? ' active' : ''}`,
      title: 'Open rest pose sheet', 'aria-pressed': String(Boolean(motion.rest)),
      onclick: () => this.selectClipPill(null, true),
    }, 'Rest'));
    bar.replaceChildren(...pills);
    if (typeof timelineRender === 'function') timelineRender();
  }

  targetSheetId() {
    const raw = motion.rest ? '@rest' : (motion.track || '@rest');
    return `${motion.character} / ${raw.replace(/^@/, '')}`;
  }

  async postContextMessage(project: any, character: any, sheet: string, part: string, parts: string[], key: string) {
    const frame = this.frame;
    try {
      if (this.project !== project) {
        this.project = project;
        this.load = motionCommand('motion.theatre.read', { expected_project: project });
      }
      const loaded = await this.load;
      if (frame !== this.frame || key !== this.key || project !== state.project) return;
      this.persistence = loaded.status;
      const statePayload = this.stateSentProject === project ? undefined : loaded.state;
      this.stateSentProject = project;
      frame.contentWindow.postMessage({ magicTheatre: true, type: 'context', project, character, sheet, part, parts, state: statePayload }, location.origin);
    } catch (error) {
      this.availability = 'error';
      this.error = String(error);
      $('theatre-status').textContent = `Theatre load failed: ${error}`;
      this.emitEvent('error', { error: String(error) });
    }
  }

  async syncContext() {
    if (!this.ready || !motion.meta) return;
    const part = motion.selected || motion.meta.layers[0]?.id;
    if (!part) return;
    const sheet = this.targetSheetId();
    const parts = (motion.meta.layers || []).map((l: any) => l.id);
    const key = JSON.stringify([state.project, motion.character, part, sheet]);
    if (key === this.key) return;
    if (this.sheet !== sheet) { this.sheet = sheet; this.rotations = {}; }
    this.key = key;
    this.part = part;
    this.rotation = this.rotations[part] || 0;
    await this.postContextMessage(state.project, motion.character, sheet, part, parts, key);
  }

  requestExport(frame: any, project: any) {
    const request = ++this.nextRequest;
    return new Promise<any>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.exports.delete(request);
        reject(new Error('Theatre export timed out; no project save was submitted'));
      }, 5000);
      this.exports.set(request, { project, resolve, reject, timer });
      frame.contentWindow.postMessage({ magicTheatre: true, type: 'export', request, project }, location.origin);
    });
  }

  async finishSave(project: any, revision: number, statusSuffix: string) {
    if (this.interactionRevision === revision && !this.gestureActive) this.savedRevision = revision;
    this.savedAt = Date.now();
    this.persistence = 'saved_snapshot';
    $('theatre-status').textContent = `${this.part} · ${statusSuffix}`;
    this.emitEvent('saved', { project, saved_unix_ms: this.savedAt });
    if (typeof animationSample === 'function') await animationSample();
  }

  async saveEmptyExport(project: any, revision: number) {
    if (motion.character) {
      const converted = await motionCommand('motion.clips.convert', { character: motion.character });
      await this.finishSave(project, revision, 'motion timeline saved');
      return converted;
    }
    if (this.interactionRevision === revision && !this.gestureActive) this.savedRevision = revision;
    $('theatre-status').textContent = `${this.part} · no Theatre edits to save`;
    return { saved: false, reason: 'no Theatre edits' };
  }

  async save() {
    if (this.availability !== 'ready') throw new Error('Theatre is not ready to save');
    const project = state.project, frame = this.frame, revision = this.interactionRevision, button = $('theatre-save');
    button.disabled = true;
    try {
      const exported = await this.requestExport(frame, project);
      if (state.project !== project || this.frame !== frame) throw new Error('Project changed during export; no state saved');
      if (!Object.keys(exported.state?.sheetsById || {}).length) return await this.saveEmptyExport(project, revision);
      const result = await motionCommand('motion.theatre.save', { expected_project: project, state: exported.state });
      if (result.saved !== true || result.project !== project) throw new Error('Native Theatre save was not acknowledged');
      if (state.project !== project || this.frame !== frame) throw new Error('Project changed during save; the previous project snapshot was saved');
      await this.finishSave(project, revision, 'motion snapshot saved');
      return result;
    } finally {
      if (frame === this.frame) button.disabled = false;
      if (typeof updateAppFooter === 'function') updateAppFooter();
    }
  }

  seek(time: number) {
    this.syncContext();
    if (this.ready && !this.syncing) {
      this.position = time;
      this.frame.contentWindow.postMessage({ magicTheatre: true, type: 'seek', time }, location.origin);
    }
  }

  render() {
    const previous = this.syncing;
    this.syncing = true;
    try { animationDraw(); } finally { this.syncing = previous; }
  }

  applyTransform(ctx: CanvasRenderingContext2D, layer: any) {
    if (!(this.interactionRevision > this.savedRevision || this.gestureActive)) return;
    const effective = layer.id === this.part ? this.rotation : (this.rotations?.[layer.id] || 0);
    if (!effective) return;
    const pivot = layer.pivot || [0, 0];
    ctx.translate(pivot[0], pivot[1]);
    ctx.rotate(effective * Math.PI / 180);
    ctx.translate(-pivot[0], -pivot[1]);
  }
}

const magicTheatre = new TheatreController();

function theatreFeedback() { return magicTheatre.feedback(); }
function theatreEvent(stage: string, detail: Record<string, any> = {}) { magicTheatre.emitEvent(stage, detail); }
function theatreRequireSaved() { magicTheatre.requireSaved(); }
function theatreCanLeave() { return magicTheatre.canLeave(); }
async function theatreGuardIdentityChange(project: string) { return magicTheatre.guardIdentityChange(project); }
function theatreMount(container: any) { magicTheatre.mount(container); }
function theatreSetView(view: string) { magicTheatre.setView(view); }
function theatreSyncClips() { magicTheatre.syncClips(); }
async function theatreContext() { return magicTheatre.syncContext(); }
function theatreRequestExport(frame: any, project: any) { return magicTheatre.requestExport(frame, project); }
async function theatreFinishSave(project: any, revision: number, statusSuffix: string) {
  return magicTheatre.finishSave(project, revision, statusSuffix);
}
async function theatreSaveEmptyExport(project: any, revision: number) {
  return magicTheatre.saveEmptyExport(project, revision);
}
async function theatreSave() { return magicTheatre.save(); }
function theatreSeek(time: number) { magicTheatre.seek(time); }
function theatreRender() { magicTheatre.render(); }
function handleTheatreInteractionMessage(message: any) {
  if (message.project !== state.project || message.project !== magicTheatre.project) return;
  if (!Number.isSafeInteger(message.revision) || message.revision <= magicTheatre.interactionRevision) return;
  magicTheatre.interactionRevision = message.revision;
  magicTheatre.gestureActive = message.active === true;
  if (typeof acFeedback !== 'undefined') acFeedback.report();
}
function handleTheatreExportReply(message: any) {
  const pending = magicTheatre.exports.get(message.request);
  if (!pending) return;
  clearTimeout(pending.timer);
  magicTheatre.exports.delete(message.request);
  if (message.type === 'error' || (!pending.barrier && message.project !== pending.project)) {
    pending.reject(new Error(message.error || 'Theatre export project mismatch'));
  } else {
    pending.resolve(message);
  }
}
function handleTheatreSelectionMessage(message: any) {
  magicTheatre.objects = message.objects;
  const picked = message.objects?.[0]?.part;
  if (picked && motion.meta?.layers?.some((l: any) => l.id === picked) && motion.selected !== picked) {
    motion.selected = picked;
    magicTheatre.part = picked;
    magicTheatre.rotation = magicTheatre.rotations[picked] || 0;
    if (typeof updateMagicSelection === 'function') updateMagicSelection();
  }
  theatreEvent('selection', { objects: message.objects });
}
function handleTheatreValuesMessage(message: any) {
  const hasLayer = Boolean(motion.meta?.layers?.some((l: any) => l.id === message.part));
  if (hasLayer) magicTheatre.rotations[message.part] = message.rotation;
  if (message.part === magicTheatre.part) {
    magicTheatre.rotation = message.rotation;
    if (!message.sync) theatreRender();
    magicTheatre.valueUpdate = { part: message.part, property: 'rotation', value: message.rotation, received_unix_ms: Date.now(), render_call_completed: true };
  } else if (hasLayer && !message.sync) {
    theatreRender();
  }
}
function handleTheatreReadyMessage(message: any) {
  magicTheatre.availability = 'ready';
  magicTheatre.error = null;
  $('theatre-save').disabled = false;
  $('theatre-status').textContent = `${message.part} · ${magicTheatre.persistence === 'saved' ? 'saved motion loaded' : 'Save motion to persist edits'}`;
  theatreEvent('ready', { part: message.part });
}
function handleTheatreErrorMessage(message: any) {
  if (message.request) handleTheatreExportReply(message);
  magicTheatre.availability = 'error';
  magicTheatre.error = String(message.error);
  $('theatre-status').textContent = `Theatre failed: ${message.error}`;
  theatreEvent('error', { error: message.error });
}
const THEATRE_MESSAGE_HANDLERS: Record<string, (m: any) => void> = {
  interaction: handleTheatreInteractionMessage,
  loaded: () => { magicTheatre.ready = true; theatreContext(); },
  ready: handleTheatreReadyMessage,
  interaction_barrier: handleTheatreExportReply,
  exported: handleTheatreExportReply,
  keyframes: (m) => theatreEvent('keyframes', { sheet: m.sheet, part: m.part, count: m.count, previous: m.previous }),
  selection: handleTheatreSelectionMessage,
  playback: (m) => { const changed = magicTheatre.playing !== m.playing; magicTheatre.playing = m.playing; if (changed) theatreEvent('playback', { playing: m.playing }); },
  values: handleTheatreValuesMessage,
  seek: (m) => {
    if (!motion.sample) return;
    animationPause();
    motion.time = Math.max(0, Math.min(m.time, motion.sample.duration));
    magicTheatre.position = m.time;
    theatreRender();
  },
  error: handleTheatreErrorMessage,
};
window.addEventListener('message', (event) => {
  if (event.source !== magicTheatre.frame?.contentWindow || !event.data?.magicTheatre) return;
  THEATRE_MESSAGE_HANDLERS[event.data.type]?.(event.data);
});
function theatreTransform(ctx: CanvasRenderingContext2D, layer: any) {
  magicTheatre.applyTransform(ctx, layer);
}
