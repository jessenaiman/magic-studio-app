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
